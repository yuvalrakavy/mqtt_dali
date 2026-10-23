//! Native group power/level with individual-address evidence under one owner.
use super::*;
use crate::{
    atx::{
        serial::Serial,
        transactions::{TransactionError, Transactions},
        wire::DaliFrame,
    },
    dali::{
        gear::{Destination, GroupAddress, Intent, Query, ShortAddress},
        groups::{
            self as frames,
            state::{self, Sample},
        },
    },
};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Values {
    pub quality: &'static str,
    pub power_state: Option<bool>,
    pub brightness: Option<f64>,
    pub power_mixed: Option<bool>,
    pub brightness_mixed: Option<bool>,
    pub known_members: usize,
    pub unresolved: Vec<u8>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub target: String,
    pub target_generation: String,
    pub address: u8,
    pub expected_member: bool,
    pub member: Option<bool>,
    pub membership_low: Option<scenes::Reading>,
    pub membership_high: Option<scenes::Reading>,
    pub actual_level: Option<scenes::Reading>,
    pub status: Option<scenes::Reading>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub revision: u64,
    pub bus: u8,
    pub group: u8,
    pub coverage: &'static str,
    pub membership_matches: Option<bool>,
    pub values: Option<Values>,
    pub members: Vec<Member>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
#[derive(Debug, Clone)]
pub(super) enum Action {
    Read,
    Set { expected: u64, intent: Intent },
}
impl Action {
    pub(super) fn writing(&self) -> bool {
        matches!(self, Self::Set { .. })
    }
    pub(super) fn name(&self) -> &'static str {
        if self.writing() {
            "set_group_output"
        } else {
            "read_group_output"
        }
    }
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub context: RequestContext,
    group: GroupAddress,
    revision: u64,
    members: Vec<Member>,
    action: Action,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub started: bool,
    pub bus: u8,
    report: Box<Report>,
    _completion: Option<mqtt_bridge_kit::CommandCompletion>,
    _permit: OwnedSemaphorePermit,
}
pub(super) struct Observation {
    report: Box<Report>,
    stamp: output::Stamp,
    pub stale: bool,
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
fn reject(reason: impl std::fmt::Display) -> ServiceError {
    ServiceError::Rejected(reason.to_string())
}
pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: &Action,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(reject("routing is not ready"));
    }
    let p = snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .ok_or_else(|| reject("unknown saved group"))?;
    let crate::inventory::Location::Group { bus, group_address } = p.location else {
        return Err(reject("saved group target required"));
    };
    let saved = snapshot
        .model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == bus && b.enabled)
        .ok_or_else(|| reject("bus is disabled"))?;
    let group = saved
        .groups
        .iter()
        .find(|g| g.group_address == group_address)
        .ok_or_else(|| reject("unknown saved group"))?;
    let table = routing::generations(&snapshot.model);
    let token = &table.targets[target];
    if cause.generation().is_some_and(|v| v != token) {
        return Err(reject("retired group generation"));
    }
    if let Action::Set { expected, .. } = action {
        if cause.generation().is_none() || *expected != snapshot.model.inventory().revision {
            return Err(reject(
                "original group generation and inventory revision required",
            ));
        }
    }
    let members = snapshot
        .model
        .projections()
        .into_iter()
        .filter_map(|p| {
            let crate::inventory::Location::Output {
                bus: member_bus,
                short_address,
            } = p.location
            else {
                return None;
            };
            (member_bus == bus).then(|| Member {
                target_generation: table.targets[&p.target].clone(),
                target: p.target,
                address: short_address,
                expected_member: group.members.contains(&short_address),
                member: None,
                membership_low: None,
                membership_high: None,
                actual_level: None,
                status: None,
            })
        })
        .collect();
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        group: GroupAddress::new(group_address).map_err(reject)?,
        context: RequestContext::new(token.clone())
            .map_err(reject)?
            .with_traceparent(cause.traceparent()),
        revision: snapshot.model.inventory().revision,
        members,
        action: action.clone(),
    })
}
pub(super) fn admit(handle: &ServiceHandle, route: &Route) -> Result<(), ServiceError> {
    if route.action.writing() && identification::blocks_bus(handle, route.bus) {
        return Err(reject("stop identification before controlling this group"));
    }
    Ok(())
}
pub(super) fn invalidate_actual(snapshot: &mut Snapshot, bus: u8) {
    for sample in snapshot.outputs.values_mut().filter(|s| s.bus == bus) {
        sample.invalidate("native group command requires fresh actual readback");
    }
}
fn report(target: &str, r: &Route) -> Report {
    Report {
        operation: r.action.name(),
        outcome: "unavailable",
        target: target.into(),
        target_generation: r.context.generation().unwrap().into(),
        revision: r.revision,
        bus: r.bus,
        group: r.group.raw(),
        coverage: "saved_inventory",
        membership_matches: None,
        values: None,
        members: r.members.clone(),
        reason: None,
        configuration_changed: false,
    }
}
fn reading(
    raw: &Option<crate::atx::transactions::BusReply>,
    mask: bool,
) -> Option<scenes::Reading> {
    use crate::atx::transactions::BusReply;
    Some(match raw.as_ref()? {
        BusReply::Value(255) if mask => scenes::Reading::Mask { raw: 255 },
        BusReply::Value(v) => scenes::Reading::Value { raw: u16::from(*v) },
        BusReply::NoResponse => scenes::Reading::NoResponse {},
        BusReply::ReceiveCollision(code) => scenes::Reading::ReceiveCollision { code: *code },
        BusReply::TransmitCollision(code) => scenes::Reading::TransmitCollision { code: *code },
    })
}
fn capture(report: &mut Report, r: &Route, samples: &[Sample]) {
    let roster = r
        .members
        .iter()
        .map(|m| ShortAddress::new(m.address).unwrap())
        .collect::<Vec<_>>();
    let summary =
        state::summarize(r.group, &roster, samples).expect("validated frozen output roster");
    report.values = Some(Values {
        quality: match summary.quality {
            state::Quality::Uniform => "uniform",
            state::Quality::Mixed => "mixed",
            state::Quality::Empty => "empty",
            state::Quality::Partial => "partial",
            state::Quality::Unavailable => "unavailable",
        },
        power_state: summary.power,
        brightness: summary.brightness(),
        power_mixed: summary.power_mixed,
        brightness_mixed: summary.level_mixed,
        known_members: summary.known_members,
        unresolved: summary.unresolved.iter().map(|a| a.raw()).collect(),
    });
    let mut complete = true;
    let mut matches = true;
    for (member, sample) in report.members.iter_mut().zip(samples) {
        member.member = sample
            .membership()
            .map(|mask| mask & (1 << r.group.raw()) != 0);
        complete &= member.member.is_some();
        matches &= member.member == Some(member.expected_member);
        member.membership_low = reading(&sample.membership_low, false);
        member.membership_high = reading(&sample.membership_high, false);
        member.actual_level = reading(&sample.actual_level, true);
        member.status = reading(&sample.status, false);
    }
    report.membership_matches = complete.then_some(matches);
}
pub(super) async fn execute(
    link: Transactions<Serial>,
    r: Route,
    stamp: output::Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = commissioning::Bus {
        link: &link,
        bus: r.bus,
        space: discovery::Space::Outputs,
        stamp,
        deadline,
    };
    let mut report = report(&target, &r);
    // Preserve the complete original roster even if capture stops at its first row.
    let mut samples = r
        .members
        .iter()
        .map(|m| Sample::unvisited(ShortAddress::new(m.address).unwrap()))
        .collect::<Vec<_>>();
    let result: Result<(), commissioning::Error> = async {
        if let Action::Set { intent, .. } = r.action {
            report.outcome = "unconfirmed";
            bus.command(
                DaliFrame::Bits16(intent.frame(Destination::Group(r.group))),
                false,
            )
            .await?;
            report.outcome = "sent";
        } else {
            report.outcome = "observed";
        }
        for sample in &mut samples {
            sample.membership_low = Some(
                bus.query(DaliFrame::Bits16(frames::query(sample.address, false)))
                    .await?,
            );
            sample.membership_high = Some(
                bus.query(DaliFrame::Bits16(frames::query(sample.address, true)))
                    .await?,
            );
            if sample
                .membership()
                .is_some_and(|mask| mask & (1 << r.group.raw()) != 0)
            {
                sample.actual_level = Some(
                    bus.query(DaliFrame::Bits16(sample.address.query(Query::ActualLevel)))
                        .await?,
                );
                sample.status = Some(
                    bus.query(DaliFrame::Bits16(sample.address.query(Query::Status)))
                        .await?,
                );
            }
        }
        Ok(())
    }
    .await;
    capture(&mut report, &r, &samples);
    let recovery = matches!(
        &result,
        Err(commissioning::Error::Transaction(
            TransactionError::StreamLoss
        ))
    );
    let error = result.err().map(|e| {
        let reason = match e {
            commissioning::Error::Transaction(e) => e.to_string(),
            commissioning::Error::Evidence(e) => e.into(),
        };
        report.reason = Some(reason.clone());
        if r.action.writing() {
            report.outcome = "unconfirmed";
            ServiceError::Uncertain(reason)
        } else {
            reject(reason)
        }
    });
    Outcome {
        observation: Observation {
            report: Box::new(report),
            stamp,
            stale: false,
        },
        error,
        recovery,
    }
}
pub(super) fn accept(
    snapshot: &Snapshot,
    c: &mut Completion,
    hat: &hat::Hat,
) -> Result<Report, ServiceError> {
    if let Some(error) = c.error.take() {
        return Err(error);
    }
    let o = c
        .observation
        .take()
        .ok_or_else(|| reject("group produced no final evidence"))?;
    let q = &c.request;
    let r = resolve(snapshot, &q.target, &q.action, &q.context)?;
    if Instant::now() >= q.deadline
        || o.stale
        || !hat.evidence_current(r.bus, o.stamp)
        || r.context.generation() != Some(o.report.target_generation.as_str())
        || r.revision != o.report.revision
    {
        return Err(if q.action.writing() {
            ServiceError::Uncertain("group evidence changed before acceptance".into())
        } else {
            reject("group evidence changed before acceptance")
        });
    }
    Ok(*o.report)
}
pub(super) fn finish(q: Request, result: Result<Report, ServiceError>) {
    let result = match result {
        Err(e) if q.started => {
            let mut r = *q.report;
            r.outcome = "unconfirmed";
            r.reason = Some(e.to_string());
            Ok(r)
        }
        r => r,
    };
    drop(q._permit);
    let _ = q.reply.send(result);
}
impl ServiceHandle {
    /// Sample actual membership and individual levels for the whole saved bus roster.
    /// A supplied generation must match; an omitted one is captured once.
    pub async fn read_group_output(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.group_output(target, Action::Read, context, deadline, None)
            .await
    }
    /// Send one native group intent and return sampled actual values, not an echo.
    /// Requires original group authority and revision; never changes assignments.
    pub async fn set_group_output(
        &self,
        target: &str,
        expected: u64,
        intent: Intent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.group_output(
            target,
            Action::Set { expected, intent },
            context,
            deadline,
            None,
        )
        .await
    }
    pub(super) async fn group_output(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        let deadline = deadline.min(Instant::now() + Duration::from_secs(15));
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
        let r = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &action,
            &context,
        )?;
        admit(self, &r)?;
        let (reply, receive) = oneshot::channel();
        let writing = action.writing();
        self.send
            .try_send(Message::GroupOutput(Request {
                target: target.into(),
                action,
                context: r.context.clone(),
                deadline,
                reply,
                started: false,
                bus: r.bus,
                report: Box::new(report(target, &r)),
                _completion: completion,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(if writing {
                ServiceError::Uncertain(
                    "group caller deadline; inspect hardware before retrying".into(),
                )
            } else {
                reject("group caller deadline")
            });
        }
        result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?
    }
}
