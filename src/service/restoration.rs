//! Restore a saved lifetime after explicit operator association of hardware.
use super::*;
use crate::inventory::{Location, Origin};
use serde::Serialize;
pub(super) mod execute;

#[derive(Debug, Clone)]
pub enum Action {
    ClearShortAddress {
        expected_revision: u64,
    },
    OnlyUnaddressed {
        expected_revision: u64,
    },
    Identified {
        candidate: String,
        candidate_generation: String,
        expected_revision: u64,
    },
}
impl Action {
    pub(super) fn expected(&self) -> u64 {
        match self {
            Self::ClearShortAddress { expected_revision }
            | Self::OnlyUnaddressed { expected_revision }
            | Self::Identified {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::ClearShortAddress { .. } => "clear_short_address",
            Self::OnlyUnaddressed { .. } => "restore_unaddressed_address",
            Self::Identified { .. } => "restore_identified_address",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    Cleared,
    Restored,
    NoUnaddressedFixture,
    MultipleUnaddressedFixtures,
    Unconfirmed,
    ConfigurationUpdatedButUnconfirmed,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: ResultKind,
    pub target: String,
    pub target_generation: String,
    pub current_generation: String,
    pub bus: u8,
    pub previous_address: u8,
    pub address: u8,
    pub revision: u64,
    pub candidate: Option<String>,
    pub candidate_generation: Option<String>,
    pub source_address: Option<u8>,
    pub configuration_changed: bool,
    pub reason: Option<String>,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub route: Option<Route>,
    pub report: Option<Report>,
    pub committed: bool,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Candidate {
    pub target: String,
    pub generation: String,
    pub address: u8,
    pub origin: Origin,
}
#[derive(Clone, Copy)]
pub(super) enum Plan {
    Clear,
    Unique,
    Identified,
}
#[derive(Clone)]
pub(super) struct Route {
    pub target: String,
    pub target_generation: String,
    pub origin: Origin,
    pub controller: String,
    pub bus: u8,
    pub space: discovery::Space,
    pub address: u8,
    pub instances: Vec<(u8, u8)>,
    pub candidate: Option<Candidate>,
    pub context: RequestContext,
    pub original_stamp: Option<output::Stamp>,
    pub plan: Plan,
}
pub(super) enum Step {
    Cleared,
    NoCandidate,
    Multiple,
    ReadyToRebind,
    Restored,
}
pub(super) struct Observation {
    pub stamp: output::Stamp,
    pub stale: bool,
    pub step: Option<Step>,
}
pub(super) struct Outcome {
    pub observation: Observation,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) struct Completion {
    pub request: Box<Request>,
    pub observation: Option<Observation>,
    pub error: Option<ServiceError>,
}
pub(super) enum Advance {
    Done(Report),
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: &Action,
    context: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready || snapshot.model.inventory().revision != action.expected() {
        return Err(ServiceError::Rejected(
            "routing or inventory revision changed".into(),
        ));
    }
    let projections = snapshot.model.projections();
    let old = projections
        .iter()
        .find(|p| p.target == target)
        .ok_or_else(|| ServiceError::Rejected("unknown saved fixture".into()))?;
    let (bus, address, space) = match old.location {
        Location::Output { bus, short_address } => (bus, short_address, discovery::Space::Outputs),
        Location::Input { bus, short_address } => (bus, short_address, discovery::Space::Inputs),
        _ => {
            return Err(ServiceError::Rejected(
                "saved output or input module required".into(),
            ))
        }
    };
    let saved = snapshot
        .model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == bus && b.enabled)
        .ok_or_else(|| ServiceError::Rejected("unknown/disabled bus".into()))?;
    let table = routing::generations(&snapshot.model);
    if context.generation() != table.targets.get(target).map(String::as_str) {
        return Err(ServiceError::Rejected(
            "original saved fixture generation required".into(),
        ));
    }
    let candidate = if let Action::Identified {
        candidate,
        candidate_generation,
        ..
    } = action
    {
        let selected = projections
            .iter()
            .find(|p| p.target == *candidate)
            .ok_or_else(|| ServiceError::Rejected("unknown identified candidate".into()))?;
        let address = match (old.location, selected.location) {
            (
                Location::Output { .. },
                Location::Output {
                    bus: source_bus,
                    short_address,
                },
            )
            | (
                Location::Input { .. },
                Location::Input {
                    bus: source_bus,
                    short_address,
                },
            ) if source_bus == bus && short_address != address => short_address,
            _ => {
                return Err(ServiceError::Rejected(
                    "candidate must be a different fixture in the same bus and address space"
                        .into(),
                ))
            }
        };
        if table.targets.get(candidate) != Some(candidate_generation) {
            return Err(ServiceError::Rejected(
                "identified candidate generation changed".into(),
            ));
        }
        Some(Candidate {
            target: candidate.clone(),
            generation: candidate_generation.clone(),
            address,
            origin: snapshot.model.origin(selected.location).unwrap().clone(),
        })
    } else {
        None
    };
    let instances = if space == discovery::Space::Inputs {
        saved
            .inputs
            .iter()
            .find(|i| i.short_address == address)
            .unwrap()
            .instances
            .iter()
            .map(|i| (i.number, i.instance_type))
            .collect()
    } else {
        Vec::new()
    };
    Ok(Route {
        target: target.into(),
        target_generation: table.targets[target].clone(),
        origin: snapshot.model.origin(old.location).unwrap().clone(),
        controller: snapshot.model.inventory().name.clone(),
        bus,
        space,
        address,
        instances,
        candidate,
        context: context.clone(),
        original_stamp: None,
        plan: match action {
            Action::ClearShortAddress { .. } => Plan::Clear,
            Action::OnlyUnaddressed { .. } => Plan::Unique,
            Action::Identified { .. } => Plan::Identified,
        },
    })
}
pub(super) fn report(action: &Action, route: &Route, revision: u64) -> Report {
    Report {
        operation: action.name(),
        outcome: ResultKind::Unconfirmed,
        target: route.target.clone(),
        target_generation: route.target_generation.clone(),
        current_generation: route.target_generation.clone(),
        bus: route.bus,
        previous_address: route.address,
        address: route.address,
        revision,
        candidate: route.candidate.as_ref().map(|c| c.target.clone()),
        candidate_generation: route.candidate.as_ref().map(|c| c.generation.clone()),
        source_address: route.candidate.as_ref().map(|c| c.address),
        configuration_changed: false,
        reason: None,
    }
}
pub(super) fn current(snapshot: &Snapshot, route: &Route, revision: u64) -> bool {
    let location = match route.space {
        discovery::Space::Outputs => Location::Output {
            bus: route.bus,
            short_address: route.address,
        },
        discovery::Space::Inputs => Location::Input {
            bus: route.bus,
            short_address: route.address,
        },
    };
    snapshot.phase == Phase::Ready
        && snapshot.model.inventory().revision == revision
        && snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|b| b.bus == route.bus && b.enabled)
        && snapshot.model.origin(location) == Some(&route.origin)
        && routing::generations(&snapshot.model)
            .targets
            .get(&route.target)
            == Some(&route.target_generation)
}
pub(super) fn finish(request: Box<Request>, result: Result<Report, ServiceError>) {
    let result = match result {
        Err(error)
            if request.report.is_some()
                && (request.committed
                    || matches!(
                        request.action,
                        Action::OnlyUnaddressed { .. } | Action::ClearShortAddress { .. }
                    )) =>
        {
            let mut report = request.report.clone().unwrap();
            report.outcome = if request.committed {
                ResultKind::ConfigurationUpdatedButUnconfirmed
            } else {
                ResultKind::Unconfirmed
            };
            report.reason = Some(error.to_string());
            Ok(report)
        }
        result => result,
    };
    let Request { reply, _permit, .. } = *request;
    drop(_permit);
    let _ = reply.send(result);
}
impl ServiceHandle {
    /// Clear only the native address, retaining the saved fixture and its reservation.
    pub async fn clear_short_address(
        &self,
        target: &str,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.restore(
            target,
            Action::ClearShortAddress { expected_revision },
            context,
            deadline,
            None,
        )
        .await
    }
    /// Associate explicitly selected hardware with a continuing saved fixture.
    pub async fn restore_address(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.restore(target, action, context, deadline, None).await
    }
    pub(super) async fn restore(
        &self,
        target: &str,
        action: Action,
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
            &action,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Restore(Box::new(Request {
                target: target.into(),
                action,
                context,
                deadline,
                completion,
                route: None,
                report: None,
                committed: false,
                reply,
                _permit: permit,
            })))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "address restoration caller deadline; inspect saved configuration and hardware"
                    .into(),
            ));
        }
        result
            .map_err(|_| {
                ServiceError::Uncertain(
                    "address restoration caller deadline; inspect hardware".into(),
                )
            })?
            .map_err(|_| ServiceError::Closed)?
    }
}
