//! Fresh input feedback evidence under the existing physical owner. Indicators
//! do not configure events, replace input values or revoke received notices.
use crate::atx::{
    connection::Connection,
    transactions::{BusReply, TransactionError, Transactions},
    wire::DaliFrame,
};
use crate::dali::input::{
    features::{self, Scope},
    feedback as frames, Address, DeviceQuery, Instance, InstanceQuery,
};
use crate::inventory::Location;
use crate::service::{
    commissioning::{Bus, Error},
    discovery::Space,
    output::Stamp,
    routing, Message, Phase, RequestContext, ServiceError, ServiceHandle, Snapshot,
};
use serde::Serialize;
use std::time::Duration;
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit},
    time::{timeout_at, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub features: Option<Vec<u8>>,
    pub capability: Option<u8>,
    pub supported: Option<bool>,
    pub active: Option<bool>,
    pub quality: &'static str,
}
impl Values {
    fn complete_supported(&self) -> bool {
        self.quality == "complete"
            && self.supported == Some(true)
            && self.features.as_ref().is_some_and(|v| v.contains(&32))
            && self.capability.is_some_and(|v| v & 9 != 0)
            && self.active.is_some()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub revision: u64,
    pub bus: u8,
    pub address: u8,
    pub instance: Option<u8>,
    pub reported_type: Option<u8>,
    pub before: Option<Values>,
    pub feedback: Option<Values>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
#[derive(Debug, Clone)]
pub(in crate::service) enum Action {
    Read,
    Set { expected: u64, active: bool },
}
impl Action {
    pub(in crate::service) fn writing(&self) -> bool {
        matches!(self, Self::Set { .. })
    }
    pub(in crate::service) fn name(&self) -> &'static str {
        if self.writing() {
            "set_input_feedback"
        } else {
            "read_input_feedback"
        }
    }
}
#[derive(Clone)]
pub(in crate::service) struct Route {
    pub controller: String,
    pub bus: u8,
    address: Address,
    scope: Scope,
    kind: Option<u8>,
    pub context: RequestContext,
    action: Action,
    revision: u64,
}
pub(in crate::service) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub started: bool,
    report: Box<Report>,
    _completion: Option<mqtt_bridge_kit::CommandCompletion>,
    _permit: OwnedSemaphorePermit,
}
pub(in crate::service) struct Observation {
    pub report: Box<Report>,
    pub stamp: Stamp,
    pub stale: bool,
}
pub(in crate::service) struct Outcome {
    pub observation: Observation,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(in crate::service) struct Completion {
    pub request: Request,
    pub observation: Option<Observation>,
    pub error: Option<ServiceError>,
}
fn reject(reason: impl std::fmt::Display) -> ServiceError {
    ServiceError::Rejected(reason.to_string())
}
pub(in crate::service) fn resolve(
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
        .ok_or_else(|| reject("unknown input target"))?;
    let (bus, address, scope, kind) = match p.location {
        Location::Input { bus, short_address } => (bus, short_address, Scope::Device, None),
        Location::InputInstance {
            bus,
            short_address,
            instance,
        } => (
            bus,
            short_address,
            Scope::Instance(Instance::new(instance).map_err(reject)?),
            p.instance_type,
        ),
        _ => return Err(reject("individual input module or instance required")),
    };
    if !snapshot
        .model
        .inventory()
        .buses
        .iter()
        .any(|b| b.bus == bus && b.enabled)
    {
        return Err(reject("bus is disabled"));
    }
    let table = routing::generations(&snapshot.model);
    let token = &table.targets[target];
    if cause.generation().is_some_and(|g| g != token) {
        return Err(reject("retired input generation"));
    }
    if let Action::Set { expected, .. } = action {
        if cause.generation().is_none() || *expected != snapshot.model.inventory().revision {
            return Err(reject(
                "original input generation and inventory revision required",
            ));
        }
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        address: Address::new(address).map_err(reject)?,
        scope,
        kind,
        context: RequestContext::new(token.clone())
            .map_err(reject)?
            .with_traceparent(cause.traceparent()),
        action: action.clone(),
        revision: snapshot.model.inventory().revision,
    })
}
fn report(target: &str, r: &Route) -> Report {
    Report {
        operation: r.action.name(),
        outcome: "unavailable",
        target: target.into(),
        target_generation: r.context.generation().unwrap().into(),
        revision: r.revision,
        bus: r.bus,
        address: r.address.raw(),
        instance: match r.scope {
            Scope::Device => None,
            Scope::Instance(i) => Some(i.raw()),
        },
        reported_type: None,
        before: None,
        feedback: None,
        reason: None,
        configuration_changed: false,
    }
}
fn byte(reply: BusReply) -> Option<u8> {
    if let BusReply::Value(v) = reply {
        Some(v)
    } else {
        None
    }
}
async fn query(bus: &Bus<'_>, frame: u32) -> Result<BusReply, Error> {
    bus.query(DaliFrame::Bits24(frame)).await
}
fn presence_frame(r: &Route) -> u32 {
    match r.scope {
        Scope::Device => r.address.query(DeviceQuery::Status),
        Scope::Instance(i) => r.address.query_instance(i, InstanceQuery::Type),
    }
}
fn proof(r: &Route, reply: BusReply) -> (bool, Option<u8>, &'static str) {
    let raw = byte(reply);
    match r.scope {
        Scope::Device => (raw.is_some(), None, "unavailable"),
        Scope::Instance(_) => (
            raw.is_some() && raw == r.kind,
            raw,
            if raw.is_some() {
                "type_mismatch"
            } else {
                "unavailable"
            },
        ),
    }
}
async fn feature_list(bus: &Bus<'_>, r: &Route) -> Result<Option<Vec<u8>>, Error> {
    let Some(first) = byte(query(bus, r.scope.query(r.address, features::Query::Type)).await?)
    else {
        return Ok(None);
    };
    let mut next = Vec::new();
    if first == 255 {
        for _ in 0..=features::MAX_FEATURES {
            let Some(raw) =
                byte(query(bus, r.scope.query(r.address, features::Query::Next)).await?)
            else {
                return Ok(None);
            };
            next.push(raw);
            if raw == 254 {
                break;
            }
            if !(32..=96).contains(&raw) || next.windows(2).any(|v| v[0] >= v[1]) {
                return Ok(None);
            }
        }
    }
    Ok(features::decode(first, &next)
        .ok()
        .map(|v| v.into_iter().map(|v| v.raw()).collect()))
}
struct Probe {
    kind: Option<u8>,
    values: Option<Values>,
    outcome: &'static str,
}
async fn probe(bus: &Bus<'_>, r: &Route) -> Result<Probe, Error> {
    let (present, kind, outcome) = proof(r, query(bus, presence_frame(r)).await?);
    if !present {
        return Ok(Probe {
            kind,
            values: None,
            outcome,
        });
    }
    let mut v = Values {
        features: feature_list(bus, r).await?,
        capability: None,
        supported: None,
        active: None,
        quality: "partial",
    };
    if let Some(list) = &v.features {
        if !list.contains(&32) {
            v.supported = Some(false);
            v.quality = "complete";
        } else {
            v.capability =
                byte(query(bus, frames::Query::Capability.frame(r.address, r.scope)).await?);
            if let Some(raw) = v.capability {
                if raw == 0 {
                    v.supported = Some(false);
                    v.quality = "complete";
                } else if raw & 9 != 0 {
                    v.supported = Some(true);
                    v.active =
                        match query(bus, frames::Query::Active.frame(r.address, r.scope)).await? {
                            BusReply::Value(255) => Some(true),
                            BusReply::NoResponse => Some(false),
                            _ => None,
                        };
                    if v.active.is_some() {
                        v.quality = "complete";
                    }
                }
            }
        }
    }
    // A silent Active reply is never false without closing presence/type proof.
    let (present, kind, outcome) = proof(r, query(bus, presence_frame(r)).await?);
    if !present {
        return Ok(Probe {
            kind,
            values: None,
            outcome,
        });
    }
    let outcome = if v.quality != "complete" {
        "unavailable"
    } else if v.supported == Some(false) {
        "unsupported"
    } else {
        "current"
    };
    Ok(Probe {
        kind,
        values: Some(v),
        outcome,
    })
}
pub(in crate::service) async fn execute(
    link: Transactions<Connection>,
    r: Route,
    stamp: Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = Bus {
        link: &link,
        bus: r.bus,
        space: Space::Inputs,
        stamp,
        deadline,
    };
    let mut report = report(&target, &r);
    let mut attempted = false;
    let result: Result<(), Error> = async {
        let first = probe(&bus, &r).await?;
        report.reported_type = first.kind;
        report.outcome = first.outcome;
        report.feedback = first.values;
        let active = match &r.action {
            Action::Read => return Ok(()),
            Action::Set { active, .. } => *active,
        };
        report.before = report.feedback.clone();
        if !report
            .feedback
            .as_ref()
            .is_some_and(Values::complete_supported)
        {
            report.reason =
                Some("complete supported feedback evidence required before activation".into());
            return Ok(());
        }
        report.outcome = "unconfirmed";
        attempted = true;
        let action = if active {
            frames::Action::Activate
        } else {
            frames::Action::Stop
        };
        bus.command(DaliFrame::Bits24(action.frame(r.address, r.scope)), false)
            .await?;
        let after = probe(&bus, &r).await?;
        report.reported_type = after.kind;
        report.feedback = after.values;
        if report
            .feedback
            .as_ref()
            .is_some_and(|v| v.complete_supported() && v.active == Some(active))
        {
            report.outcome = "set";
        } else {
            report.reason = Some("feedback was not confirmed by complete matching readback".into());
        }
        Ok(())
    }
    .await;
    let recovery = matches!(
        &result,
        Err(Error::Transaction(TransactionError::StreamLoss))
    );
    let mut error = None;
    if let Err(e) = result {
        let reason = match e {
            Error::Transaction(e) => e.to_string(),
            Error::Evidence(e) => e.into(),
        };
        report.reason = Some(reason.clone());
        report.feedback = None;
        if attempted {
            report.outcome = "unconfirmed";
        } else {
            error = Some(reject(reason));
        }
    }
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
pub(in crate::service) fn accept(
    snapshot: &Snapshot,
    c: &mut Completion,
    hat: &crate::service::hat::Hat,
) -> Result<Report, ServiceError> {
    if let Some(error) = c.error.take() {
        return Err(error);
    }
    let o = c
        .observation
        .take()
        .ok_or_else(|| reject("feedback produced no final evidence"))?;
    let q = &c.request;
    let r = resolve(snapshot, &q.target, &q.action, &q.context)?;
    if Instant::now() >= q.deadline
        || o.stale
        || !hat.evidence_current(r.bus, o.stamp)
        || r.context.generation() != Some(o.report.target_generation.as_str())
    {
        return Err(if q.action.writing() {
            ServiceError::Uncertain("feedback evidence changed before acceptance".into())
        } else {
            reject("feedback evidence changed before acceptance")
        });
    }
    Ok(*o.report)
}
pub(in crate::service) fn finish(q: Request, result: Result<Report, ServiceError>) {
    let result = match result {
        Err(e) if q.started => {
            let mut r = *q.report;
            r.outcome = "unconfirmed";
            r.reason = Some(e.to_string());
            r.feedback = None;
            Ok(r)
        }
        r => r,
    };
    drop(q._permit);
    let _ = q.reply.send(result);
}
impl ServiceHandle {
    /// Inspect actual device/instance feature and feedback evidence; no input events.
    pub async fn read_input_feedback(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.input_feedback(target, Action::Read, context, deadline, None)
            .await
    }
    /// Send an explicit Activate/Stop after fresh support, then verify actual state.
    pub async fn set_input_feedback(
        &self,
        target: &str,
        expected: u64,
        active: bool,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.input_feedback(
            target,
            Action::Set { expected, active },
            context,
            deadline,
            None,
        )
        .await
    }
    pub(in crate::service) async fn input_feedback(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        let deadline = deadline.min(Instant::now() + Duration::from_secs(8));
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
        let (reply, receive) = oneshot::channel();
        let writing = action.writing();
        self.send
            .try_send(Message::InputFeedback(Request {
                target: target.into(),
                action,
                context: r.context.clone(),
                deadline,
                reply,
                started: false,
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
                    "feedback caller deadline; inspect hardware before retrying".into(),
                )
            } else {
                reject("feedback caller deadline")
            });
        }
        result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?
    }
}
