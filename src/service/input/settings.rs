//! Explicit hardware-owned event configuration, separate from current values.
use crate::atx::{
    connection::Connection,
    transactions::{BusReply, TransactionError, Transactions},
    wire::DaliFrame,
};
use crate::dali::input::{settings as frames, Group, InstanceQuery, InstanceType};
use crate::service::{
    commissioning::{Bus, Error},
    discovery::Space,
    input::readback,
    output::Stamp,
    Message, Phase, RequestContext, ServiceError, ServiceHandle, Snapshot,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit},
    time::{timeout_at, Instant},
};
mod button;
mod sensor;
pub use button::{Patch as ButtonPatch, Values as ButtonValues};
pub use sensor::{Patch as SensorPatch, Values as SensorValues};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub enabled: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub event_priority: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub event_scheme: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub event_filter: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub groups: Option<[Option<u8>; 3]>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub button_timers: Option<ButtonPatch>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub sensor_settings: Option<SensorPatch>,
}
fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub enabled: Option<bool>,
    pub event_priority: Option<u8>,
    pub event_scheme: Option<u8>,
    pub event_filter: Option<u32>,
    pub filter_bytes: [Option<u8>; 3],
    pub filter_width: u8,
    /// Raw group bytes: FF is unassigned; null is unavailable.
    pub groups: [Option<u8>; 3],
    pub button_timers: Option<ButtonValues>,
    pub sensor_settings: Option<SensorValues>,
    pub quality: &'static str,
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
    pub instance: u8,
    pub reported_type: Option<u8>,
    pub before: Option<Values>,
    pub settings: Option<Values>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
impl Patch {
    fn plans(&self, kind: u8) -> Result<Vec<frames::Setting>, ServiceError> {
        use frames::{EventFilter, EventPriority, EventScheme, GroupSlot, Setting};
        let mut values = Vec::new();
        if let Some(v) = self.enabled {
            values.push(Setting::Enabled(v));
        }
        if let Some(v) = self.event_priority {
            values.push(Setting::Priority(EventPriority::new(v).map_err(reject)?));
        }
        if let Some(v) = self.event_scheme {
            values.push(Setting::Scheme(EventScheme::new(v).map_err(reject)?));
        }
        if let Some(v) = self.event_filter {
            values.push(Setting::Filter(
                EventFilter::new(v)
                    .and_then(|f| f.validate_for(InstanceType::from_raw(kind)))
                    .map_err(reject)?,
            ));
        }
        if let Some(groups) = self.groups {
            for (slot, group) in [GroupSlot::Primary, GroupSlot::Group1, GroupSlot::Group2]
                .into_iter()
                .zip(groups)
            {
                values.push(Setting::Group(
                    slot,
                    group.map(Group::new).transpose().map_err(reject)?,
                ));
            }
        }
        if let Some(timers) = &self.button_timers {
            if kind != 1 {
                return Err(reject("button timers require a Part301 button instance"));
            }
            timers.plans()?;
        }
        if let Some(sensor) = &self.sensor_settings {
            sensor.plans(kind)?;
        }
        if values.is_empty() && self.button_timers.is_none() && self.sensor_settings.is_none() {
            return Err(ServiceError::Rejected(
                "change at least one input setting".into(),
            ));
        }
        Ok(values)
    }
    fn matches(&self, v: &Values) -> bool {
        self.enabled.is_none_or(|x| v.enabled == Some(x))
            && self
                .event_priority
                .is_none_or(|x| v.event_priority == Some(x))
            && self.event_scheme.is_none_or(|x| v.event_scheme == Some(x))
            && self.event_filter.is_none_or(|x| v.event_filter == Some(x))
            && self.groups.is_none_or(|groups| {
                groups
                    .into_iter()
                    .zip(v.groups)
                    .all(|(wanted, actual)| actual == Some(wanted.unwrap_or(255)))
            })
            && self
                .button_timers
                .as_ref()
                .is_none_or(|p| v.button_timers.as_ref().is_some_and(|v| p.matches(v)))
            && self
                .sensor_settings
                .as_ref()
                .is_none_or(|p| v.sensor_settings.as_ref().is_some_and(|v| p.matches(v)))
    }
}
fn reject(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Rejected(error.to_string())
}

#[derive(Debug, Clone)]
pub(in crate::service) enum Action {
    Read,
    Configure { expected: u64, settings: Patch },
}
impl Action {
    pub(in crate::service) fn writing(&self) -> bool {
        matches!(self, Self::Configure { .. })
    }
    pub(in crate::service) fn name(&self) -> &'static str {
        if self.writing() {
            "configure_input"
        } else {
            "read_input_settings"
        }
    }
}
#[derive(Clone)]
pub(in crate::service) struct Route {
    pub input: readback::Route,
    pub action: Action,
    pub revision: u64,
    plans: Vec<frames::Setting>,
    button_plans: Vec<crate::dali::input::button::Value>,
    sensor_plans: Vec<crate::dali::input::sensor::Value>,
}
pub(in crate::service) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub started: bool,
    pub report: Box<Report>,
    pub _completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub _permit: OwnedSemaphorePermit,
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

