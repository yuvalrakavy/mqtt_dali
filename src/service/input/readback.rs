//! Full-resolution reads owned by the same physical-operation lifecycle as lights.
use super::super::{Message, ServiceError, ServiceHandle};
use super::*;
use crate::atx::{
    connection::Connection,
    transactions::{BusReply, Response, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::input::{Address, InputValue, Instance, InstanceQuery, Resolution};
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit},
    time::{timeout_at, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadValue {
    pub reported_type: Option<u8>,
    pub resolution: Option<Resolution>,
    pub value: Option<InputValue>,
    pub quality: Quality,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Current,
    Partial,
    Unavailable,
    Unsupported,
    TypeMismatch,
}
impl Quality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::TypeMismatch => "type_mismatch",
        }
    }
}
pub(in crate::service) struct Request {
    pub target: String,
    pub context: RequestContext,
    pub deadline: Instant,
    pub native_sequence: u64,
    pub reply: oneshot::Sender<Result<Observation, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(in crate::service) struct Route {
    pub controller: String,
    pub bus: u8,
    pub address: Address,
    pub instance: Instance,
    pub kind: u8,
    pub context: RequestContext,
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
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    let projection = snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .ok_or_else(|| ServiceError::Rejected("unknown input instance".into()))?;
    let Location::InputInstance {
        bus,
        short_address,
        instance,
    } = projection.location
    else {
        return Err(ServiceError::Rejected(
            "individual input instance required".into(),
        ));
    };
    if !snapshot
        .model
        .inventory()
        .buses
        .iter()
        .any(|b| b.bus == bus && b.enabled)
    {
        return Err(ServiceError::Rejected("bus is disabled".into()));
    }
    let table = routing::generations(&snapshot.model);
    let token = &table.targets[target];
    if cause.generation().is_some_and(|g| g != token) {
        return Err(ServiceError::Rejected("retired input generation".into()));
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        address: Address::new(short_address).map_err(|e| ServiceError::Rejected(e.to_string()))?,
        instance: Instance::new(instance).map_err(|e| ServiceError::Rejected(e.to_string()))?,
        kind: projection.instance_type.expect("validated instance type"),
        context: RequestContext::new(token.clone())
            .map_err(ServiceError::Rejected)?
            .with_traceparent(cause.traceparent()),
    })
}

pub(in crate::service) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: Stamp,
    deadline: Instant,
) -> Outcome {
    let mut read = ReadValue {
        reported_type: None,
        resolution: None,
        value: None,
        quality: Quality::Unavailable,
        reason: None,
    };
    let result: Result<(), TransactionError> = async {
        read.reported_type = query(&link, &route, stamp, deadline, InstanceQuery::Type).await?;
        let Some(kind) = read.reported_type else {
            return Ok(());
        };
        if kind != route.kind {
            read.quality = Quality::TypeMismatch;
            return Ok(());
        }
        read.quality = Quality::Partial;
        read.resolution = query(&link, &route, stamp, deadline, InstanceQuery::Resolution)
            .await?
            .and_then(|v| Resolution::new(v).ok());
        let Some(resolution) = read.resolution else {
            return Ok(());
        };
        let mut bytes = Vec::with_capacity(resolution.byte_count());
        for index in 0..resolution.byte_count() {
            let command = if index == 0 {
                InstanceQuery::InputValue
            } else {
                InstanceQuery::InputValueLatch
            };
            let Some(byte) = query(&link, &route, stamp, deadline, command).await? else {
                return Ok(());
            };
            bytes.push(byte);
        }
        read.value =
            Some(InputValue::decode(resolution, &bytes).expect("complete bounded input bytes"));
        read.quality = match (kind, resolution.bits()) {
            (1, 1) | (2, _) | (3, 2) | (4, _) => Quality::Current,
            _ => Quality::Unsupported,
        };
        Ok(())
    }
    .await;
    let recovery = matches!(result, Err(TransactionError::StreamLoss));
    let error = result.err().map(|e| {
        read.reason = Some(e.to_string());
        ServiceError::Rejected(e.to_string())
    });
    Outcome {
        observation: Observation {
            context: route.context,
            evidence: Evidence::Query(read),
            bus: route.bus,
            stamp,
            stale: false,
        },
        error,
        recovery,
    }
}
async fn query(
    link: &Transactions<Connection>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    query: InstanceQuery,
) -> Result<Option<u8>, TransactionError> {
    let response = link
        .execute_until(
            Command::Send {
                bus: route.bus,
                frame: DaliFrame::Bits24(route.address.query_instance(route.instance, query)),
                repeat: false,
            },
            deadline,
            Some((stamp.actor, stamp.bus)),
        )
        .await?;
    match response {
        Response::Dali(replies) if replies.len() == 1 => Ok(match replies[0] {
            BusReply::Value(v) => Some(v),
            _ => None,
        }),
        _ => Err(TransactionError::StreamLoss),
    }
}

impl ServiceHandle {
    /// Read a saved input instance without generating a native input action.
    pub async fn read_input(
        &self,
        target: &str,
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
        resolve(&snapshot, target, &context)?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Input(Request {
                target: target.into(),
                context,
                deadline,
                native_sequence: 0,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Rejected("input read caller deadline".into()));
        }
        result
            .map_err(|_| ServiceError::Rejected("input read caller deadline".into()))?
            .map_err(|_| ServiceError::Closed)?
    }
}
