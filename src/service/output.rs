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
    pub intent: Option<Intent>,
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
    intent: Option<Intent>,
    deadline: Instant,
) -> Outcome {
    let mut observation = Observation {
        context: route.context,
        actual_level: None,
        status: None,
        quality: Quality::Unavailable,
        reason: None,
        stamp,
        bus: route.bus,
    };
    let mut recovery = false;
    let operation = async {
        if let Some(intent) = intent {
            match frame(
                &link,
                route.bus,
                intent.frame(Destination::Short(route.address)),
                deadline,
                stamp,
            )
            .await?
            {
                BusReply::NoResponse => {}
                BusReply::Value(_) => {
                    recovery = true;
                    return Err(TransactionError::StreamLoss);
                }
                reply => {
                    observation.reason = Some(format!("write collision: {reply:?}"));
                    return Ok(());
                }
            }
        }
        let level = frame(
            &link,
            route.bus,
            route.address.query(Query::ActualLevel),
            deadline,
            stamp,
        )
        .await?;
        observation.actual_level = Some(Query::ActualLevel.decode(level));
        observation.update_quality();
        let status = frame(
            &link,
            route.bus,
            route.address.query(Query::Status),
            deadline,
            stamp,
        )
        .await?;
        observation.status = Some(Query::Status.decode(status));
        observation.update_quality();
        Ok(())
    }
    .await;
    let error = match operation {
        Err(error) => {
            observation.reason = Some(error.to_string());
            Some(if intent.is_some() {
                ServiceError::Uncertain(error.to_string())
            } else {
                ServiceError::Rejected(error.to_string())
            })
        }
        Ok(()) if intent.is_some() && observation.quality != Quality::Current => Some(
            ServiceError::Uncertain("post-write fixture readback is incomplete".into()),
        ),
        _ => None,
    };
    Outcome {
        observation,
        error,
        recovery,
    }
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
        self.output(target, None, context, deadline).await
    }
    pub async fn apply_output(
        &self,
        target: &str,
        intent: Intent,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Observation, ServiceError> {
        self.output(target, Some(intent), context, deadline).await
    }
    async fn output(
        &self,
        target: &str,
        intent: Option<Intent>,
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
        resolve(&snapshot, target, &context, intent.is_some())?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Output(Request {
                target: target.into(),
                intent,
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
