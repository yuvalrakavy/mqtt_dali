//! Explicit common fixture configuration, separate from current output State.
use crate::dali::gear::settings as frames;
use crate::service::{
    identification, output, Message, Phase, RequestContext, ServiceError, ServiceHandle, Snapshot,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit},
    time::{timeout_at, Instant},
};
mod execute;
mod switching;
pub(in crate::service) use execute::run as execute;
pub use switching::{Patch as SwitchingPatch, Values as SwitchingValues};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub minimum_level: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub maximum_level: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub power_on_level: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub system_failure_level: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub fade_time: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub fade_rate: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub switching: Option<SwitchingPatch>,
}
fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub physical_minimum: Option<u8>,
    pub minimum_level: Option<u8>,
    pub maximum_level: Option<u8>,
    pub power_on_level: Option<u8>,
    pub system_failure_level: Option<u8>,
    pub fade_raw: Option<u8>,
    pub fade_time: Option<u8>,
    pub fade_rate: Option<u8>,
    pub switching_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switching: Option<SwitchingValues>,
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
    pub before: Option<Values>,
    pub settings: Option<Values>,
    pub readings: Option<std::collections::BTreeMap<&'static str, crate::service::scenes::Reading>>,
    pub status_flags: Option<StatusFlags>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusFlags {
    pub gear_failure: bool,
    pub lamp_failure: bool,
    pub lamp_on: bool,
    pub limit_error: bool,
    pub fade_running: bool,
    pub reset_state: bool,
    pub missing_short_address: bool,
    pub power_failure: bool,
}
impl Patch {
    fn validate(&self) -> Result<(), ServiceError> {
        if self == &Self::default() {
            return Err(reject("change at least one output setting"));
        }
        if let Some(patch) = &self.switching {
            patch.validate()?;
        }
        for raw in [self.minimum_level, self.maximum_level]
            .into_iter()
            .flatten()
        {
            frames::LimitLevel::new(raw).map_err(reject)?;
        }
        if let Some(raw) = self.fade_time {
            frames::FadeTime::new(raw).map_err(reject)?;
        }
        if let Some(raw) = self.fade_rate {
            frames::FadeRate::new(raw).map_err(reject)?;
        }
        if self
            .minimum_level
            .zip(self.maximum_level)
            .is_some_and(|(min, max)| min > max)
        {
            return Err(reject("minimum level exceeds maximum level"));
        }
        Ok(())
    }
    fn plans(
        &self,
        before: &Values,
    ) -> Result<
        (
            Vec<frames::Setting>,
            Vec<crate::dali::gear::switching::Setting>,
        ),
        ServiceError,
    > {
        use frames::{FadeRate, FadeTime, LimitLevel, PowerOnLevel, Setting, SystemFailureLevel};
        self.validate()?;
        let mut plans = Vec::new();
        if self.minimum_level.is_some() || self.maximum_level.is_some() {
            let (Some(physical), Some(old_min), Some(old_max)) = (
                before.physical_minimum,
                before.minimum_level,
                before.maximum_level,
            ) else {
                return Err(reject("fresh physical minimum and current limits required"));
            };
            let physical = before.limit_floor().unwrap_or(physical);
            let min = self.minimum_level.unwrap_or(old_min);
            let max = self.maximum_level.unwrap_or(old_max);
            if physical > old_min || old_min > old_max || min < physical || min > max {
                return Err(reject(
                    "requested limits are outside fresh physical/current limits",
                ));
            }
            let low = self
                .minimum_level
                .map(|v| Setting::Minimum(LimitLevel::new(v).unwrap()));
            let high = self
                .maximum_level
                .map(|v| Setting::Maximum(LimitLevel::new(v).unwrap()));
            if min > old_max {
                plans.extend(high);
                plans.extend(low);
            } else {
                plans.extend(low);
                plans.extend(high);
            }
        }
        if let Some(raw) = self.power_on_level {
            if before.power_on_level.is_none() {
                return Err(reject("fresh power-on setting required"));
            }
            plans.push(Setting::PowerOn(PowerOnLevel::from_raw(raw)));
        }
        if let Some(raw) = self.system_failure_level {
            if before.system_failure_level.is_none() {
                return Err(reject("fresh bus-failure setting required"));
            }
            plans.push(Setting::SystemFailure(SystemFailureLevel::from_raw(raw)));
        }
        if let Some(raw) = self.fade_time {
            if before.fade_raw.is_none() {
                return Err(reject("fresh packed fade setting required"));
            }
            plans.push(Setting::FadeTime(FadeTime::new(raw).unwrap()));
        }
        if let Some(raw) = self.fade_rate {
            if before.fade_raw.is_none() {
                return Err(reject("fresh packed fade setting required"));
            }
            plans.push(Setting::FadeRate(FadeRate::new(raw).unwrap()));
        }
        let switching = self
            .switching
            .as_ref()
            .map(|p| p.plans(before))
            .transpose()?
            .unwrap_or_default();
        Ok((plans, switching))
    }
    fn matches(&self, v: &Values) -> bool {
        self.minimum_level
            .is_none_or(|x| v.minimum_level == Some(x))
            && self
                .maximum_level
                .is_none_or(|x| v.maximum_level == Some(x))
            && self
                .power_on_level
                .is_none_or(|x| v.power_on_level == Some(x))
            && self
                .system_failure_level
                .is_none_or(|x| v.system_failure_level == Some(x))
            && self.fade_time.is_none_or(|x| v.fade_time == Some(x))
            && self.fade_rate.is_none_or(|x| v.fade_rate == Some(x))
            && self.switching.as_ref().is_none_or(|p| p.matches(v))
    }
}
impl Values {
    fn limit_floor(&self) -> Option<u8> {
        self.physical_minimum?;
        if self.switching_support == Some(true)
            && self
                .switching
                .as_ref()
                .is_some_and(|s| s.adjustable_thresholds == Some(true))
        {
            Some(1)
        } else {
            self.physical_minimum
        }
    }
}
fn reject(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Rejected(e.to_string())
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
            "configure_output"
        } else {
            "read_output_settings"
        }
    }
}
#[derive(Clone)]
pub(in crate::service) struct Route {
    pub output: output::Route,
    pub action: Action,
    pub revision: u64,
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
    pub stamp: output::Stamp,
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
    let output = output::resolve(snapshot, target, context, action.writing())?;
    if let Action::Configure { expected, settings } = action {
        if *expected != snapshot.model.inventory().revision {
            return Err(reject("original output inventory revision required"));
        }
        settings.validate()?;
    }
    Ok(Route {
        output,
        action: action.clone(),
        revision: snapshot.model.inventory().revision,
    })
}
fn report(target: &str, r: &Route) -> Report {
    Report {
        operation: r.action.name(),
        outcome: "unavailable",
        target: target.into(),
        target_generation: r.output.context.generation().unwrap().into(),
        revision: r.revision,
        bus: r.output.bus,
        address: r.output.address.raw(),
        before: None,
        settings: None,
        readings: None,
        status_flags: None,
        reason: None,
        configuration_changed: false,
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
        .ok_or_else(|| reject("output settings produced no final evidence"))?;
    let q = &c.request;
    let r = resolve(snapshot, &q.target, &q.action, &q.context)?;
    if snapshot.phase != Phase::Ready
        || Instant::now() >= q.deadline
        || o.stale
        || !hat.evidence_current(r.output.bus, o.stamp)
        || r.output.context.generation() != Some(o.report.target_generation.as_str())
        || r.revision != q.report.revision
        || r.revision != o.report.revision
    {
        return Err(ServiceError::Uncertain(
            "output settings evidence changed before acceptance".into(),
        ));
    }
    Ok(*o.report)
}
pub(in crate::service) fn finish(q: Request, result: Result<Report, ServiceError>) {
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
    /// Read native common settings and diagnostics without changing output State.
    pub async fn read_output_settings(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.output_settings(target, Action::Read, context, deadline, None)
            .await
    }
    /// Patch native common settings and require actual requested readback.
    pub async fn configure_output(
        &self,
        target: &str,
        expected: u64,
        settings: Patch,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.output_settings(
            target,
            Action::Configure { expected, settings },
            context,
            deadline,
            None,
        )
        .await
    }
    pub(in crate::service) async fn output_settings(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        let writing = action.writing();
        if writing && identification::blocks(self, target) {
            return Err(reject(
                "stop identification before configuring this fixture",
            ));
        }
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
        let revision = route.revision;
        let original = route.output.context.clone();
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::OutputSettings(Request {
                target: target.into(),
                action: action.clone(),
                context: original.clone(),
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
                    "output settings caller deadline; inspect hardware before retrying".into(),
                )
            } else {
                reject("output settings caller deadline")
            });
        }
        let result = result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)??;
        let latest = self.snapshot().ok_or(ServiceError::Closed)?;
        let current = resolve(&latest, target, &action, &original);
        if !current.as_ref().is_ok_and(|r| r.revision == revision) {
            if writing {
                let mut result = result;
                result.outcome = "unconfirmed";
                result.reason = Some("output settings mapping changed before reply".into());
                result.settings = None;
                result.readings = None;
                result.status_flags = None;
                return Ok(result);
            }
            return Err(reject("output settings mapping changed before reply"));
        }
        Ok(result)
    }
}
