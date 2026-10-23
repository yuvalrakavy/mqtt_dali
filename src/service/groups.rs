//! Captured group evidence and save-before-program operations for one fixture.
use super::*;
use crate::atx::{
    connection::Connection,
    transactions::{BusReply, Response, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::{gear::GroupAddress, groups as frames};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Read,
    Assign { group: GroupAddress, member: bool },
    Adopt { reviewed_mask: u16 },
    Repair { reviewed_mask: u16 },
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Assign { .. } => "assign",
            Self::Adopt { .. } => "adopt",
            Self::Repair { .. } => "repair",
        }
    }
    pub(super) fn writes_hardware(self) -> bool {
        matches!(self, Self::Assign { .. } | Self::Repair { .. })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Complete,
    Partial,
    Missing,
    Collision,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Probe {
    pub low: Option<u8>,
    pub high: Option<u8>,
    pub mask: Option<u16>,
    pub quality: Quality,
    pub stale: bool,
}
impl Probe {
    fn empty() -> Self {
        Self {
            low: None,
            high: None,
            mask: None,
            quality: Quality::Missing,
            stale: false,
        }
    }
    fn update(&mut self) {
        self.mask = self
            .low
            .zip(self.high)
            .map(|(l, h)| u16::from_le_bytes([l, h]));
        if self.quality != Quality::Collision {
            self.quality = if self.mask.is_some() {
                Quality::Complete
            } else if self.low.is_some() || self.high.is_some() {
                Quality::Partial
            } else {
                Quality::Missing
            };
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub bus: u8,
    pub address: u8,
    pub revision: u64,
    pub configuration_changed: bool,
    pub expected_mask: u16,
    pub probe: Option<Probe>,
    pub drift_mask: Option<u16>,
    pub reason: Option<String>,
    pub group: Option<u8>,
    pub member: Option<bool>,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub expected: Option<u64>,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub committed: Option<Report>,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) enum Plan {
    Probe,
    Program {
        desired: u16,
        original_stamp: output::Stamp,
    },
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub address: crate::dali::gear::ShortAddress,
    pub context: RequestContext,
    pub action: Action,
    pub plan: Plan,
}
pub(super) struct Observation {
    pub bus: u8,
    pub stamp: output::Stamp,
    pub probe: Option<Probe>,
    pub programmed: bool,
}
pub(super) struct Outcome {
    pub observation: Observation,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) struct Completion {
    pub request: Request,
    pub observation: Option<Observation>,
    pub error: Option<ServiceError>,
}
pub(super) enum Advance {
    Done(Report),
    Program(Route),
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: Action,
    expected: Option<u64>,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    let output = output::resolve(snapshot, target, cause, action != Action::Read)?;
    if action != Action::Read && expected != Some(snapshot.model.inventory().revision) {
        return Err(ServiceError::Rejected("inventory revision changed".into()));
    }
    if let Action::Assign { group, .. } = action {
        if !snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|b| b.bus == output.bus && b.groups.iter().any(|g| g.group_address == group.raw()))
        {
            return Err(ServiceError::Rejected(
                "group must be saved before assignment".into(),
            ));
        }
    }
    Ok(Route {
        controller: output.controller,
        bus: output.bus,
        address: output.address,
        context: output.context,
        action,
        plan: Plan::Probe,
    })
}
pub(super) fn expected_mask(model: &Model, bus: u8, address: u8) -> u16 {
    model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == bus)
        .unwrap()
        .groups
        .iter()
        .filter(|g| g.members.contains(&address))
        .fold(0, |mask, g| mask | (1 << g.group_address))
}
pub(super) fn report(
    model: &Model,
    request: &Request,
    route: &Route,
    outcome: &'static str,
    changed: bool,
    probe: Option<Probe>,
    reason: Option<String>,
) -> Report {
    let expected_mask = expected_mask(model, route.bus, route.address.raw());
    let (group, member) = match request.action {
        Action::Assign { group, member } => (Some(group.raw()), Some(member)),
        _ => (None, None),
    };
    Report {
        operation: request.action.name(),
        outcome,
        target: request.target.clone(),
        target_generation: route.context.generation().unwrap().into(),
        bus: route.bus,
        address: route.address.raw(),
        revision: model.inventory().revision,
        configuration_changed: changed,
        expected_mask,
        drift_mask: probe
            .as_ref()
            .and_then(|p| p.mask)
            .map(|m| m ^ expected_mask),
        probe,
        reason,
        group,
        member,
    }
}
pub(super) fn edits(model: &Model, route: &Route, mask: u16) -> Vec<Edit> {
    let saved = model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == route.bus)
        .unwrap();
    let address = route.address.raw();
    let mut edits = Vec::new();
    let mut names: std::collections::BTreeSet<String> =
        saved.groups.iter().map(|g| g.name.clone()).collect();
    for slot in 0..16 {
        let group = GroupAddress::new(slot).unwrap();
        let member = frames::Mask(mask).contains(group);
        if let Action::Assign {
            group: selected, ..
        } = route.action
        {
            if selected != group {
                continue;
            }
        }
        if let Some(existing) = saved.groups.iter().find(|g| g.group_address == slot) {
            if existing.members.contains(&address) == member {
                continue;
            }
            let mut members = existing.members.clone();
            members.retain(|a| *a != address);
            if member {
                members.push(address);
                members.sort_unstable();
            }
            edits.push(Edit::GroupMembers {
                bus: route.bus,
                address: slot,
                members,
            });
        } else if member {
            let name = discovery::unused_name("Group", names.iter().map(String::as_str));
            names.insert(name.clone());
            edits.push(Edit::AddGroup {
                bus: route.bus,
                group: crate::config::Group {
                    group_address: slot,
                    name,
                    description: None,
                    members: vec![address],
                },
            });
        }
    }
    edits
}
pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let result = match (result, request.committed) {
        (Err(error), Some(mut report)) => {
            report.outcome = "unconfirmed";
            report.probe = None;
            report.drift_mask = None;
            report.reason = Some(error.to_string());
            Ok(report)
        }
        (result, _) => result,
    };
    let _permit = request._permit;
    let reply = request.reply;
    drop(_permit);
    let _ = reply.send(result);
}

