//! Explicit DT1 requests under the existing physical owner; a test never holds it.
use super::*;
use crate::{
    atx::{
        connection::Connection,
        transactions::{TransactionError, Transactions},
        wire::DaliFrame,
    },
    dali::{
        emergency as native,
        gear::{Destination, Query as GearQuery},
    },
    service::commissioning::{Bus, Error},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub emergency_level: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub prolong_seconds: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub test_execution_timeout_minutes: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_test_interval_days: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub duration_test_interval_weeks: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_delay_minutes: Option<u32>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub duration_delay_minutes: Option<u32>,
}
fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn reject(e: impl ToString) -> ServiceError {
    ServiceError::Rejected(e.to_string())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub raw: Option<u32>,
    pub status: &'static str,
    pub value: Option<u32>,
    pub unit: &'static str,
    pub at_least: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub fields: BTreeMap<&'static str, Field>,
    pub mode: BTreeMap<&'static str, Option<bool>>,
    pub features: BTreeMap<&'static str, Option<bool>>,
    pub failures: BTreeMap<&'static str, Option<bool>>,
    pub status: BTreeMap<&'static str, Option<bool>>,
    pub quality: &'static str,
}
impl Values {
    fn value(&self, name: &str) -> Option<u32> {
        self.fields
            .get(name)
            .filter(|f| f.status == "value")
            .and_then(|f| f.value)
    }
}
type Readings = BTreeMap<String, scenes::Reading>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub revision: u64,
    pub bus: u8,
    pub address: u8,
    pub support: Option<bool>,
    pub before: Option<Values>,
    pub observed: Option<Values>,
    pub before_readings: Option<Readings>,
    pub readings: Option<Readings>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
