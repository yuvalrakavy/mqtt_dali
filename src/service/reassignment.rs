//! Explicit retirement followed by bounded native address-space clearing.
pub use super::discovery::Space;
use super::*;
use crate::atx::{
    connection::Connection,
    transactions::{BusReply, Response, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::{addressing, gear, input as device};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetOutcome {
    Cleared,
    RetiredButUnconfirmed,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub bus: u8,
    pub space: Space,
    pub outcome: ResetOutcome,
    pub revision: u64,
    pub retired_devices: usize,
    pub retired_instances: usize,
    pub checked_addresses: u8,
    pub reason: Option<String>,
}
pub(super) struct Request {
    pub bus: u8,
    pub space: Space,
    pub expected: u64,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub committed: Option<Report>,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Route {
    pub bus: u8,
    pub space: Space,
    pub controller: String,
    pub context: RequestContext,
    pub original_stamp: output::Stamp,
}
pub(super) struct Observation {
    pub stamp: output::Stamp,
    pub stale: bool,
    pub checked: u8,
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

pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let result = match (result, request.committed.as_ref()) {
        (Err(error), Some(saved)) => {
            let mut report = saved.clone();
            report.reason = Some(error.to_string());
            Ok(report)
        }
        (result, _) => result,
    };
    let Request { reply, _permit, .. } = request;
    drop(_permit);
    let _ = reply.send(result);
}

struct Bus<'a> {
    link: &'a Transactions<Connection>,
    route: &'a Route,
    deadline: Instant,
}
impl Bus<'_> {
    async fn exchange(
        &self,
        frame: DaliFrame,
        repeat: bool,
        deadline: Instant,
    ) -> Result<Vec<BusReply>, TransactionError> {
        let stamp = self.route.original_stamp;
        match self
            .link
            .execute_until(
                Command::Send {
                    bus: self.route.bus,
                    frame,
                    repeat,
                },
                deadline,
                Some((stamp.actor, stamp.bus)),
            )
            .await?
        {
            Response::Dali(replies) => Ok(replies),
            _ => Err(TransactionError::StreamLoss),
        }
    }
    async fn command(&self, frame: DaliFrame, repeat: bool) -> Result<(), ServiceError> {
        let pair = repeat && matches!(frame, DaliFrame::Bits24(_));
        let deadline = if pair {
            self.deadline
                .min(Instant::now() + Duration::from_millis(100))
        } else {
            self.deadline
        };
        for _ in 0..if pair { 2 } else { 1 } {
            let replies = self
                .exchange(frame.clone(), repeat && !pair, deadline)
                .await
                .map_err(|e| ServiceError::Rejected(e.to_string()))?;
            if replies.is_empty() || replies.iter().any(|r| *r != BusReply::NoResponse) {
                return Err(ServiceError::Rejected(
                    "unexpected reply to address clear".into(),
                ));
            }
        }
        Ok(())
    }
    async fn clear(&self, checked: &mut u8) -> Result<(), ServiceError> {
        let frames = match self.route.space {
            Space::Outputs => addressing::output(None, None)
                .into_iter()
                .map(DaliFrame::Bits16)
                .collect::<Vec<_>>(),
            Space::Inputs => addressing::input(None, None)
                .into_iter()
                .map(DaliFrame::Bits24)
                .collect(),
        };
        for (index, frame) in frames.into_iter().enumerate() {
            self.command(frame, index == 1).await?;
        }
        for address in 0..64 {
            let frame = match self.route.space {
                Space::Outputs => DaliFrame::Bits16(
                    gear::ShortAddress::new(address)
                        .unwrap()
                        .query(gear::Query::Status),
                ),
                Space::Inputs => DaliFrame::Bits24(
                    device::Address::new(address)
                        .unwrap()
                        .query(device::DeviceQuery::Status),
                ),
            };
            let replies = self
                .exchange(frame, false, self.deadline)
                .await
                .map_err(|e| ServiceError::Rejected(e.to_string()))?;
            *checked += 1;
            if replies != [BusReply::NoResponse] {
                return Err(ServiceError::Rejected(
                    "short-address absence was not confirmed".into(),
                ));
            }
        }
        Ok(())
    }
}
pub(super) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let mut checked = 0;
    let bus = Bus {
        link: &link,
        route: &route,
        deadline,
    };
    // WAIT: v2-physical-operation
    let result = bus.clear(&mut checked).await;
    let recovery = link.status() != crate::atx::transactions::SessionState::Ready;
    Outcome {
        observation: Observation {
            stamp,
            stale: false,
            checked,
        },
        error: result.err(),
        recovery,
    }
}

impl ServiceHandle {
    /// Retire saved identities, clear one native address space, then verify absence.
    /// Inspect outcome; confirmed retirement is retained on physical failure.
    pub async fn reset_address_space(
        &self,
        bus: u8,
        space: Space,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.reset_space(bus, space, expected_revision, context, deadline, None)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn reset_space(
        &self,
        bus: u8,
        space: Space,
        expected: u64,
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
        commissioning::resolve_space(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            bus,
            space,
            expected,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Reassignment(Request {
                bus,
                space,
                expected,
                context,
                deadline,
                completion,
                committed: None,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "reassignment caller deadline; inspect inventory".into(),
            ));
        }
        result
            .map_err(|_| {
                ServiceError::Uncertain("reassignment caller deadline; inspect inventory".into())
            })?
            .map_err(|_| ServiceError::Closed)?
    }
}