pub(super) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let mut observation = Observation {
        bus: route.bus,
        stamp,
        probe: None,
        programmed: matches!(route.plan, Plan::Program { .. }),
    };
    let mut refusal = None;
    let result: Result<(), TransactionError> = async {
        let mut probe = Probe::empty();
        read_probe(&link, &route, stamp, deadline, &mut probe).await?;
        let Some(mask) = probe.mask.filter(|_| probe.quality == Quality::Complete) else {
            observation.probe = Some(probe);
            return Ok(());
        };
        match route.plan {
            Plan::Probe => observation.probe = Some(probe),
            Plan::Program { desired, .. } => {
                if let Action::Repair { reviewed_mask } = route.action {
                    if mask != reviewed_mask {
                        refusal = Some(ServiceError::Rejected(
                            "hardware group mask changed before programming; review again".into(),
                        ));
                        return Ok(());
                    }
                }
                let desired = match route.action {
                    Action::Assign { group, member } => frames::Mask(mask).with(group, member).0,
                    _ => desired,
                };
                // Preparation evidence can never masquerade as post-write verification.
                observation.probe = None;
                for (group, member) in frames::Mask(mask).changes(frames::Mask(desired)) {
                    let replies = exchange(
                        &link,
                        &route,
                        stamp,
                        deadline,
                        frames::setting(route.address, group, member),
                        true,
                    )
                    .await?;
                    if replies.is_empty() {
                        return Err(TransactionError::StreamLoss);
                    }
                    if replies.iter().any(|r| !matches!(r, BusReply::NoResponse)) {
                        // A correlated bus collision/data terminal preserves UART ordering.
                        refusal = Some(ServiceError::Rejected(
                            "group setting collided or returned data; inspect hardware".into(),
                        ));
                        return Ok(());
                    }
                }
                let mut verification = Probe::empty();
                let result = read_probe(&link, &route, stamp, deadline, &mut verification).await;
                observation.probe = Some(verification);
                result?;
            }
        }
        Ok(())
    }
    .await;
    Outcome {
        observation,
        recovery: matches!(result, Err(TransactionError::StreamLoss)),
        error: refusal.or_else(|| result.err().map(|e| ServiceError::Rejected(e.to_string()))),
    }
}
async fn exchange(
    link: &Transactions<Connection>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    frame: u16,
    repeat: bool,
) -> Result<Vec<BusReply>, TransactionError> {
    match link
        .execute_until(
            Command::Send {
                bus: route.bus,
                frame: DaliFrame::Bits16(frame),
                repeat,
            },
            deadline,
            Some((stamp.actor, stamp.bus)),
        )
        .await?
    {
        Response::Dali(replies) => Ok(replies),
        _ => Err(TransactionError::StreamLoss),
    }
}
async fn read_probe(
    link: &Transactions<Connection>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    probe: &mut Probe,
) -> Result<(), TransactionError> {
    for high in [false, true] {
        let replies = exchange(
            link,
            route,
            stamp,
            deadline,
            frames::query(route.address, high),
            false,
        )
        .await?;
        if replies.len() != 1 {
            return Err(TransactionError::StreamLoss);
        }
        match replies[0] {
            BusReply::Value(value) => {
                if high {
                    probe.high = Some(value)
                } else {
                    probe.low = Some(value)
                }
            }
            BusReply::NoResponse => {}
            _ => probe.quality = Quality::Collision,
        }
        probe.update();
    }
    Ok(())
}

impl ServiceHandle {
    pub async fn read_groups(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.groups(target, Action::Read, None, context, deadline, None)
            .await
    }
    /// Mutating group operations require the saved revision and original output authority.
    pub async fn change_groups(
        &self,
        target: &str,
        action: Action,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.groups(
            target,
            action,
            Some(expected_revision),
            context,
            deadline,
            None,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn groups(
        &self,
        target: &str,
        action: Action,
        expected: Option<u64>,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        if self.permit.is_closed() {
            return Err(ServiceError::Closed);
        }
        let permit = self
            .permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| ServiceError::Busy)?;
        resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            action,
            expected,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Groups(Request {
                target: target.into(),
                action,
                expected,
                context,
                deadline,
                completion,
                committed: None,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "group caller deadline; inspect saved intent and hardware".into(),
            ));
        }
        result
            .map_err(|_| {
                ServiceError::Uncertain(
                    "group caller deadline; inspect saved intent and hardware".into(),
                )
            })?
            .map_err(|_| ServiceError::Closed)?
    }
}