#[derive(Debug, Clone)]
pub(super) enum Action {
    Read,
    Configure { expected: u64, settings: Patch },
    Start { expected: u64, test: native::Test },
    Stop { expected: u64 },
}
impl Action {
    pub(super) fn writing(&self) -> bool {
        !matches!(self, Self::Read)
    }
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Read => "read_emergency",
            Self::Configure { .. } => "configure_emergency",
            Self::Start { .. } => "start_emergency_test",
            Self::Stop { .. } => "stop_emergency_test",
        }
    }
    fn expected(&self) -> Option<u64> {
        match self {
            Self::Read => None,
            Self::Configure { expected, .. }
            | Self::Start { expected, .. }
            | Self::Stop { expected } => Some(*expected),
        }
    }
}
#[derive(Clone)]
pub(super) struct Route {
    pub output: output::Route,
    pub action: Action,
    pub revision: u64,
}
pub(super) struct Request {
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
pub(super) struct Observation {
    pub report: Box<Report>,
    pub stamp: output::Stamp,
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
pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: &Action,
    context: &RequestContext,
) -> Result<Route, ServiceError> {
    let output = output::resolve(snapshot, target, context, action.writing())?;
    let revision = snapshot.model.inventory().revision;
    if action.expected().is_some_and(|v| v != revision) {
        return Err(reject("original emergency inventory revision required"));
    }
    if let Action::Configure { settings, .. } = action {
        settings.validate()?;
    }
    Ok(Route {
        output,
        action: action.clone(),
        revision,
    })
}
fn report(target: &str, r: &Route) -> Report {
    Report {
        operation: r.action.name(),
        outcome: "unavailable",
        target: target.into(),
        target_generation: r
            .output
            .context
            .generation()
            .expect("resolved token")
            .into(),
        revision: r.revision,
        bus: r.output.bus,
        address: r.output.address.raw(),
        support: None,
        before: None,
        observed: None,
        before_readings: None,
        readings: None,
        reason: None,
        configuration_changed: false,
    }
}
impl Patch {
    fn entries(&self) -> Vec<(&'static str, native::SettingKey, u32)> {
        use native::SettingKey as K;
        [
            ("emergency_level", K::EmergencyLevel, self.emergency_level),
            ("prolong_seconds", K::ProlongSeconds, self.prolong_seconds),
            (
                "test_execution_timeout_minutes",
                K::ExecutionTimeoutMinutes,
                self.test_execution_timeout_minutes,
            ),
            (
                "function_test_interval_days",
                K::FunctionIntervalDays,
                self.function_test_interval_days,
            ),
            (
                "duration_test_interval_weeks",
                K::DurationIntervalWeeks,
                self.duration_test_interval_weeks,
            ),
            (
                "next_function_test_minutes",
                K::InitialDelayMinutes,
                self.function_delay_minutes,
            ),
            (
                "next_duration_test_minutes",
                K::InitialDelayMinutes,
                self.duration_delay_minutes,
            ),
        ]
        .into_iter()
        .filter_map(|(name, key, value)| value.map(|v| (name, key, v)))
        .collect()
    }
    pub(super) fn validate(&self) -> Result<(), ServiceError> {
        if self == &Self::default() {
            return Err(reject("change at least one emergency setting"));
        }
        for (_, key, value) in self.entries() {
            native::Setting::new(key, value).map_err(reject)?;
        }
        for (interval, delay) in [
            (
                self.function_test_interval_days,
                self.function_delay_minutes,
            ),
            (
                self.duration_test_interval_weeks,
                self.duration_delay_minutes,
            ),
        ] {
            if interval.is_some_and(|v| v > 0) != delay.is_some() {
                return Err(reject("a positive interval requires its explicit initial delay; delay-only/disabled pairs are invalid"));
            }
        }
        Ok(())
    }
    fn plans(
        &self,
        before: &Values,
        destination: Destination,
    ) -> Result<Vec<crate::dali::gear::settings::Step>, ServiceError> {
        self.validate()?;
        let features = before
            .value("features_raw")
            .ok_or_else(|| reject("fresh emergency features required"))?;
        for (name, _, _) in self.entries() {
            if before.value(name).is_none() && !name.starts_with("next_") {
                return Err(reject("fresh requested emergency settings required"));
            }
        }
        if let Some(level) = self.emergency_level {
            let (min, max) = (before.value("minimum_level"), before.value("maximum_level"));
            if features & 16 == 0
                || !min
                    .zip(max)
                    .is_some_and(|(min, max)| min <= max && (min..=max).contains(&level))
            {
                return Err(reject(
                    "adjustable emergency level and fresh ordered manufacturer limits required",
                ));
            }
        }
        if (self.function_test_interval_days.is_some()
            || self.duration_test_interval_weeks.is_some())
            && features & 8 == 0
        {
            return Err(reject("automatic emergency test capability required"));
        }
        let mut plans = Vec::new();
        for (_, key, value) in self.entries().into_iter().filter(|(_, key, _)| {
            !matches!(
                key,
                native::SettingKey::InitialDelayMinutes
                    | native::SettingKey::FunctionIntervalDays
                    | native::SettingKey::DurationIntervalWeeks
            )
        }) {
            plans.extend(
                native::Setting::new(key, value)
                    .map_err(reject)?
                    .plan(destination),
            );
        }
        for (interval, delay, key) in [
            (
                self.function_test_interval_days,
                self.function_delay_minutes,
                native::SettingKey::FunctionIntervalDays,
            ),
            (
                self.duration_test_interval_weeks,
                self.duration_delay_minutes,
                native::SettingKey::DurationIntervalWeeks,
            ),
        ] {
            if let Some(interval) = interval {
                if let Some(delay) = delay {
                    plans.extend(
                        native::Setting::new(native::SettingKey::InitialDelayMinutes, delay)
                            .map_err(reject)?
                            .plan(destination),
                    );
                }
                plans.extend(
                    native::Setting::new(key, interval)
                        .map_err(reject)?
                        .plan(destination),
                );
            }
        }
        Ok(plans)
    }
    fn matches(&self, values: &Values) -> bool {
        self.entries()
            .iter()
            .all(|(name, _, value)| values.value(name) == Some(*value))
    }
}

fn raw(reading: &scenes::Reading) -> Option<u8> {
    if let scenes::Reading::Value { raw } = reading {
        u8::try_from(*raw).ok()
    } else {
        None
    }
}
fn field(query: native::Query, reading: Option<&scenes::Reading>) -> Field {
    let raw = reading.and_then(raw);
    let (status, value) = match raw.map(|value| query.decode(value)) {
        Some(native::Value::Known(value)) => ("value", Some(value)),
        Some(native::Value::Bitmap(value)) => ("value", Some(u32::from(value))),
        Some(native::Value::Unknown) => ("unknown", None),
        Some(native::Value::Invalid) => ("invalid", None),
        None => ("unavailable", None),
    };
    Field {
        raw: raw.map(u32::from),
        status,
        value,
        unit: query.unit(),
        at_least: raw.is_some_and(|v| query.saturated(v)),
    }
}
fn bits(
    raw: Option<u32>,
    names: [Option<&'static str>; 8],
) -> BTreeMap<&'static str, Option<bool>> {
    names
        .into_iter()
        .enumerate()
        .filter_map(|(bit, name)| name.map(|name| (name, raw.map(|value| value & (1 << bit) != 0))))
        .collect()
}
const QUERIES: [(&str, native::Query); 13] = [
    ("battery_charge", native::Query::Battery),
    ("last_duration_seconds", native::Query::DurationResult),
    ("emergency_runtime_hours", native::Query::EmergencyHours),
    ("total_runtime_hours", native::Query::TotalHours),
    ("emergency_level", native::Query::Level),
    ("minimum_level", native::Query::Minimum),
    ("maximum_level", native::Query::Maximum),
    ("rated_duration_seconds", native::Query::RatedDuration),
    ("mode_raw", native::Query::Mode),
    ("features_raw", native::Query::Features),
    ("failures_raw", native::Query::Failures),
    ("status_raw", native::Query::Status),
    ("extended_version", native::Query::ExtendedVersion),
];
async fn query(bus: &Bus<'_>, r: &Route, query: native::Query) -> Result<scenes::Reading, Error> {
    let plan = query
        .plan(r.output.address)
        .expect("checked timing selector");
    for step in &plan[..plan.len() - 1] {
        bus.command(DaliFrame::Bits16(step.frame), step.repeat)
            .await?;
    }
    Ok(scenes::Reading::byte(
        bus.query(DaliFrame::Bits16(plan.last().unwrap().frame))
            .await?,
        false,
    ))
}
struct Probe {
    support: Option<bool>,
    values: Option<Values>,
    readings: Readings,
}
async fn probe(bus: &Bus<'_>, r: &Route) -> Result<Probe, Error> {
    let mut readings = BTreeMap::new();
    let presence = scenes::Reading::byte(
        bus.query(DaliFrame::Bits16(r.output.address.query(GearQuery::Status)))
            .await?,
        false,
    );
    let present = raw(&presence).is_some();
    readings.insert("presence".into(), presence);
    let first = scenes::Reading::byte(
        bus.query(DaliFrame::Bits16(
            r.output.address.query(GearQuery::DeviceType),
        ))
        .await?,
        false,
    );
    let mut evidence = vec![raw(&first)];
    readings.insert("device_type".into(), first);
    if evidence[0] == Some(255) {
        let mut previous = None;
        for index in 0..9 {
            let next = scenes::Reading::byte(
                bus.query(DaliFrame::Bits16(
                    r.output.address.query(GearQuery::NextDeviceType),
                ))
                .await?,
                false,
            );
            let value = raw(&next);
            evidence.push(value);
            readings.insert(format!("next_device_type_{index}"), next);
            match value {
                Some(value) if value < 254 && previous.is_none_or(|old| value > old) => {
                    if value >= 1 {
                        break;
                    }
                    previous = Some(value);
                }
                _ => break,
            }
        }
    }
    let support = native::supports(&evidence, 1);
    if !present || support != Some(true) {
        return Ok(Probe {
            support,
            values: None,
            readings,
        });
    }
    let mut fields = BTreeMap::new();
    for (name, kind) in QUERIES {
        let reading = query(bus, r, kind).await?;
        fields.insert(name, field(kind, Some(&reading)));
        readings.insert(name.into(), reading);
    }
    for (name, selector) in [
        ("function_test_interval_days", 4),
        ("duration_test_interval_weeks", 5),
        ("test_execution_timeout_minutes", 6),
        ("prolong_seconds", 7),
    ] {
        let kind = native::Query::Timing(selector);
        let reading = query(bus, r, kind).await?;
        fields.insert(name, field(kind, Some(&reading)));
        readings.insert(name.into(), reading);
    }
    for (name, selector) in [
        ("next_function_test_minutes", 0),
        ("next_duration_test_minutes", 2),
    ] {
        let high = query(bus, r, native::Query::Timing(selector)).await?;
        // F2 latches the corresponding low byte into DTR1 for this high read.
        let low = scenes::Reading::byte(
            bus.query(DaliFrame::Bits16(
                Destination::Short(r.output.address).command(0x9c),
            ))
            .await?,
            false,
        );
        let word = raw(&high)
            .zip(raw(&low))
            .map(|(hi, lo)| u16::from_be_bytes([hi, lo]));
        fields.insert(
            name,
            Field {
                raw: word.map(u32::from),
                status: match word {
                    Some(65535) => "unknown",
                    Some(_) => "value",
                    None => "unavailable",
                },
                value: word.filter(|v| *v != 65535).map(|v| u32::from(v) * 15),
                unit: "minutes",
                at_least: false,
            },
        );
        readings.insert(format!("{name}_high"), high);
        readings.insert(format!("{name}_low"), low);
    }
    let closing = scenes::Reading::byte(
        bus.query(DaliFrame::Bits16(r.output.address.query(GearQuery::Status)))
            .await?,
        false,
    );
    let closing_present = raw(&closing).is_some();
    readings.insert("closing_presence".into(), closing);
    let val = |key| fields.get(key).and_then(|f| f.value);
    let mode = bits(
        val("mode_raw"),
        [
            Some("rest"),
            Some("normal"),
            Some("emergency"),
            Some("extended_emergency"),
            Some("function_test"),
            Some("duration_test"),
            Some("hardwired_inhibit"),
            Some("hardwired_switch_on"),
        ],
    );
    let features = bits(
        val("features_raw"),
        [
            Some("integral"),
            Some("maintained"),
            Some("switched_maintained"),
            Some("automatic_tests"),
            Some("adjustable_level"),
            Some("hardwired_inhibit"),
            Some("physical_selection"),
            Some("relight_in_rest"),
        ],
    );
    let failures = bits(
        val("failures_raw"),
        [
            Some("circuit_failure"),
            Some("battery_duration_failure"),
            Some("battery_failure"),
            Some("emergency_lamp_failure"),
            Some("function_max_delay_exceeded"),
            Some("duration_max_delay_exceeded"),
            Some("function_test_failed"),
            Some("duration_test_failed"),
        ],
    );
    let status = bits(
        val("status_raw"),
        [
            Some("inhibited"),
            Some("function_done_valid"),
            Some("duration_done_valid"),
            None,
            Some("function_pending"),
            Some("duration_pending"),
            Some("identification_active"),
            Some("physically_selected"),
        ],
    );
    let quality = if readings.values().all(|reading| raw(reading).is_some()) {
        "complete"
    } else {
        "partial"
    };
    Ok(Probe {
        support,
        values: closing_present.then_some(Values {
            fields,
            mode,
            features,
            failures,
            status,
            quality,
        }),
        readings,
    })
}
pub(super) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = Bus {
        link: &link,
        bus: route.output.bus,
        space: discovery::Space::Outputs,
        stamp,
        deadline,
    };
    let mut report = report(&target, &route);
    let mut attempted = false;
    let result:Result<(),Error>=async{
        let first=probe(&bus,&route).await?;report.support=first.support;report.observed=first.values;report.readings=Some(first.readings);report.outcome="observed";
        if !route.action.writing(){return Ok(());}
        let plans=(||{
            if report.support!=Some(true){return Err(reject("fresh DT1 emergency support required"));}
            let before=report.observed.as_ref().ok_or_else(||reject("fresh emergency fixture presence required"))?;
            if before.value("features_raw").is_none()||before.value("status_raw").is_none()||before.value("mode_raw").is_none(){return Err(reject("fresh emergency features, mode and status required"));}
            Ok(match &route.action{
                Action::Configure{settings,..}=>settings.plans(before,Destination::Short(route.output.address))?,
                Action::Start{test,..}=>native::control(Some(*test),route.output.address),
                Action::Stop{..}=>native::control(None,route.output.address),Action::Read=>unreachable!(),
            })
        })();
        let plans=match plans{Ok(plans)=>plans,Err(error)=>{report.outcome="rejected";report.reason=Some(error.to_string());return Ok(());}};
        report.before=report.observed.take();report.before_readings=report.readings.take();report.outcome="unconfirmed";
        for step in plans{attempted=true;bus.command(DaliFrame::Bits16(step.frame),step.repeat).await?;}
        let after=probe(&bus,&route).await?;report.support=after.support;report.observed=after.values;report.readings=Some(after.readings);
        if report.support==Some(true){
            if let Some(values)=&report.observed{
                match &route.action{
                    Action::Configure{settings,..} if settings.matches(values)=>report.outcome="applied",
                    Action::Start{..}|Action::Stop{..} if values.value("mode_raw").is_some()&&values.value("status_raw").is_some()=>report.outcome="request_sent",
                    _=>{},
                }
            }
        }
        if report.outcome=="unconfirmed"{report.reason=Some("native request/settings were sent; fresh readback could not confirm the requested observation".into());}
        Ok(())
    }.await;
    let recovery = matches!(
        &result,
        Err(Error::Transaction(TransactionError::StreamLoss))
    );
    let mut error = None;
    if let Err(failure) = result {
        let reason = match failure {
            Error::Transaction(error) => error.to_string(),
            Error::Evidence(reason) => reason.into(),
        };
        report.reason = Some(reason.clone());
        report.observed = None;
        report.readings = None;
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
pub(super) fn accept(
    snapshot: &Snapshot,
    completed: &mut Completion,
    hat: &hat::Hat,
) -> Result<Report, ServiceError> {
    if let Some(error) = completed.error.take() {
        return Err(error);
    }
    let observed = completed
        .observation
        .take()
        .ok_or_else(|| reject("emergency work produced no final evidence"))?;
    let q = &completed.request;
    let r = resolve(snapshot, &q.target, &q.action, &q.context)?;
    if Instant::now() >= q.deadline
        || observed.stale
        || !hat.evidence_current(r.output.bus, observed.stamp)
        || r.revision != q.report.revision
        || r.revision != observed.report.revision
        || r.output.context.generation() != Some(observed.report.target_generation.as_str())
    {
        return Err(reject("emergency evidence changed before acceptance"));
    }
    Ok(*observed.report)
}
pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let result = match result {
        Err(error) if request.started => {
            let mut report = *request.report;
            report.outcome = "unconfirmed";
            report.reason = Some(error.to_string());
            Ok(report)
        }
        result => result,
    };
    drop(request._permit);
    let _ = request.reply.send(result);
}
impl ServiceHandle {
    pub async fn read_emergency(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.emergency(target, Action::Read, context, deadline, None, None)
            .await
    }
    pub async fn configure_emergency(
        &self,
        target: &str,
        expected: u64,
        settings: Patch,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.emergency(
            target,
            Action::Configure { expected, settings },
            context,
            deadline,
            None,
            None,
        )
        .await
    }
    pub async fn start_emergency_test(
        &self,
        target: &str,
        expected: u64,
        test: native::Test,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.emergency(
            target,
            Action::Start { expected, test },
            context,
            deadline,
            None,
            None,
        )
        .await
    }
    pub async fn stop_emergency_test(
        &self,
        target: &str,
        expected: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.emergency(
            target,
            Action::Stop { expected },
            context,
            deadline,
            None,
            None,
        )
        .await
    }
    pub(super) async fn emergency(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        admitted_revision: Option<u64>,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        if action.writing() && identification::blocks(self, target) {
            return Err(reject("stop identification before emergency operations"));
        }
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
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &action,
            &context,
        )?;
        if admitted_revision.is_some_and(|revision| revision != route.revision) {
            return Err(reject(
                "original emergency revision changed before service admission",
            ));
        }
        let revision = route.revision;
        let original = route.output.context.clone();
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Emergency(Request {
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
        let result = timeout_at(deadline, receive)
            .await
            .map_err(|_| {
                if action.writing() {
                    ServiceError::Uncertain(
                        "emergency caller deadline; inspect native status before retrying".into(),
                    )
                } else {
                    ServiceError::Expired
                }
            })?
            .map_err(|_| ServiceError::Closed)??;
        let current = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &action,
            &original,
        );
        if Instant::now() >= deadline || !current.is_ok_and(|r| r.revision == revision) {
            if action.writing() {
                let mut result = result;
                result.outcome = "unconfirmed";
                result.reason = Some("emergency mapping changed before reply".into());
                result.observed = None;
                result.readings = None;
                return Ok(result);
            }
            return Err(reject("emergency mapping changed before reply"));
        }
        Ok(result)
    }
}
