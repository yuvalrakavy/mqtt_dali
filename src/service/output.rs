//! Single-output observations retain their original mapping and UART evidence.
use super::*;
use crate::atx::{
    serial::Serial,
    transactions::{BusReply, Response, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::gear::{
    Destination, Intent, LevelValue, Query, QueryReply, QueryValue, ShortAddress,
};
use std::collections::BTreeMap;
pub mod colour;

#[derive(Debug, Clone, Copy)]
pub(super) enum WriteIntent {
    Base(Intent),
    Colour(crate::dali::colour::Intent),
    Combined(Intent, crate::dali::colour::Intent),
}
impl WriteIntent {
    fn base(self) -> Option<Intent> {
        match self {
            Self::Base(base) | Self::Combined(base, _) => Some(base),
            _ => None,
        }
    }
    fn colour(self) -> Option<crate::dali::colour::Intent> {
        match self {
            Self::Colour(colour) | Self::Combined(_, colour) => Some(colour),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Current,
    Partial,
    Unavailable,
    Stale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub context: RequestContext,
    pub actual_level: Option<QueryReply>,
    pub status: Option<QueryReply>,
    pub colour: Option<colour::Observation>,
    pub quality: Quality,
    pub reason: Option<String>,
    pub(super) stamp: Stamp,
    pub(super) bus: u8,
}
impl Observation {
    pub fn level(&self) -> Option<LevelValue> {
        match self.actual_level {
            Some(QueryReply::Value(QueryValue::ActualLevel(level))) => Some(level),
            _ => None,
        }
    }
    pub fn available(&self) -> bool {
        self.quality != Quality::Stale
            && matches!(self.actual_level, Some(QueryReply::Value(_)))
            && matches!(self.status, Some(QueryReply::Value(_)))
    }
    pub fn power(&self) -> Option<bool> {
        self.brightness().map(|level| level > 0.0)
    }
    pub fn brightness(&self) -> Option<f64> {
        if self.quality != Quality::Current {
            return None;
        }
        match self.level() {
            Some(LevelValue::Known(level)) => Some(level.normalized()),
            _ => None,
        }
    }
    pub(super) fn invalidate(&mut self, reason: &str) {
        self.quality = Quality::Stale;
        self.reason = Some(reason.into());
        if let Some(colour) = &mut self.colour {
            colour.quality = colour::Quality::Stale;
            colour.reason = Some(reason.into());
        }
    }
    fn update_quality(&mut self) {
        self.quality = if matches!(self.level(), Some(LevelValue::Known(_)))
            && matches!(self.status, Some(QueryReply::Value(QueryValue::Status(_))))
        {
            Quality::Current
        } else if matches!(self.actual_level, Some(QueryReply::Value(_)))
            || matches!(self.status, Some(QueryReply::Value(_)))
        {
            Quality::Partial
        } else {
            Quality::Unavailable
        };
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp {
    pub session: u64,
    pub actor: u64,
    pub bus: u64,
    pub observed_bus: u64,
}
pub(super) struct Request {
    pub target: String,
    pub intent: Option<WriteIntent>,
    pub include_colour: bool,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Observation, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub address: ShortAddress,
    pub context: RequestContext,
}
pub(super) struct Active {
    pub request: Request,
    pub route: Route,
    pub stamp: Stamp,
    pub stage: Stage,
    pub outcome: Option<Outcome>,
    pub cleanup: Option<Instant>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Pending,
    Running,
    Settling,
    Closing,
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
    cause: &RequestContext,
    writing: bool,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    let projection = snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .ok_or_else(|| ServiceError::Rejected("unknown output".into()))?;
    let crate::inventory::Location::Output { bus, short_address } = projection.location else {
        return Err(ServiceError::Rejected("individual output required".into()));
    };
    if !snapshot
        .model
        .inventory()
        .buses
        .iter()
        .any(|value| value.bus == bus && value.enabled)
    {
        return Err(ServiceError::Rejected("bus is disabled".into()));
    }
    let table = routing::generations(&snapshot.model);
    let token = &table.targets[target];
    if (writing || cause.generation().is_some()) && cause.generation() != Some(token.as_str()) {
        return Err(ServiceError::Rejected(
            "missing or retired target generation".into(),
        ));
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        address: ShortAddress::new(short_address)
            .map_err(|e| ServiceError::Rejected(e.to_string()))?,
        context: RequestContext::new(token.clone())
            .map_err(ServiceError::Rejected)?
            .with_traceparent(cause.traceparent()),
    })
}

pub(super) async fn execute(
    link: Transactions<Serial>,
    route: Route,
    stamp: Stamp,
    intent: Option<WriteIntent>,
    include_colour: bool,
    deadline: Instant,
) -> Outcome {
    let mut observation = Observation {
        context: route.context.clone(),
        actual_level: None,
        status: None,
        colour: None,
        quality: Quality::Unavailable,
        reason: None,
        stamp,
        bus: route.bus,
    };
    let mut recovery = false;
    let mut rejection = None;
    let base = intent.and_then(WriteIntent::base);
    let colour_intent = intent.and_then(WriteIntent::colour);
    let operation: Result<(), TransactionError> = async {
        if let Some(base) = base.filter(|_| colour_intent.is_none()) {
            if !write_base(&mut observation, &link, &route, stamp, deadline, base).await? {
                return Ok(());
            }
        }
        read_base(&mut observation, &link, &route, stamp, deadline).await?;
        if include_colour {
            let sample = observation.colour.insert(colour::Observation::default());
            colour::discover(sample, &link, &route, stamp, deadline).await?;
            if let Some(intent) = colour_intent {
                if let Err(reason) = sample.preflight(intent) {
                    rejection = Some(reason);
                    return Ok(());
                }
                // From the first mutation onward pre-write colour/base evidence
                // cannot confirm this mutation, even if the transport fails.
                sample.quality = colour::Quality::Partial;
                sample.values.clear();
                sample.raw_values.clear();
                sample.status = None;
                observation.actual_level = None;
                observation.status = None;
                observation.update_quality();
                if let Some(base) = base.filter(|base| base.is_off()) {
                    if !write_base(&mut observation, &link, &route, stamp, deadline, base).await? {
                        return Ok(());
                    }
                }
                colour::apply(&link, &route, stamp, deadline, intent).await?;
                if let Some(base) = base.filter(|base| !base.is_off()) {
                    if !write_base(&mut observation, &link, &route, stamp, deadline, base).await? {
                        return Ok(());
                    }
                }
                read_base(&mut observation, &link, &route, stamp, deadline).await?;
                colour::read_values(
                    observation.colour.as_mut().unwrap(),
                    &link,
                    &route,
                    stamp,
                    deadline,
                )
                .await?;
            }
        }
        Ok(())
    }
    .await;
    let error = match operation {
        Err(error) => {
            recovery |= error == TransactionError::StreamLoss;
            observation.reason = Some(error.to_string());
            Some(if intent.is_some() {
                ServiceError::Uncertain(error.to_string())
            } else {
                ServiceError::Rejected(error.to_string())
            })
        }
        Ok(()) if rejection.is_some() => rejection.map(ServiceError::Rejected),
        Ok(()) if intent.is_some() && observation.quality != Quality::Current => Some(
            ServiceError::Uncertain("post-write fixture readback is incomplete".into()),
        ),
        Ok(())
            if colour_intent.is_some_and(|value| {
                !observation
                    .colour
                    .as_ref()
                    .is_some_and(|sample| sample.confirms(value))
            }) =>
        {
            Some(ServiceError::Uncertain(
                "requested colour representation readback is incomplete".into(),
            ))
        }
        _ => None,
    };
    Outcome {
        observation,
        error,
        recovery,
    }
}
async fn write_base(
    observation: &mut Observation,
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    intent: Intent,
) -> Result<bool, TransactionError> {
    match frame(
        link,
        route.bus,
        intent.frame(Destination::Short(route.address)),
        deadline,
        stamp,
    )
    .await?
    {
        BusReply::NoResponse => Ok(true),
        BusReply::Value(_) => Err(TransactionError::StreamLoss),
        reply => {
            observation.reason = Some(format!("write collision: {reply:?}"));
            Ok(false)
        }
    }
}
async fn read_base(
    observation: &mut Observation,
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
) -> Result<(), TransactionError> {
    observation.actual_level = Some(
        Query::ActualLevel.decode(
            frame(
                link,
                route.bus,
                route.address.query(Query::ActualLevel),
                deadline,
                stamp,
            )
            .await?,
        ),
    );
    observation.update_quality();
    observation.status = Some(
        Query::Status.decode(
            frame(
                link,
                route.bus,
                route.address.query(Query::Status),
                deadline,
                stamp,
            )
            .await?,
        ),
    );
    observation.update_quality();
    Ok(())
}
async fn frame(
    link: &Transactions<Serial>,
    bus: u8,
    value: u16,
    deadline: Instant,
    stamp: Stamp,
) -> Result<BusReply, TransactionError> {
    let result = link
        .execute_until(
            Command::Send {
                bus,
                frame: DaliFrame::Bits16(value),
                repeat: false,
            },
            deadline,
            Some((stamp.actor, stamp.bus)),
        )
        .await?;
    match result {
        Response::Dali(mut replies) if replies.len() == 1 => Ok(replies.remove(0)),
        _ => Err(TransactionError::StreamLoss),
    }
}

impl ServiceHandle {
    pub async fn read_output(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(target, None, false, context, deadline).await
    }
    pub async fn apply_output(
        &self,
        target: &str,
        intent: Intent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(
            target,
            Some(WriteIntent::Base(intent)),
            false,
            context,
            deadline,
        )
        .await
    }
    pub async fn read_colour(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(target, None, true, context, deadline).await
    }
    pub async fn apply_colour(
        &self,
        target: &str,
        intent: crate::dali::colour::Intent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(
            target,
            Some(WriteIntent::Colour(intent)),
            true,
            context,
            deadline,
        )
        .await
    }
    pub async fn apply_colour_with_output(
        &self,
        target: &str,
        colour: crate::dali::colour::Intent,
        base: Intent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(
            target,
            Some(WriteIntent::Combined(base, colour)),
            true,
            context,
            deadline,
        )
        .await
    }
    async fn output(
        &self,
        target: &str,
        intent: Option<WriteIntent>,
        include_colour: bool,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
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
        let snapshot = self
            .snapshot()
            .ok_or_else(|| ServiceError::Rejected("service is starting".into()))?;
        let route = resolve(&snapshot, target, &context, intent.is_some())?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Output(Request {
                target: target.into(),
                intent,
                include_colour: include_colour
                    || snapshot.outputs.get(target).is_some_and(|sample| {
                        sample.context.generation() == route.context.generation()
                            && sample.colour.is_some()
                    }),
                context,
                deadline,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain("caller deadline".into()));
        }
        result
            .map_err(|_| ServiceError::Uncertain("caller deadline".into()))?
            .map_err(|_| ServiceError::Closed)?
    }
}

pub(super) fn retained(
    model: &Model,
    observations: BTreeMap<String, Observation>,
) -> BTreeMap<String, Observation> {
    let table = routing::generations(model);
    observations
        .into_iter()
        .filter(|(target, observation)| {
            table.targets.get(target).map(String::as_str) == observation.context.generation()
        })
        .collect()
}

#[cfg(test)]
mod tests;
