//! Native group power/level with individual-address evidence under one owner.
use super::*;
use crate::{
    atx::{
        connection::Connection,
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
mod color;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Values {
    pub quality: &'static str,
    pub power_state: Option<bool>,
    pub brightness: Option<f64>,
    pub power_mixed: Option<bool>,
    pub brightness_mixed: Option<bool>,
    pub known_members: usize,
    pub unresolved: Vec<u8>,
}
/// A native group color change, optionally combined with power or brightness.
#[derive(Debug, Clone, Copy)]
pub struct ColorIntent {
    pub color: crate::dali::colour::Intent,
    pub base: Option<Intent>,
}
impl From<ColorIntent> for output::WriteIntent {
    fn from(intent: ColorIntent) -> Self {
        match intent.base {
            Some(base) => Self::Combined(base, intent.color),
            None => Self::Colour(intent.color),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    ReadColor,
    Set {
        expected: u64,
        intent: Intent,
    },
    Color {
        expected: u64,
        intent: output::WriteIntent,
    },
}
impl Action {
    pub(super) fn writing(&self) -> bool {
        matches!(self, Self::Set { .. } | Self::Color { .. })
    }
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Read => "read_group_output",
            Self::ReadColor => "read_group_color",
            Self::Set { .. } => "set_group_output",
            Self::Color { .. } => "set_group_color",
        }
    }
    pub(super) fn from_intent(expected: u64, intent: output::WriteIntent) -> Self {
        match intent {
            output::WriteIntent::Base(intent) => Self::Set { expected, intent },
            intent => Self::Color { expected, intent },
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
    representation: Option<crate::config::ColorRepresentation>,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Reply, ServiceError>>,
    pub started: bool,
    pub bus: u8,
    report: Box<Report>,
    _completion: Option<mqtt_bridge_kit::CommandCompletion>,
    _permit: OwnedSemaphorePermit,
}
/// One owner-accepted group capture, retaining its original mapping and evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    report: Arc<Report>,
    pub context: RequestContext,
    stamp: output::Stamp,
    pub stale: bool,
    colors: Option<std::collections::BTreeMap<u8, output::colour::Observation>>,
}
impl Observation {
    /// Original sampled report. Check staleness and current lifecycle before use.
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn color_observations(
        &self,
    ) -> Option<&std::collections::BTreeMap<u8, output::colour::Observation>> {
        self.colors.as_ref()
    }
}
pub(super) enum Reply {
    Observed(Observation),
    Unconfirmed(Box<Report>),
}
impl Reply {
    fn into_report(self) -> Report {
        match self {
            Self::Observed(o) => o.report().clone(),
            Self::Unconfirmed(r) => *r,
        }
    }
}

pub(super) fn retained(
    model: &Model,
    mut observations: std::collections::BTreeMap<String, Observation>,
) -> std::collections::BTreeMap<String, Observation> {
    let table = routing::generations(model);
    observations.retain(|target, o| {
        o.stale |= o.report.revision != model.inventory().revision;
        table.targets.get(target).map(String::as_str) == o.context.generation()
            && model
                .inventory()
                .buses
                .iter()
                .any(|b| b.bus == o.report.bus && b.enabled)
    });
    observations
}

/// Called before polling the HAT (and after physical progress). No new helper
/// can start a mutation while a cached group still claims current evidence.
pub(super) fn invalidate(snapshot: &mut Snapshot, hat: &hat::Hat) -> bool {
    let changing = hat.group_output_mutation_bus();
    let mut changed = false;
    for observation in snapshot.group_outputs.values_mut() {
        if !observation.stale
            && (changing == Some(observation.report.bus)
                || observation.report.revision != snapshot.model.inventory().revision
                || !hat.cached_evidence_current(observation.report.bus, observation.stamp))
        {
            observation.stale = true;
            changed = true;
        }
    }
    changed
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
    if let Action::Set { expected, .. } | Action::Color { expected, .. } = action {
        if cause.generation().is_none() || *expected != snapshot.model.inventory().revision {
            return Err(reject(
                "original group generation and inventory revision required",
            ));
        }
    }
    if let Action::Color { intent, .. } = action {
        use crate::{config::ColorRepresentation as Representation, dali::colour::ColourType};
        let kind = group
            .color_representation
            .map(|representation| match representation {
                Representation::Temperature => ColourType::Temperature,
                Representation::Xy => ColourType::Xy,
                Representation::Rgbwaf => ColourType::Rgbwaf,
            });
        if intent.colour().is_none() || intent.colour().map(|color| color.kind()) != kind {
            return Err(reject(
                "group color intent requires the selected representation",
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
        representation: group.color_representation,
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
    link: Transactions<Connection>,
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
    let mut colors = color::included(&r).then(color::Samples::new);
    let mut rejection = None;
    let result: Result<(), commissioning::Error> = async {
        if let Action::Color { intent, .. } = r.action {
            report.outcome = "unconfirmed";
            read_members(&bus, &r, &mut samples).await?;
            if let Err(reason) = color::roster(&r, &samples) {
                rejection = Some(reason);
                return Ok(());
            }
            let captured = colors.as_mut().unwrap();
            color::read(&bus, &r, &samples, captured, false).await?;
            if let Err(reason) = color::preflight(&r, &samples, captured, intent.colour().unwrap())
            {
                rejection = Some(reason);
                return Ok(());
            }
            // From the first mutation, only fresh post-write samples can confirm.
            color::invalidate(captured);
            for sample in &mut samples {
                *sample = Sample::unvisited(sample.address);
            }
            color::write(&bus, &r, intent).await?;
            report.outcome = "sent";
            read_members(&bus, &r, &mut samples).await?;
            color::read(&bus, &r, &samples, captured, true).await?;
            return Ok(());
        } else if let Action::Set { intent, .. } = r.action {
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
        read_members(&bus, &r, &mut samples).await?;
        if let Some(captured) = &mut colors {
            color::read(&bus, &r, &samples, captured, false).await?;
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
    let mut error = result.err().map(|e| {
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
    if error.is_none() {
        if let Some(reason) = rejection {
            report.reason = Some(reason.clone());
            report.outcome = "unconfirmed";
            error = Some(reject(reason));
        } else if let Action::Color { intent, .. } = r.action {
            let base_current = intent.base().is_none()
                || report
                    .values
                    .as_ref()
                    .is_some_and(|v| matches!(v.quality, "uniform" | "mixed"));
            if !base_current
                || !color::confirms(
                    &r,
                    &samples,
                    colors.as_ref().unwrap(),
                    intent.colour().unwrap(),
                )
            {
                let reason = "post-write group color/base evidence is incomplete".to_owned();
                report.reason = Some(reason.clone());
                report.outcome = "unconfirmed";
                error = Some(ServiceError::Uncertain(reason));
            }
        }
    }
    Outcome {
        observation: Observation {
            report: Arc::new(report),
            context: r.context,
            stamp,
            stale: false,
            colors,
        },
        error,
        recovery,
    }
}

async fn read_members(
    bus: &commissioning::Bus<'_>,
    route: &Route,
    samples: &mut [Sample],
) -> Result<(), commissioning::Error> {
    for sample in samples {
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
            .is_some_and(|mask| mask & (1 << route.group.raw()) != 0)
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
pub(super) fn accept(
    snapshot: &Snapshot,
    c: &mut Completion,
    hat: &hat::Hat,
) -> Result<Observation, ServiceError> {
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
    Ok(o)
}
pub(super) fn finish(q: Request, result: Result<Observation, ServiceError>) {
    let result = match result {
        Err(e) if q.started => {
            let mut r = q.report;
            r.outcome = "unconfirmed";
            r.reason = Some(e.to_string());
            Ok(Reply::Unconfirmed(r))
        }
        r => r.map(Reply::Observed),
    };
    drop(q._permit);
    let _ = q.reply.send(result);
}
impl ServiceHandle {
    /// Inspect actual members' native color support, including unconfigured groups.
    pub async fn read_group_color(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.group_output(target, Action::ReadColor, context, deadline, None)
            .await
    }
    /// Change a native group's selected color family after checking every known member.
    pub async fn set_group_color(
        &self,
        target: &str,
        expected: u64,
        intent: ColorIntent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.group_output(
            target,
            Action::Color {
                expected,
                intent: intent.into(),
            },
            context,
            deadline,
            None,
        )
        .await
    }
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
        self.group_evidence(target, action, context, deadline, completion)
            .await
            .map(Reply::into_report)
    }
    pub(super) async fn group_evidence(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Reply, ServiceError> {
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
