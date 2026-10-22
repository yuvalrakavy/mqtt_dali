//! One output/input assignment: search, durable reservation, verified programming.
pub use super::discovery::Space;
use super::*;
use crate::atx::{
    serial::Serial,
    transactions::{BusReply, Response, SessionState, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::input::commissioning as device;
use crate::dali::{commissioning as frames, gear, input as input_frames};
use serde::Serialize;
use std::{collections::BTreeSet, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentOutcome {
    Assigned,
    ReservedButUnconfirmed,
    NoUnaddressedOutput,
    NoUnaddressedInput,
    NoFreeAddress,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub space: Space,
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
    pub space: Space,
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
    pub space: Space,
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
    resolve_space(snapshot, bus, Space::Outputs, expected, cause)
}
pub(super) fn resolve_space(
    snapshot: &Snapshot,
    bus: u8,
    space: Space,
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
        space,
        bus,
        controller: snapshot.model.inventory().name.clone(),
        context: cause.clone(),
        plan: Plan::Search(match space {
            Space::Outputs => saved.outputs.iter().map(|o| o.short_address).collect(),
            Space::Inputs => saved.inputs.iter().map(|i| i.short_address).collect(),
        }),
    })
}
pub(super) fn report(
    request: &Request,
    outcome: AssignmentOutcome,
    revision: u64,
    reason: Option<String>,
) -> Report {
    Report {
        space: request.space,
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
    fn frame(&self, gear: u16, input: u32) -> DaliFrame {
        match self.route.space {
            Space::Outputs => DaliFrame::Bits16(gear),
            Space::Inputs => DaliFrame::Bits24(input),
        }
    }
    fn status(&self, address: u8) -> DaliFrame {
        self.frame(
            gear::ShortAddress::new(address)
                .unwrap()
                .query(gear::Query::Status),
            input_frames::Address::new(address)
                .unwrap()
                .query(input_frames::DeviceQuery::Status),
        )
    }
    async fn exchange(
        &self,
        frame: DaliFrame,
        repeat: bool,
        deadline: Instant,
    ) -> Result<Vec<BusReply>, Error> {
        match self
            .link
            .execute_until(
                Command::Send {
                    bus: self.route.bus,
                    frame,
                    repeat,
                },
                deadline,
                Some((self.stamp.actor, self.stamp.bus)),
            )
            .await?
        {
            Response::Dali(replies) => Ok(replies),
            _ => Err(TransactionError::StreamLoss.into()),
        }
    }
    async fn query(&self, frame: DaliFrame) -> Result<BusReply, Error> {
        let mut replies = self.exchange(frame, false, self.deadline).await?;
        if replies.len() != 1 {
            return Err(TransactionError::StreamLoss.into());
        }
        Ok(replies.remove(0))
    }
    async fn command(&self, frame: DaliFrame, repeat: bool) -> Result<(), Error> {
        let input_pair = repeat && matches!(frame, DaliFrame::Bits24(_));
        let deadline = if input_pair {
            self.deadline
                .min(Instant::now() + Duration::from_millis(100))
        } else {
            self.deadline
        };
        for _ in 0..if input_pair { 2 } else { 1 } {
            let replies = self
                .exchange(frame.clone(), repeat && !input_pair, deadline)
                .await?;
            if replies.is_empty() || replies.iter().any(|r| *r != BusReply::NoResponse) {
                return Err(Error::Evidence(
                    "unexpected backward reply to addressing command",
                ));
            }
        }
        Ok(())
    }
    async fn select(&self, address: frames::SearchAddress) -> Result<(), Error> {
        let frames: Vec<_> = match self.route.space {
            Space::Outputs => address
                .frames()
                .into_iter()
                .map(DaliFrame::Bits16)
                .collect(),
            Space::Inputs => device::search(address)
                .into_iter()
                .map(DaliFrame::Bits24)
                .collect(),
        };
        for frame in frames {
            self.command(frame, false).await?;
        }
        Ok(())
    }
    async fn compare(&self, exact: bool) -> Result<bool, Error> {
        match self
            .query(self.frame(frames::COMPARE, device::COMPARE))
            .await?
        {
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
            match self.query(self.status(address)).await? {
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
        self.command(self.frame(frames::TERMINATE, device::TERMINATE), false)
            .await?;
        *entered = true;
        self.command(
            self.frame(
                frames::INITIALISE_UNADDRESSED,
                device::INITIALISE_UNADDRESSED,
            ),
            true,
        )
        .await?;
        self.command(self.frame(frames::RANDOMISE, device::RANDOMISE), true)
            .await?;
        // WAIT: v2-commissioning-randomise
        timeout_at(
            self.deadline,
            tokio::time::sleep(Duration::from_millis(
                if self.route.space == Space::Inputs {
                    200
                } else {
                    100
                },
            )),
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
            || self
                .query(self.frame(frames::QUERY_SHORT_ADDRESS, device::QUERY_SHORT_ADDRESS))
                .await?
                != BusReply::Value(255)
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
        if self.query(self.status(candidate.address)).await? != BusReply::NoResponse {
            return Err(Error::Evidence("reserved destination became occupied"));
        }
        self.select(candidate.random).await?;
        if !self.compare(true).await?
            || self
                .query(self.frame(frames::QUERY_SHORT_ADDRESS, device::QUERY_SHORT_ADDRESS))
                .await?
                != BusReply::Value(255)
        {
            return Err(Error::Evidence(
                "unaddressed candidate changed after reservation",
            ));
        }
        self.command(
            self.frame(
                frames::program(address),
                device::program(input_frames::Address::new(candidate.address).unwrap()),
            ),
            false,
        )
        .await?;
        if self
            .query(self.frame(
                frames::verify(address),
                device::verify(input_frames::Address::new(candidate.address).unwrap()),
            ))
            .await?
            != BusReply::Value(255)
            || self
                .query(self.frame(frames::QUERY_SHORT_ADDRESS, device::QUERY_SHORT_ADDRESS))
                .await?
                != BusReply::Value(match self.route.space {
                    Space::Outputs => (candidate.address << 1) | 1,
                    Space::Inputs => candidate.address,
                })
        {
            return Err(Error::Evidence("programmed short address was not verified"));
        }
        self.command(self.frame(frames::WITHDRAW, device::WITHDRAW), false)
            .await?;
        self.command(self.frame(frames::TERMINATE, device::TERMINATE), false)
            .await?;
        *entered = false;
        if !matches!(
            self.query(self.status(candidate.address)).await?,
            BusReply::Value(_)
        ) {
            return Err(Error::Evidence(
                "assigned fixture did not answer a fresh status query",
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
        let cleanup = bus
            .command(bus.frame(frames::TERMINATE, device::TERMINATE), false)
            .await;
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
        self.assign_space(bus, Space::Outputs, expected, context, deadline, completion)
            .await
    }
    /// Assign a new module; call addressed discovery afterward for real instances.
    pub async fn assign_next_input(
        &self,
        bus: u8,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.assign_space(
            bus,
            Space::Inputs,
            expected_revision,
            context,
            deadline,
            None,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn assign_space(
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
        resolve_space(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            bus,
            space,
            expected,
            &context,
        )?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Commission(Request {
                space,
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