pub(in crate::service) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: &Action,
    context: &RequestContext,
) -> Result<Route, ServiceError> {
    let input = readback::resolve(snapshot, target, context)?;
    let mut plans = Vec::new();
    let mut button_plans = Vec::new();
    let mut sensor_plans = Vec::new();
    if let Action::Configure { expected, settings } = action {
        if context.generation().is_none() || *expected != snapshot.model.inventory().revision {
            return Err(ServiceError::Rejected(
                "original input generation and inventory revision required".into(),
            ));
        }
        plans = settings.plans(input.kind)?;
        button_plans = settings
            .button_timers
            .as_ref()
            .map(ButtonPatch::plans)
            .transpose()?
            .unwrap_or_default();
        sensor_plans = settings
            .sensor_settings
            .as_ref()
            .map(|p| p.plans(input.kind))
            .transpose()?
            .unwrap_or_default();
    }
    Ok(Route {
        input,
        action: action.clone(),
        revision: snapshot.model.inventory().revision,
        plans,
        button_plans,
        sensor_plans,
    })
}
fn report(target: &str, route: &Route) -> Report {
    Report {
        operation: route.action.name(),
        outcome: "unavailable",
        target: target.into(),
        target_generation: route.input.context.generation().unwrap().into(),
        revision: route.revision,
        bus: route.input.bus,
        address: route.input.address.raw(),
        instance: route.input.instance.raw(),
        reported_type: None,
        before: None,
        settings: None,
        reason: None,
        configuration_changed: false,
    }
}

async fn query(bus: &Bus<'_>, route: &Route, query: InstanceQuery) -> Result<BusReply, Error> {
    bus.query(DaliFrame::Bits24(
        route
            .input
            .address
            .query_instance(route.input.instance, query),
    ))
    .await
}
fn byte(reply: BusReply) -> Option<u8> {
    if let BusReply::Value(v) = reply {
        Some(v)
    } else {
        None
    }
}
struct Probe {
    kind: Option<u8>,
    values: Option<Values>,
    outcome: &'static str,
}
async fn probe(bus: &Bus<'_>, route: &Route) -> Result<Probe, Error> {
    let kind = byte(query(bus, route, InstanceQuery::Type).await?);
    if kind != Some(route.input.kind) {
        return Ok(Probe {
            kind,
            values: None,
            outcome: if kind.is_some() {
                "type_mismatch"
            } else {
                "unavailable"
            },
        });
    }
    let enabled = match query(bus, route, InstanceQuery::Enabled).await? {
        BusReply::Value(255) => Some(true),
        BusReply::NoResponse => Some(false),
        _ => None,
    };
    let event_priority = byte(query(bus, route, InstanceQuery::EventPriority).await?);
    let event_scheme = byte(query(bus, route, InstanceQuery::EventScheme).await?);
    let mut groups = [None; 3];
    for (index, q) in [
        InstanceQuery::PrimaryGroup,
        InstanceQuery::Group1,
        InstanceQuery::Group2,
    ]
    .into_iter()
    .enumerate()
    {
        groups[index] = byte(query(bus, route, q).await?);
    }
    let common = frames::common_filter_mask(InstanceType::from_raw(route.input.kind));
    let filter_width = if common.is_some() { 8 } else { 24 };
    let mut filter_bytes = [None; 3];
    for (index, q) in [
        InstanceQuery::EventFilterLow,
        InstanceQuery::EventFilterMiddle,
        InstanceQuery::EventFilterHigh,
    ]
    .into_iter()
    .take(usize::from(filter_width / 8))
    .enumerate()
    {
        filter_bytes[index] = byte(query(bus, route, q).await?);
    }
    let event_filter = if filter_width == 8 {
        filter_bytes[0].map(u32::from)
    } else {
        filter_bytes[0]
            .zip(filter_bytes[1])
            .zip(filter_bytes[2])
            .map(|((l, m), h)| u32::from_le_bytes([l, m, h, 0]))
    };
    let button_timers = if route.input.kind == 1 {
        Some(button::probe(bus, route).await?)
    } else {
        None
    };
    let sensor_settings = if matches!(route.input.kind, 2..=4) {
        Some(sensor::probe(bus, route).await?)
    } else {
        None
    };
    let final_kind = byte(query(bus, route, InstanceQuery::Type).await?);
    if final_kind != kind {
        return Ok(Probe {
            kind: final_kind,
            values: None,
            outcome: if final_kind.is_some() {
                "type_mismatch"
            } else {
                "unavailable"
            },
        });
    }
    let complete = enabled.is_some()
        && event_priority.is_some_and(|v| (2..=5).contains(&v))
        && event_scheme.is_some_and(|v| v <= 4)
        && groups
            .iter()
            .all(|v| v.is_some_and(|v| v <= 31 || v == 255))
        && event_filter.is_some_and(|v| common.is_none_or(|mask| v & !mask == 0));
    Ok(Probe {
        kind,
        values: Some(Values {
            enabled,
            event_priority,
            event_scheme,
            event_filter,
            filter_bytes,
            filter_width,
            groups,
            button_timers,
            sensor_settings,
            quality: if complete { "complete" } else { "partial" },
        }),
        outcome: if complete { "current" } else { "unavailable" },
    })
}

