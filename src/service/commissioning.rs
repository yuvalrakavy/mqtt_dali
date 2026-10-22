//! One output assignment: search, durable reservation, then verified programming.
use super::*;
use crate::atx::{
    serial::Serial,
    transactions::{BusReply, Response, SessionState, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::{commissioning as frames, gear};
use serde::Serialize;
use std::{collections::BTreeSet, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentOutcome {
    Assigned,
    ReservedButUnconfirmed,
    NoUnaddressedOutput,
    NoFreeAddress,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub outcome: AssignmentOutcome,
    pub bus: u8,
    pub address: Option<u8>,
    pub target: Option<String>,
    pub revision: u64,
    pub reason: Option<String>,
}
#[derive(Clone)]
pub(super) struct Reservation {
    pub address: u8,
    pub target: String,
    pub revision: u64,
}
pub(super) struct Request {
    pub bus: u8,
    pub expected: u64,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub reservation: Option<Reservation>,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Candidate {
    pub address: u8,
    pub random: frames::SearchAddress,
    pub stamp: output::Stamp,
}
#[derive(Clone)]
pub(super) enum Plan {
    Search(BTreeSet<u8>),
    Program(Candidate),
}
#[derive(Clone)]
pub(super) struct Route {
    pub bus: u8,
    pub controller: String,
    pub context: RequestContext,
    pub plan: Plan,
}
pub(super) enum Step {
    NoCapacity,
    NoCandidate,
    Found {
        address: u8,
        random: frames::SearchAddress,
    },
    Assigned,
}
pub(super) struct Observation {
    pub bus: u8,
    pub stamp: output::Stamp,
    pub stale: bool,
    pub step: Option<Step>,
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
pub(super) enum Advance {
    Program(Route),
    Done(Report),
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    bus: u8,
    expected: u64,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    if expected != snapshot.model.inventory().revision {
        return Err(ServiceError::Rejected("inventory revision changed".into()));
    }
    if cause.generation() != Some(routing::generations(&snapshot.model).instance.as_str()) {
        return Err(ServiceError::Rejected(
            "missing or retired controller generation".into(),
        ));
    }
    let saved = snapshot
        .model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == bus && b.enabled && bus <= 3)
        .ok_or_else(|| ServiceError::Rejected("unknown/disabled bus".into()))?;
    Ok(Route {
        bus,
        controller: snapshot.model.inventory().name.clone(),
        context: cause.clone(),
        plan: Plan::Search(saved.outputs.iter().map(|o| o.short_address).collect()),
    })
}
pub(super) fn report(
    request: &Request,
    outcome: AssignmentOutcome,
    revision: u64,
    reason: Option<String>,
) -> Report {
    Report {
        outcome,
        bus: request.bus,
        address: request.reservation.as_ref().map(|r| r.address),
        target: request.reservation.as_ref().map(|r| r.target.clone()),
        revision,
        reason,
    }
}
pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let result = match (result, &request.reservation) {
        (Err(error), Some(reservation)) => Ok(report(
            &request,
            AssignmentOutcome::ReservedButUnconfirmed,
            reservation.revision,
            Some(error.to_string()),
        )),
        (result, _) => result,
    };
    let Request { reply, _permit, .. } = request;
    drop(_permit);
    let _ = reply.send(result);
}

#[derive(Debug)]
enum Error {
    Transaction(TransactionError),
    Evidence(&'static str),
}
impl From<TransactionError> for Error {
    fn from(value: TransactionError) -> Self {
        Self::Transaction(value)
    }
}
struct Bus<'a> {
    link: &'a Transactions<Serial>,
    route: &'a Route,
    stamp: output::Stamp,
    deadline: Instant,
}
impl Bus<'_> {
    async fn exchange(&self, frame: u16, repeat: bool) -> Result<Vec<BusReply>, Error> {
        match self
            .link
            .execute_until(
                Command::Send {
                    bus: self.route.bus,
                    frame: DaliFrame::Bits16(frame),
                    repeat,
                },
                self.deadline,
                Some((self.stamp.actor, self.stamp.bus)),
            )
            .await?
        {
            Response::Dali(replies) => Ok(replies),
            _ => Err(TransactionError::StreamLoss.into()),
        }
    }
    async fn query(&self, frame: u16) -> Result<BusReply, Error> {
        let mut replies = self.exchange(frame, false).await?;
        if replies.len() != 1 {
            return Err(TransactionError::StreamLoss.into());
        }
        Ok(replies.remove(0))
    }
    async fn command(&self, frame: u16, repeat: bool) -> Result<(), Error> {
        let replies = self.exchange(frame, repeat).await?;
        if !replies.is_empty() && replies.iter().all(|r| *r == BusReply::NoResponse) {
            Ok(())
        } else {
            Err(Error::Evidence(
                "unexpected backward reply to addressing command",
            ))
        }
    }
    async fn select(&self, address: frames::SearchAddress) -> Result<(), Error> {
        for frame in address.frames() {
            self.command(frame, false).await?;
        }
        Ok(())
    }
    async fn compare(&self, exact: bool) -> Result<bool, Error> {
        match self.query(frames::COMPARE).await? {
            BusReply::Value(255) => Ok(true),
            BusReply::NoResponse => Ok(false),
            BusReply::ReceiveCollision(_) if !exact => Ok(true),
            _ => Err(Error::Evidence(
                "ambiguous or invalid commissioning COMPARE reply",
            )),
        }
    }
    async fn search(&self, reserved: &BTreeSet<u8>, entered: &mut bool) -> Result<Step, Error> {
        let mut destination = None;
        for address in 0..64 {
            if reserved.contains(&address) {
                continue;
            }
            match self
                .query(
                    gear::ShortAddress::new(address)
                        .unwrap()
                        .query(gear::Query::Status),
                )
                .await?
            {
                BusReply::NoResponse => {
                    destination = Some(address);
                    break;
                }
                BusReply::Value(_) | BusReply::ReceiveCollision(_) => {}
                _ => return Err(Error::Evidence("destination address could not be checked")),
            }
        }
        let Some(address) = destination else {
            return Ok(Step::NoCapacity);
        };
        self.command(frames::TERMINATE, false).await?;
        *entered = true;
        self.command(frames::INITIALISE_UNADDRESSED, true).await?;
        self.command(frames::RANDOMISE, true).await?;
        // WAIT: v2-commissioning-randomise
        timeout_at(
            self.deadline,
            tokio::time::sleep(Duration::from_millis(100)),
        )
        .await
        .map_err(|_| TransactionError::Deadline)?;
        let (mut low, mut high) = (0u32, 0xffffffu32);
        self.select(frames::SearchAddress::new(high).unwrap())
            .await?;
        if !self.compare(false).await? {
            return Ok(Step::NoCandidate);
        }
        while low < high {
            let middle = low + (high - low) / 2;
            self.select(frames::SearchAddress::new(middle).unwrap())
                .await?;
            if self.compare(false).await? {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let random = frames::SearchAddress::new(low).unwrap();
        self.select(random).await?;
        if !self.compare(true).await?
            || self.query(frames::QUERY_SHORT_ADDRESS).await? != BusReply::Value(255)
        {
            return Err(Error::Evidence(
                "selected candidate is no longer uniquely unaddressed",
            ));
        }
        Ok(Step::Found { address, random })
    }
    async fn program(&self, candidate: &Candidate, entered: &mut bool) -> Result<Step, Error> {
        *entered = true;
        let address = gear::ShortAddress::new(candidate.address).unwrap();
        if self.query(address.query(gear::Query::Status)).await? != BusReply::NoResponse {
            return Err(Error::Evidence("reserved destination became occupied"));
        }
        self.select(candidate.random).await?;
        if !self.compare(true).await?
            || self.query(frames::QUERY_SHORT_ADDRESS).await? != BusReply::Value(255)
        {
            return Err(Error::Evidence(
                "unaddressed candidate changed after reservation",
            ));
        }
        self.command(frames::program(address), false).await?;
        if self.query(frames::verify(address)).await? != BusReply::Value(255)
            || self.query(frames::QUERY_SHORT_ADDRESS).await?
                != BusReply::Value((candidate.address << 1) | 1)
        {
            return Err(Error::Evidence("programmed short address was not verified"));
        }
        self.command(frames::WITHDRAW, false).await?;
        self.command(frames::TERMINATE, false).await?;
        *entered = false;
        if !matches!(
            self.query(address.query(gear::Query::Status)).await?,
            BusReply::Value(_)
        ) {
            return Err(Error::Evidence(
                "assigned output did not answer a fresh status query",
            ));
        }
        Ok(Step::Assigned)
    }
}
pub(super) async fn execute(
    link: Transactions<Serial>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let bus = Bus {
        link: &link,
        route: &route,
        stamp,
        deadline,
    };
    let mut entered = false;
    let mut result = match &route.plan {
        Plan::Search(reserved) => bus.search(reserved, &mut entered).await,
        Plan::Program(candidate) => bus.program(candidate, &mut entered).await,
    };
    let mut recovery = matches!(
        &result,
        Err(Error::Transaction(TransactionError::StreamLoss))
    );
    if entered
        && !matches!(&result, Ok(Step::Found { .. }))
        && link.status() == SessionState::Ready
        && Instant::now() < deadline
    {
        let cleanup = bus.command(frames::TERMINATE, false).await;
        recovery |= matches!(
            &cleanup,
            Err(Error::Transaction(TransactionError::StreamLoss))
        );
        if result.is_ok() {
            if let Err(error) = cleanup {
                result = Err(error);
            }
        }
    }
    let (step, error) = match result {
        Ok(step) => (Some(step), None),
        Err(Error::Transaction(error)) => (None, Some(ServiceError::Rejected(error.to_string()))),
        Err(Error::Evidence(reason)) => (None, Some(ServiceError::Rejected(reason.into()))),
    };
    Outcome {
        observation: Observation {
            bus: route.bus,
            stamp,
            stale: false,
            step,
        },
        error,
        recovery,
    }
}

impl ServiceHandle {
    /// Assign one unaddressed output while preserving saved/missing addresses.
    /// Inspect the report outcome: a confirmed reservation can remain even when
    /// physical programming is unconfirmed. Cancellation never undoes a save;
    /// inspect inventory after a lost result. See `docs/commissioning.md`.
    pub async fn assign_next_output(
        &self,
        bus: u8,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.assign(bus, expected_revision, context, deadline, None)
            .await
    }
    pub(super) async fn assign(
        &self,
        bus: u8,
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
        resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            bus,
            expected,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Commission(Request {
                bus,
                expected,
                context,
                deadline,
                completion,
                reservation: None,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "assignment caller deadline; inspect inventory".into(),
            ));
        }
        result
            .map_err(|_| {
                ServiceError::Uncertain("assignment caller deadline; inspect inventory".into())
            })?
            .map_err(|_| ServiceError::Closed)?
    }
}
