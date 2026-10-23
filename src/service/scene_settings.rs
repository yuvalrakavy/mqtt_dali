//! Private desired scene rows with retained probe/save/program ownership.
use super::*;
use crate::{
    atx::{
        serial::Serial,
        transactions::{BusReply, Response, TransactionError, Transactions},
        wire::{Command, DaliFrame},
    },
    config::scene_settings::{Color, Level, Setting},
    dali::{
        colour::{self, Reply},
        gear::ShortAddress,
        scenes as codec,
    },
    inventory::{Location, Origin},
};
use serde::Serialize;
mod capture;
pub(super) mod execute;
pub(super) mod view;
use scenes::settings_value::{self, Comparison};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Set {
        fixture: String,
        setting: Setting,
        expected_revision: u64,
    },
    Adopt {
        fixture: String,
        reviewed: scenes::Stored,
        expected_revision: u64,
    },
    Repair {
        fixture: String,
        reviewed: scenes::Stored,
        expected_revision: u64,
    },
    Capture {
        slot: codec::Slot,
    },
}
impl Action {
    pub(super) fn writing(&self) -> bool {
        !matches!(self, Self::Capture { .. })
    }
    fn name(&self) -> &'static str {
        match self {
            Self::Set { .. } => "set",
            Self::Adopt { .. } => "adopt",
            Self::Repair { .. } => "repair",
            Self::Capture { .. } => "capture",
        }
    }
    pub(super) fn expected(&self) -> Option<u64> {
        match self {
            Self::Set {
                expected_revision, ..
            }
            | Self::Adopt {
                expected_revision, ..
            }
            | Self::Repair {
                expected_revision, ..
            } => Some(*expected_revision),
            Self::Capture { .. } => None,
        }
    }
    fn reviewed(&self) -> Option<&scenes::Stored> {
        match self {
            Self::Adopt { reviewed, .. } | Self::Repair { reviewed, .. } => Some(reviewed),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub fixture: String,
    pub fixture_generation: String,
    pub bus: u8,
    pub address: u8,
    pub slot: u8,
    pub revision: u64,
    pub configuration_changed: bool,
    pub saved: Option<Setting>,
    pub stored: Option<scenes::Stored>,
    pub comparison: Comparison,
    pub candidate: Option<Setting>,
    pub reason: Option<String>,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub committed: Option<Report>,
    pub selection: Route,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) enum Plan {
    Probe,
    Program {
        desired: Setting,
        original_stamp: output::Stamp,
    },
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub address: ShortAddress,
    pub slot: codec::Slot,
    pub fixture: String,
    pub fixture_generation: String,
    pub origin: Origin,
    pub action: Action,
    pub context: RequestContext,
    pub plan: Plan,
}
impl Route {
    pub(super) fn read_route(&self) -> scenes::Route {
        scenes::Route {
            controller: self.controller.clone(),
            bus: self.bus,
            address: Some(self.address),
            slot: self.slot,
            action: scenes::Action::Read { slot: self.slot },
            context: self.context.clone(),
        }
    }
    pub(super) fn output_route(&self) -> output::Route {
        output::Route {
            controller: self.controller.clone(),
            bus: self.bus,
            address: self.address,
            context: self.context.clone(),
        }
    }
}
pub(super) struct Observation {
    pub stamp: output::Stamp,
    pub stale: bool,
    pub programmed: bool,
    pub stored: Option<scenes::Stored>,
    pub desired: Option<Setting>,
    pub candidate: Option<Setting>,
    pub reason: Option<String>,
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
    Program(Route),
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: &Action,
    context: &RequestContext,
    committed: bool,
) -> Result<Route, ServiceError> {
    let (bus, address, slot, fixture) = match action {
        Action::Capture { slot } => {
            let route = output::resolve(snapshot, target, context, false)?;
            (route.bus, route.address, *slot, target)
        }
        Action::Set { fixture, .. }
        | Action::Adopt { fixture, .. }
        | Action::Repair { fixture, .. } => {
            let scene = scenes::resolve(
                snapshot,
                target,
                scenes::Action::Recall {
                    group: None,
                    expected_revision: if committed {
                        snapshot.model.inventory().revision
                    } else {
                        action.expected().unwrap()
                    },
                },
                context,
            )?;
            let p = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == *fixture)
                .ok_or_else(|| ServiceError::Rejected("unknown scene fixture".into()))?;
            let Location::Output { bus, short_address } = p.location else {
                return Err(ServiceError::Rejected(
                    "individual output fixture required".into(),
                ));
            };
            if bus != scene.bus {
                return Err(ServiceError::Rejected(
                    "scene and fixture must share a bus".into(),
                ));
            }
            (
                bus,
                ShortAddress::new(short_address).unwrap(),
                scene.slot,
                fixture.as_str(),
            )
        }
    };
    let origin = snapshot
        .model
        .origin(Location::Output {
            bus,
            short_address: address.raw(),
        })
        .unwrap()
        .clone();
    let fixture_generation = routing::generations(&snapshot.model).targets[fixture].clone();
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        address,
        slot,
        fixture: fixture.into(),
        fixture_generation,
        origin,
        action: action.clone(),
        context: context.clone(),
        plan: Plan::Probe,
    })
}
pub(super) fn report(
    model: &Model,
    q: &Request,
    route: &Route,
    outcome: &'static str,
    changed: bool,
    observed: Observation,
) -> Report {
    let saved = settings_value::saved(model, route.bus, route.slot.raw(), route.address.raw());
    Report {
        operation: q.action.name(),
        outcome,
        target: q.target.clone(),
        target_generation: q
            .context
            .generation()
            .unwrap_or(&route.fixture_generation)
            .into(),
        fixture: route.fixture.clone(),
        fixture_generation: route.fixture_generation.clone(),
        bus: route.bus,
        address: route.address.raw(),
        slot: route.slot.raw(),
        revision: model.inventory().revision,
        configuration_changed: changed,
        comparison: settings_value::compare(saved, observed.stored.as_ref()),
        saved,
        stored: observed.stored,
        candidate: observed.candidate,
        reason: observed.reason,
    }
}
pub(super) fn finish(request: Box<Request>, result: Result<Report, ServiceError>) {
    let request = *request;
    let result = match (result, request.committed) {
        (Err(error), Some(mut saved)) => {
            saved.outcome = "unconfirmed";
            saved.stored = None;
            saved.comparison = Comparison::Unresolved;
            saved.reason = Some(error.to_string());
            Ok(saved)
        }
        (result, _) => result,
    };
    let Request { reply, _permit, .. } = request;
    drop(_permit);
    let _ = reply.send(result);
}
impl ServiceHandle {
    pub async fn change_scene_row(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.scene_setting(target, action, context, deadline, None)
            .await
    }
    pub(super) async fn scene_setting(
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
        let permit = self.permit.clone().try_acquire_owned().map_err(|_| {
            if self.permit.is_closed() {
                ServiceError::Closed
            } else {
                ServiceError::Busy
            }
        })?;
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &action,
            &context,
            false,
        )?;
        if identification::blocks_bus(self, route.bus) {
            return Err(ServiceError::Rejected(
                "stop identification before scene settings work".into(),
            ));
        }
        let writing = action.writing();
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::SceneSetting(Box::new(Request {
                target: target.into(),
                action,
                context,
                deadline,
                selection: route,
                completion,
                committed: None,
                reply,
                _permit: permit,
            })))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(if writing {
                ServiceError::Uncertain(
                    "scene settings deadline; inspect saved intent and hardware".into(),
                )
            } else {
                ServiceError::Expired
            });
        }
        result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?
    }
}