pub(in crate::service) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = Bus {
        link: &link,
        bus: route.input.bus,
        space: Space::Inputs,
        stamp,
        deadline,
    };
    let mut report = report(&target, &route);
    let mut attempted = false;
    let result: Result<(), Error> = async {
        let first = probe(&bus, &route).await?;
        report.reported_type = first.kind;
        report.outcome = first.outcome;
        report.settings = first.values;
        if !route.action.writing() || report.outcome != "current" {
            return Ok(());
        }
        report.before = report.settings.take();
        report.outcome = "unconfirmed";
        if let Action::Configure { settings, .. } = &route.action {
            if let Some(sensor) = &settings.sensor_settings {
                sensor
                    .preflight(
                        report
                            .before
                            .as_ref()
                            .and_then(|v| v.sensor_settings.as_ref()),
                    )
                    .map_err(Error::Evidence)?;
            }
            if let Some(timers) = &settings.button_timers {
                timers
                    .preflight(
                        report
                            .before
                            .as_ref()
                            .and_then(|v| v.button_timers.as_ref()),
                    )
                    .map_err(Error::Evidence)?;
            }
        }
        for setting in &route.plans {
            for step in frames::plan(route.input.address, route.input.instance, *setting) {
                attempted = true;
                bus.command(DaliFrame::Bits24(step.frame), step.repeat)
                    .await?;
            }
        }
        for value in &route.button_plans {
            for step in value.plan(route.input.address, route.input.instance) {
                attempted = true;
                bus.command(DaliFrame::Bits24(step.frame), step.repeat)
                    .await?;
            }
        }
        for value in &route.sensor_plans {
            for step in value.plan(route.input.address, route.input.instance) {
                attempted = true;
                bus.command(DaliFrame::Bits24(step.frame), step.repeat)
                    .await?;
            }
        }
        let after = probe(&bus, &route).await?;
        report.reported_type = after.kind;
        report.settings = after.values;
        if let Action::Configure { settings, .. } = &route.action {
            if after.outcome == "current"
                && report
                    .settings
                    .as_ref()
                    .is_some_and(|v| settings.matches(v))
            {
                report.outcome = "configured";
            } else {
                report.reason =
                    Some("input settings were not confirmed by complete matching readback".into());
            }
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
        report.settings = None;
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
    completed: &mut Completion,
    hat: &crate::service::hat::Hat,
) -> Result<Report, ServiceError> {
    if let Some(error) = completed.error.take() {
        return Err(error);
    }
    let observation = completed
        .observation
        .take()
        .ok_or_else(|| reject("input settings produced no final evidence"))?;
    let q = &completed.request;
    let route = resolve(snapshot, &q.target, &q.action, &q.context)?;
    if snapshot.phase != Phase::Ready
        || Instant::now() >= q.deadline
        || observation.stale
        || !hat.evidence_current(route.input.bus, observation.stamp)
        || route.input.context.generation() != Some(observation.report.target_generation.as_str())
    {
        return Err(ServiceError::Uncertain(
            "input settings evidence changed before acceptance".into(),
        ));
    }
    Ok(*observation.report)
}
pub(in crate::service) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let result = match result {
        Err(error) if request.started => {
            let mut report = *request.report;
            report.outcome = "unconfirmed";
            report.reason = Some(error.to_string());
            report.settings = None;
            Ok(report)
        }
        result => result,
    };
    let reply = request.reply;
    drop(request._permit);
    let _ = reply.send(result);
}

impl ServiceHandle {
    /// Read hardware event settings without generating actions or replacing values.
    pub async fn read_input_settings(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.input_settings(target, Action::Read, context, deadline, None)
            .await
    }
    /// Patch native settings; saved identity/configuration is unchanged.
    pub async fn configure_input(
        &self,
        target: &str,
        expected: u64,
        settings: Patch,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.input_settings(
            target,
            Action::Configure { expected, settings },
            context,
            deadline,
            None,
        )
        .await
    }
    pub(in crate::service) async fn input_settings(
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
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &action,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        let writing = action.writing();
        self.send
            .try_send(Message::InputSettings(Request {
                target: target.into(),
                action,
                context: route.input.context.clone(),
                deadline,
                reply,
                started: false,
                report: Box::new(report(target, &route)),
                _completion: completion,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(if writing {
                ServiceError::Uncertain(
                    "input settings caller deadline; inspect hardware before retrying".into(),
                )
            } else {
                ServiceError::Rejected("input settings caller deadline".into())
            });
        }
        result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?
    }
}
