//! UART lifecycle only. Fixture operations must join the service admission path.
use super::{input, output, physical, ServiceError};
use super::{BusHealth, Hardware};
use crate::atx::{
    monitor::{CloseReason, Event},
    serial::Serial,
    transactions::{RepeatPolicy, Response, SessionState, TransactionError, Transactions},
    wire::{Command, Kind, Version},
};
use std::{future::pending, io, path::PathBuf, time::Duration};
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
    time::{timeout_at, Instant},
};
use tracing::Instrument;

pub(super) struct Settings {
    path: PathBuf,
    repeat: RepeatPolicy,
    pub(super) quiet: Duration,
    pub(super) timing: Timing,
    #[cfg(test)]
    pub(super) open_hook: Option<crate::atx::serial::OpenHook>,
}
#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub setup: Duration,
    pub poll: Duration,
    pub retry: Duration,
}
impl Settings {
    pub(super) fn new(path: PathBuf, repeat: RepeatPolicy) -> Self {
        Self {
            path,
            repeat,
            quiet: Duration::from_millis(250),
            timing: Timing {
                setup: Duration::from_secs(5),
                poll: Duration::from_secs(5),
                retry: Duration::from_secs(5),
            },
            #[cfg(test)]
            open_hook: None,
        }
    }
}
/// An owned join result survives cancellation of the containing select future.
struct Job(JoinHandle<Done>, JobKind);
#[derive(Clone, Copy, PartialEq, Eq)]
enum JobKind {
    Open,
    Boundary,
    Recover,
    Query,
    Close,
    Physical,
    InputBoundary,
    Settle,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.0.abort();
    }
}
enum Done {
    Open(io::Result<Serial>),
    Setup(Result<(Version, Vec<BusHealth>), TransactionError>),
    Poll(Result<Vec<BusHealth>, TransactionError>),
    Boundary(Result<u64, TransactionError>),
    Recover(Result<(), TransactionError>),
    Close(Result<(), TransactionError>, bool),
    Physical(physical::Outcome),
    InputBoundary(Vec<input::Cut>, Result<u64, TransactionError>),
    Settled(Result<(), TransactionError>),
}
struct Session {
    link: Transactions<Serial>,
    events: broadcast::Receiver<Event>,
    status: watch::Receiver<SessionState>,
}
pub(super) struct Hat {
    settings: Option<Settings>,
    buses: Vec<u8>,
    enabled_buses: std::collections::BTreeSet<u8>,
    session: Option<Session>,
    job: Option<Job>,
    discard_job: bool,
    closing: bool,
    closed: bool,
    published: Hardware,
    next: Instant,
    terminal: bool,
    reset: bool,
    requires_boot: bool,
    needs_quiet: bool,
    needs_boundary: bool,
    boundary: Option<u64>,
    boot: Option<u64>,
    version: Option<Version>,
    observations: Vec<BusHealth>,
    error: String,
    session_generation: u64,
    bus_generations: [u64; 4],
    physical: Option<physical::Active>,
    completed: Option<physical::Completion>,
    inputs: input::Router,
}
enum Wake {
    Job(Result<Done, tokio::task::JoinError>),
    Event(Result<Event, broadcast::error::RecvError>),
    Status,
    Timer,
    OutputExpired,
}
impl Hat {
    pub(super) fn new(settings: Option<Settings>, mut buses: Vec<u8>) -> Self {
        let enabled_buses = buses.iter().copied().collect();
        if buses.is_empty() {
            buses.push(0);
        }
        Self {
            settings,
            buses,
            enabled_buses,
            session: None,
            job: None,
            discard_job: false,
            closing: false,
            closed: false,
            published: Hardware::NotStarted,
            next: Instant::now(),
            terminal: false,
            reset: false,
            requires_boot: false,
            needs_quiet: false,
            needs_boundary: false,
            boundary: None,
            boot: None,
            version: None,
            observations: Vec::new(),
            error: String::new(),
            session_generation: 0,
            bus_generations: [0; 4],
            physical: None,
            completed: None,
            inputs: input::Router::default(),
        }
    }
    fn spawn(
        &mut self,
        kind: JobKind,
        future: impl std::future::Future<Output = Done> + Send + 'static,
    ) {
        assert!(self.job.is_none(), "only one owned HAT helper");
        self.job = Some(Job(tokio::spawn(future), kind));
    }
    pub(super) fn hardware(&self) -> Hardware {
        if self.settings.is_none() {
            return Hardware::NotStarted;
        }
        if self.closed {
            return Hardware::Closed;
        }
        if self.terminal || self.closing || (self.session.is_none() && self.job.is_none()) {
            return Hardware::Unavailable {
                reason: self.error.clone(),
                retrying: !self.terminal,
            };
        }
        let Some(session) = &self.session else {
            return Hardware::Connecting;
        };
        if self.needs_quiet || session.link.status() == SessionState::Resynchronizing {
            return Hardware::Resynchronizing;
        }
        match session.link.status() {
            SessionState::WaitingLateReply { remaining } => {
                Hardware::WaitingLateReply { remaining }
            }
            SessionState::ResetRequired => Hardware::ResetRequired,
            _ if self.reset => Hardware::ResetRequired,
            SessionState::Closed => Hardware::Unavailable {
                reason: "transport closed".into(),
                retrying: true,
            },
            SessionState::Uninitialized => Hardware::Initializing,
            _ => match &self.version {
                Some(version) => Hardware::Ready {
                    version: version.clone(),
                    buses: self.observations.clone(),
                },
                None => Hardware::Initializing,
            },
        }
    }
    /// Only internal pending work is polled. Dropping this future loses no owner.
    pub(super) async fn changed(&mut self) -> Result<(), String> {
        if self.settings.is_none() || self.terminal {
            // WAIT: v2-hat-loop
            return pending().await;
        }
        let mut progressed = self.expire_physical();
        // Preserve queued transport evidence without allowing renewed input to
        // postpone an actual helper join indefinitely. Each lag also uses budget.
        for _ in 0..=crate::atx::monitor::EVENT_CAPACITY {
            if self.closing {
                break;
            }
            let Some(session) = self.session.as_mut() else {
                break;
            };
            let event = match session.events.try_recv() {
                Ok(event) => Ok(event),
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    Err(broadcast::error::RecvError::Lagged(n))
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    Err(broadcast::error::RecvError::Closed)
                }
            };
            self.event(event)?;
            progressed = true;
        }
        if let Some(job) = self.job.as_mut().filter(|job| job.0.is_finished()) {
            // The hint never substitutes for the actual join. Keep ownership if
            // this future is canceled before the result becomes ready.
            // WAIT: v2-hat-loop
            let result = (&mut job.0).await;
            self.joined(result)?;
            progressed = true;
        }
        self.advance()?;
        if progressed || self.completed.is_some() || self.hardware() != self.published {
            self.published = self.hardware();
            return Ok(());
        }
        let wake = {
            let closing = self.closing;
            let timer = self.job.is_none()
                && (self.session.is_none()
                    || self.needs_boundary
                    || self.needs_quiet
                    || self.can_query());
            let job = &mut self.job;
            let physical = &mut self.physical;
            let (events, status) = match self.session.as_mut() {
                Some(s) => (Some(&mut s.events), Some(&mut s.status)),
                None => (None, None),
            };
            // WAIT: v2-hat-loop
            tokio::select! {
                // Prefer public transport evidence; owned close also handles
                // EOF that reaches this receiver after a Closed result/status.
                biased;
                _=async { match physical.as_mut() {
                    Some(p) if p.stage==physical::Stage::Pending=>{
                        let deadline=p.work.deadline();
                        // WAIT: v2-hat-loop
                        tokio::select! { biased; _=p.work.wait_for_receiver_close()=>{},_=tokio::time::sleep_until(deadline)=>{} }
                    },_=>dormant().await
                }}=>Wake::OutputExpired,
                event=async { match events { Some(events) if !closing => {
                    // WAIT: v2-hat-loop
                    events.recv().await
                }, _=>dormant().await } }=>Wake::Event(event),
                done=async { match job.as_mut() { Some(j)=>{
                    // WAIT: v2-hat-loop
                    (&mut j.0).await
                }, None=>dormant().await } }=>Wake::Job(done),
                _=async { match status { Some(status) if !closing=>{
                    // WAIT: v2-hat-loop
                    let _=status.changed().await;
                },_=>dormant().await } }=>Wake::Status,
                _=tokio::time::sleep_until(self.next),if timer=>Wake::Timer,
            }
        };
        match wake {
            Wake::Job(result) => {
                self.joined(result)?;
            }
            Wake::Event(event) => self.event(event)?,
            Wake::Status => {
                if self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.link.status() == SessionState::Closed)
                {
                    // Closure can reach watch/results before the public EOF.
                    // Only the actual owned join classifies teardown success.
                    self.begin_close();
                }
            }
            Wake::Timer => {}
            Wake::OutputExpired => {
                self.expire_physical();
            }
        }
        self.advance()?;
        self.published = self.hardware();
        Ok(())
    }
    fn joined(&mut self, result: Result<Done, tokio::task::JoinError>) -> Result<(), String> {
        self.job.take();
        if self.discard_job {
            self.discard_job = false;
            if let Err(error) = result {
                if !error.is_cancelled() {
                    return Err(format!("HAT helper failed: {error}"));
                }
            }
        } else {
            self.finished(result.map_err(|error| format!("HAT helper failed: {error}"))?)?;
        }
        Ok(())
    }
    fn can_query(&self) -> bool {
        !self.closing
            && !self.reset
            && !self.needs_quiet
            && self.session.as_ref().is_some_and(|s| {
                matches!(
                    s.link.status(),
                    SessionState::Ready | SessionState::Uninitialized
                )
            })
    }
    fn begin_close(&mut self) {
        self.error = "serial transport lost; resynchronizing after reopen".into();
        self.reset = true;
        self.version = None;
        self.observations.clear();
        self.boot = None;
        self.cancel_job();
        self.closing = true;
        if let Some(physical) = &mut self.physical {
            physical.stage = physical::Stage::Closing;
            physical
                .cleanup
                .get_or_insert_with(|| Instant::now() + Duration::from_secs(1));
        }
    }
    fn advance(&mut self) -> Result<(), String> {
        if self.job.is_some() || self.terminal || self.settings.is_none() {
            return Ok(());
        }
        if !self.closing
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.link.status() == SessionState::Closed)
        {
            self.begin_close();
        }
        if self.closing {
            let link = self.session.as_ref().expect("closing session").link.clone();
            let deadline = self
                .physical
                .as_ref()
                .and_then(|p| p.cleanup)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(1));
            self.spawn(JobKind::Close, async move {
                let result = link.close_until(deadline).await;
                let poisoned = link.write_is_poisoned();
                Done::Close(result, poisoned)
            });
            return Ok(());
        }
        if self.advance_physical()? {
            return Ok(());
        }
        if self.session.is_none() {
            if Instant::now() < self.next {
                return Ok(());
            }
            let settings = self.settings.as_ref().unwrap();
            let path = settings.path.clone();
            #[cfg(test)]
            let hook = settings.open_hook.clone();
            self.spawn(JobKind::Open, async move {
                #[cfg(test)]
                if let Some(hook) = hook {
                    return Done::Open(Serial::open_with_hook(path, hook).await);
                }
                Done::Open(Serial::open(path).await)
            });
            return Ok(());
        }
        let link = self.session.as_ref().unwrap().link.clone();
        self.requires_boot |= link.write_is_poisoned();
        if self.requires_boot {
            self.reset = true;
            self.needs_quiet = false;
        }
        if self.needs_boundary {
            if Instant::now() < self.next {
                return Ok(());
            }
            self.spawn(JobKind::Boundary, async move {
                Done::Boundary(link.fresh_boot_boundary(Duration::from_secs(1)).await)
            });
            return Ok(());
        }
        if self.reset && self.requires_boot {
            if self
                .boot
                .is_some_and(|sequence| self.boundary.is_none_or(|min| sequence >= min))
                && link.status() == SessionState::Uninitialized
            {
                self.reset = false;
                self.requires_boot = false;
                self.needs_quiet = true;
                self.boot = None;
                self.boundary = None;
                self.version = None;
                self.observations.clear();
                self.next = Instant::now();
            } else {
                return Ok(());
            }
        }
        if self.needs_quiet && !self.requires_boot {
            if Instant::now() < self.next {
                return Ok(());
            }
            let settings = self.settings.as_ref().unwrap();
            let quiet = settings.quiet;
            let within = settings.timing.setup + quiet;
            self.spawn(JobKind::Recover, async move {
                Done::Recover(link.resynchronize(quiet, within).await)
            });
            return Ok(());
        }
        match link.status() {
            SessionState::ResetRequired => {
                self.reset = true;
                if !self.requires_boot && !self.needs_quiet {
                    self.needs_quiet = true;
                    self.next = Instant::now() + self.settings.as_ref().unwrap().timing.retry;
                }
                self.version = None;
                self.observations.clear();
            }
            SessionState::WaitingLateReply { .. } => {
                if !self.needs_quiet {
                    self.needs_quiet = true;
                    self.next = Instant::now() + self.settings.as_ref().unwrap().timing.retry;
                }
            }
            SessionState::Resynchronizing
            | SessionState::Writing
            | SessionState::Awaiting { .. } => {}
            // Public events, a result, or the status branch starts owned close.
            SessionState::Closed => {}
            SessionState::Uninitialized | SessionState::Ready => {
                if Instant::now() < self.next {
                    return Ok(());
                }
                if self.version.is_some() && link.status() == SessionState::Ready {
                    let stamps = self.input_stamps();
                    let cuts = self.inputs.pending(|bus| stamps[usize::from(bus)]);
                    if !cuts.is_empty() {
                        self.next = Instant::now() + self.settings.as_ref().unwrap().timing.retry;
                        self.spawn(JobKind::InputBoundary, async move {
                            Done::InputBoundary(
                                cuts,
                                link.input_boundary(Duration::from_millis(500)).await,
                            )
                        });
                        return Ok(());
                    }
                }
                let timing = self.settings.as_ref().unwrap().timing;
                let buses = self.buses.clone();
                let initialize =
                    self.version.is_none() || link.status() == SessionState::Uninitialized;
                self.next = Instant::now() + timing.poll;
                self.spawn(JobKind::Query, async move {
                    let deadline = Instant::now() + timing.setup;
                    if initialize {
                        let setup = async {
                            let version = link.initialize(remaining(deadline)?).await?;
                            let buses = query_health(&link, &buses, deadline).await?;
                            Ok((version, buses))
                        };
                        // WAIT: v2-hat-setup
                        let result = setup.await;
                        Done::Setup(result)
                    } else {
                        Done::Poll(query_health(&link, &buses, deadline).await)
                    }
                });
            }
        }
        Ok(())
    }
    fn finished(&mut self, done: Done) -> Result<(), String> {
        let retry = self.settings.as_ref().unwrap().timing.retry;
        match done {
            Done::Open(Ok(serial)) => {
                self.session_generation = self.session_generation.saturating_add(1);
                let (link, events) =
                    Transactions::start(serial, self.settings.as_ref().unwrap().repeat);
                let status = link.observe_status();
                self.session = Some(Session {
                    link,
                    events,
                    status,
                });
                self.needs_boundary = self.requires_boot;
                self.needs_quiet = !self.requires_boot;
                self.boundary = None;
                self.boot = None;
                self.next = Instant::now();
            }
            Done::Open(Err(error)) => {
                self.terminal = error.kind() == io::ErrorKind::TimedOut;
                self.error = error.to_string();
                self.next = Instant::now() + retry;
            }
            Done::Setup(Ok((version, buses))) => {
                self.version = Some(version);
                self.fill_missing(buses);
                self.schedule_inputs();
            }
            Done::Poll(Ok(buses)) => {
                self.fill_missing(buses);
                self.schedule_inputs();
            }
            Done::Boundary(Ok(minimum)) => {
                self.boundary = Some(minimum);
                self.needs_boundary = false;
            }
            Done::Boundary(Err(error)) => {
                self.error = error.to_string();
                self.next = Instant::now() + retry;
            }
            Done::Recover(Ok(())) => {
                self.reset = false;
                self.needs_quiet = false;
                self.version = None;
                self.observations.clear();
                self.next = Instant::now();
            }
            Done::Recover(Err(error)) => {
                if error == TransactionError::Closed && !self.closing {
                    self.begin_close();
                    return Ok(());
                }
                self.error = error.to_string();
                self.reset = true;
                self.needs_quiet = true;
                self.next = Instant::now() + retry;
            }
            Done::Setup(Err(error)) | Done::Poll(Err(error)) => {
                if error == TransactionError::Closed && !self.closing {
                    self.begin_close();
                    return Ok(());
                }
                self.error = error.to_string();
                self.version = None;
                self.observations.clear();
                self.next = Instant::now() + retry;
            }
            Done::Close(result, poisoned) => {
                result.map_err(|error| format!("HAT close incomplete: {error}"))?;
                self.requires_boot |= poisoned;
                self.session = None;
                self.needs_quiet = false;
                self.closing = false;
                self.next = Instant::now() + retry;
                if self.physical.is_some() {
                    self.complete_physical(Some(
                        "transport closed before operation settlement".into(),
                    ));
                }
            }
            Done::Physical(outcome) => {
                let physical = self.physical.as_mut().expect("owned physical result");
                physical.outcome = Some(outcome);
                physical.stage = physical::Stage::Settling;
                physical
                    .cleanup
                    .get_or_insert_with(|| Instant::now() + Duration::from_secs(1));
            }
            Done::Settled(Ok(())) => self.complete_physical(None),
            Done::Settled(Err(_)) => self.begin_close(),
            Done::InputBoundary(cuts, Ok(minimum)) => {
                let stamps = self.input_stamps();
                self.inputs
                    .install(&cuts, minimum, |bus| stamps[usize::from(bus)]);
            }
            Done::InputBoundary(_, Err(error)) => {
                tracing::debug!(kind="input_boundary_pending",%error,"Input mapping awaits a receive boundary");
            }
        }
        Ok(())
    }
    fn event(&mut self, event: Result<Event, broadcast::error::RecvError>) -> Result<(), String> {
        match event {
            Ok(Event::Closed(CloseReason::Eof | CloseReason::Io(_))) => {
                // Inspect the canceled helper before starting close/recovery.
                self.begin_close();
            }
            Ok(Event::Closed(reason)) => {
                return Err(format!("HAT monitor unexpectedly stopped: {reason:?}"))
            }
            Ok(Event::Packet {
                sequence,
                packet: Ok(packet),
            }) => match packet.kind {
                Kind::Dali(crate::atx::wire::DaliFrame::Bits24(raw)) => {
                    let stamp = self.input_stamps()[usize::from(packet.bus)];
                    self.inputs.observe(packet.bus, sequence, raw, stamp);
                }
                Kind::Boot(_) => {
                    self.boot = Some(sequence);
                    self.version = None;
                    self.observations.clear();
                    self.cancel_query();
                    self.next = Instant::now();
                }
                Kind::BusStatus { status, channels } => {
                    if !self.observations.iter().any(|bus| {
                        bus.bus == packet.bus && bus.status == status && bus.channels == channels
                    }) {
                        if let Some(epoch) = self.bus_generations.get_mut(usize::from(packet.bus)) {
                            *epoch = epoch.saturating_add(1);
                        }
                    }
                    self.observations.retain(|value| value.bus != packet.bus);
                    self.observations.push(BusHealth {
                        bus: packet.bus,
                        status,
                        channels,
                    });
                    self.observations.sort_by_key(|value| value.bus);
                }
                _ => {}
            },
            Err(broadcast::error::RecvError::Lagged(_)) => {
                self.cancel_query();
                self.version = None;
                self.observations.clear();
                self.error = "HAT observation stream lagged; resynchronizing".into();
                self.next = Instant::now();
            }
            Err(broadcast::error::RecvError::Closed) => {
                return Err("HAT observation stream unexpectedly closed".into())
            }
            _ => {}
        }
        Ok(())
    }
    fn stamp(&self, bus: u8) -> Option<output::Stamp> {
        let session = self.session.as_ref()?;
        let actor = session.link.observation_epoch();
        let observed_bus = *self.bus_generations.get(usize::from(bus))?;
        let bus = session.link.bus_epoch(bus);
        (self.session_generation != u64::MAX
            && actor != u64::MAX
            && bus != u64::MAX
            && observed_bus != u64::MAX)
            .then_some(output::Stamp {
                session: self.session_generation,
                actor,
                bus,
                observed_bus,
            })
    }
    pub(super) fn enqueue_scene(
        &mut self,
        request: super::scenes::Request,
        route: super::scenes::Route,
    ) {
        self.enqueue_physical(physical::Work::Scenes(request, route));
    }
    pub(super) fn enqueue_scene_setting(
        &mut self,
        request: Box<super::scene_settings::Request>,
        route: super::scene_settings::Route,
    ) {
        if request.action.writing()
            && self
                .settings
                .as_ref()
                .is_none_or(|s| s.repeat == RepeatPolicy::Unverified)
        {
            super::scene_settings::finish(
                request,
                Err(ServiceError::Rejected(
                    "scene settings require --uart-repeat-terminals 1 or 2 for a known HAT profile"
                        .into(),
                )),
            );
            return;
        }
        self.enqueue_physical(physical::Work::SceneSettings(request, route));
    }
    pub(super) fn enqueue_groups(
        &mut self,
        request: super::groups::Request,
        route: super::groups::Route,
    ) {
        if request.action.writes_hardware()
            && self
                .settings
                .as_ref()
                .is_none_or(|s| s.repeat == RepeatPolicy::Unverified)
        {
            super::groups::finish(request,Err(ServiceError::Rejected("group programming requires --uart-repeat-terminals 1 or 2 for a known HAT profile".into())));
            return;
        }
        self.enqueue_physical(physical::Work::Groups(request, route));
    }
    pub(super) fn enqueue_commission(
        &mut self,
        request: super::commissioning::Request,
        route: super::commissioning::Route,
    ) {
        if route.space == super::discovery::Space::Outputs
            && self
                .settings
                .as_ref()
                .is_none_or(|s| s.repeat == RepeatPolicy::Unverified)
        {
            super::commissioning::finish(
                request,
                Err(ServiceError::Rejected(
                    "commissioning requires --uart-repeat-terminals 1 or 2 for a known HAT profile"
                        .into(),
                )),
            );
            return;
        }
        self.enqueue_physical(physical::Work::Commission(request, route));
    }
    pub(super) fn evidence_current(&self, bus: u8, stamp: output::Stamp) -> bool {
        self.stamp(bus) == Some(stamp)
            && !self.closing
            && !self.needs_quiet
            && !self.reset
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.link.status() == SessionState::Ready)
            && self
                .observations
                .iter()
                .any(|b| b.bus == bus && b.status == 2)
    }
    pub(super) fn enqueue_discovery(
        &mut self,
        request: super::discovery::Request,
        route: super::discovery::Route,
    ) {
        self.enqueue_physical(physical::Work::Discovery(request, route));
    }
    pub(super) fn reassignment_stamp(
        &self,
        bus: u8,
        space: super::discovery::Space,
    ) -> Result<output::Stamp, ServiceError> {
        if self.physical.is_some() || self.completed.is_some() {
            return Err(ServiceError::Busy);
        }
        if space == super::discovery::Space::Outputs
            && self
                .settings
                .as_ref()
                .is_none_or(|s| s.repeat == RepeatPolicy::Unverified)
        {
            return Err(ServiceError::Rejected(
                "reassignment requires --uart-repeat-terminals 1 or 2 for a known HAT profile"
                    .into(),
            ));
        }
        let stamp = self
            .stamp(bus)
            .ok_or_else(|| ServiceError::Rejected("UART evidence unavailable".into()))?;
        if !matches!(self.hardware(), Hardware::Ready { .. }) || !self.evidence_current(bus, stamp)
        {
            return Err(ServiceError::Rejected("UART or bus is not ready".into()));
        }
        Ok(stamp)
    }
    pub(super) fn enqueue_reassignment(
        &mut self,
        request: super::reassignment::Request,
        route: super::reassignment::Route,
    ) {
        self.enqueue_physical(physical::Work::Reassignment(request, route));
    }
    pub(super) fn enqueue_output(&mut self, request: output::Request, route: output::Route) {
        self.enqueue_physical(physical::Work::Output(request, route));
    }
    pub(super) fn enqueue_input(
        &mut self,
        request: input::readback::Request,
        route: input::readback::Route,
    ) {
        self.enqueue_physical(physical::Work::Input(request, route));
    }
    fn enqueue_physical(&mut self, work: physical::Work) {
        let error = if self.physical.is_some() || self.completed.is_some() {
            Some(ServiceError::Busy)
        } else if !matches!(self.hardware(), Hardware::Ready { .. })
            || self.closing
            || !self
                .observations
                .iter()
                .any(|bus| bus.bus == work.bus() && bus.status == 2)
        {
            Some(ServiceError::Rejected("UART or bus is not ready".into()))
        } else {
            None
        };
        if let Some(error) = error {
            work.reject(error);
            return;
        }
        let Some(stamp) = self.stamp(work.bus()) else {
            work.reject(ServiceError::Rejected(
                "UART evidence is exhausted or unavailable".into(),
            ));
            return;
        };
        if work
            .required_stamp()
            .is_some_and(|expected| expected != stamp)
        {
            work.reject(ServiceError::Rejected(
                "commissioning candidate physical epoch changed".into(),
            ));
            return;
        }
        self.physical = Some(physical::Active {
            work,
            stamp,
            stage: physical::Stage::Pending,
            outcome: None,
            cleanup: None,
        });
    }
    fn advance_physical(&mut self) -> Result<bool, String> {
        let Some(physical) = self.physical.as_ref() else {
            return Ok(false);
        };
        match physical.stage {
            physical::Stage::Pending => {
                let reason =
                    if physical.work.is_closed() || Instant::now() >= physical.work.deadline() {
                        Some("request expired before hardware admission")
                    } else if self.stamp(physical.work.bus()) != Some(physical.stamp)
                        || !self.can_query()
                        || self
                            .session
                            .as_ref()
                            .is_none_or(|s| s.link.status() != SessionState::Ready)
                        || !self
                            .observations
                            .iter()
                            .any(|bus| bus.bus == physical.work.bus() && bus.status == 2)
                    {
                        Some("request's captured UART or bus epoch is no longer ready")
                    } else {
                        None
                    };
                if let Some(reason) = reason {
                    let physical = self.physical.take().unwrap();
                    self.completed = Some(
                        physical
                            .work
                            .finish(None, Some(ServiceError::Rejected(reason.into()))),
                    );
                    return Ok(false);
                }
                let physical = self.physical.as_mut().unwrap();
                physical.stage = physical::Stage::Running;
                if let physical::Work::Input(request, _) = &mut physical.work {
                    request.native_sequence = self.inputs.native_sequence(&request.target);
                }
                let link = self.session.as_ref().unwrap().link.clone();
                let span = physical.work.span();
                let execution = physical.work.execution(link, physical.stamp);
                self.spawn(
                    JobKind::Physical,
                    async move {
                        // WAIT: v2-physical-operation
                        Done::Physical(execution.await)
                    }
                    .instrument(span),
                );
            }
            physical::Stage::Settling => {
                let link = self.session.as_ref().unwrap().link.clone();
                let deadline = physical.cleanup.expect("one physical cleanup ceiling");
                self.spawn(JobKind::Settle, async move {
                    Done::Settled(link.settle_until(deadline).await)
                });
            }
            physical::Stage::Running => return Err("physical helper ownership disappeared".into()),
            physical::Stage::Closing => {
                return Err("physical teardown ownership disappeared".into())
            }
        }
        Ok(true)
    }
    fn expire_physical(&mut self) -> bool {
        if self.physical.as_ref().is_some_and(|p| {
            p.stage == physical::Stage::Pending
                && (p.work.is_closed() || Instant::now() >= p.work.deadline())
        }) {
            let physical = self.physical.take().unwrap();
            self.completed = Some(physical.work.finish(None, Some(ServiceError::Expired)));
            true
        } else {
            false
        }
    }
    fn complete_physical(&mut self, reason: Option<String>) {
        let physical = self.physical.take().expect("owned physical completion");
        let stale = reason.or_else(|| {
            (self.stamp(physical.work.bus()) != Some(physical.stamp)
                || self.closing
                || self.needs_quiet
                || self.reset
                || self
                    .session
                    .as_ref()
                    .is_none_or(|s| s.link.status() != SessionState::Ready)
                || !self
                    .observations
                    .iter()
                    .any(|bus| bus.bus == physical.work.bus() && bus.status == 2))
            .then(|| "UART or bus observation epoch changed".into())
        });
        let (mut sample, mut error) = match physical.outcome {
            Some(mut outcome) => {
                if outcome.recovery {
                    self.needs_quiet = true;
                    self.next = Instant::now();
                }
                if let Some(reason) = &stale {
                    outcome.sample.invalidate(reason);
                }
                (Some(outcome.sample), outcome.error)
            }
            None => (None, None),
        };
        if let Some(reason) = stale {
            error = Some(if physical.work.mutating() {
                ServiceError::Uncertain(reason)
            } else {
                ServiceError::Rejected(reason)
            });
        }
        if let (physical::Work::Input(request, _), Some(physical::Sample::Input(observation))) =
            (&physical.work, &mut sample)
        {
            self.inputs
                .accept_query(&request.target, request.native_sequence, observation);
        }
        self.completed = Some(physical.work.finish(sample, error));
    }
    pub(super) fn take_completed_physical(&mut self) -> Option<physical::Completion> {
        self.completed.take()
    }
    #[cfg(test)]
    fn take_completed_output(&mut self) -> Option<output::Completion> {
        self.take_completed_physical().map(|value| match value {
            physical::Completion::Output(output) => output,
            _ => panic!("output fixture received an input completion"),
        })
    }
    #[cfg(test)]
    pub(super) fn power_cycle_output_samples_for_test(&mut self) {
        for status in [0, 2] {
            self.event(Ok(Event::Packet {
                sequence: u64::MAX,
                packet: Ok(crate::atx::wire::Packet {
                    bus: 0,
                    kind: Kind::BusStatus {
                        status,
                        channels: 4,
                    },
                }),
            }))
            .unwrap();
        }
    }
    pub(super) fn invalidate_outputs(
        &self,
        outputs: &mut std::collections::BTreeMap<String, output::Observation>,
    ) {
        for observation in outputs.values_mut() {
            self.invalidate_output(observation);
        }
    }
    pub(super) fn invalidate_discovery(&self, observation: &mut super::discovery::Observation) {
        observation.stale |= self.stamp(observation.bus) != Some(observation.stamp)
            || self.closing
            || self.needs_quiet
            || self
                .session
                .as_ref()
                .is_none_or(|s| s.link.status() != SessionState::Ready)
            || !self
                .observations
                .iter()
                .any(|b| b.bus == observation.bus && b.status == 2);
    }
    pub(super) fn invalidate_output(&self, observation: &mut output::Observation) {
        if self.stamp(observation.bus) != Some(observation.stamp)
            || self.closing
            || self.needs_quiet
            || self.session.as_ref().is_none_or(|s| {
                matches!(
                    s.link.status(),
                    SessionState::Closed
                        | SessionState::ResetRequired
                        | SessionState::WaitingLateReply { .. }
                        | SessionState::Resynchronizing
                )
            })
            || !self
                .observations
                .iter()
                .any(|bus| bus.bus == observation.bus && bus.status == 2)
        {
            observation.invalidate("UART or bus observation ordering changed");
        }
    }
    fn cancel_query(&mut self) {
        // A receive fence is still valid across Boot: its returned minimum
        // decides whether that Boot was fresh. Only query observations expire.
        if self
            .job
            .as_ref()
            .is_some_and(|job| matches!(job.1, JobKind::Query | JobKind::InputBoundary))
        {
            self.cancel_job();
        }
    }
    fn cancel_job(&mut self) {
        if let Some(job) = &self.job {
            self.discard_job = true;
            job.0.abort();
        }
    }
    fn fill_missing(&mut self, buses: Vec<BusHealth>) {
        // The public stream was drained before this result. Never overwrite a
        // later proactive observation with the helper's earlier snapshot.
        for bus in buses {
            if !self
                .observations
                .iter()
                .any(|observed| observed.bus == bus.bus)
            {
                self.observations.push(bus);
            }
        }
        self.observations.sort_by_key(|bus| bus.bus);
    }
    fn input_stamps(&self) -> [Option<output::Stamp>; 4] {
        std::array::from_fn(|bus| {
            (!self.reset
                && !self.closing
                && !self.needs_quiet
                && matches!(self.hardware(),Hardware::Ready {ref buses,..}
                    if buses.iter().any(|b|usize::from(b.bus)==bus && b.status==2)))
            .then(|| self.stamp(bus as u8))
            .flatten()
        })
    }
    pub(super) fn sync_model(&mut self, model: &crate::inventory::Model) {
        let enabled: std::collections::BTreeSet<_> = model
            .inventory()
            .buses
            .iter()
            .filter(|bus| bus.enabled)
            .map(|bus| bus.bus)
            .collect();
        if self.enabled_buses != enabled {
            let affected: std::collections::BTreeSet<_> = self
                .enabled_buses
                .symmetric_difference(&enabled)
                .copied()
                .collect();
            for bus in &affected {
                self.bus_generations[usize::from(*bus)] =
                    self.bus_generations[usize::from(*bus)].saturating_add(1);
            }
            self.observations.retain(|bus| !affected.contains(&bus.bus));
            self.buses = enabled.iter().copied().collect();
            if self.buses.is_empty() {
                self.buses.push(0);
            }
            self.enabled_buses = enabled;
            // Own the join/discard before the replacement poll; physical work
            // cannot overlap metadata because its global permit is still held.
            self.cancel_query();
            self.next = Instant::now();
        }
        if self.inputs.sync(model) {
            self.schedule_inputs();
        }
    }
    fn schedule_inputs(&mut self) {
        let stamps = self.input_stamps();
        if !self
            .inputs
            .pending(|bus| stamps[usize::from(bus)])
            .is_empty()
        {
            self.next = Instant::now();
        }
    }
    pub(super) fn input_observations(
        &mut self,
    ) -> std::collections::BTreeMap<String, input::Observation> {
        let stamps = self.input_stamps();
        self.inputs.invalidate(|bus| stamps[usize::from(bus)]);
        self.inputs.observations.clone()
    }
    pub(super) fn take_input_notices(&mut self) -> (Vec<input::Notice>, u64) {
        self.inputs.take_notices()
    }
    pub(super) fn input_listeners(&self) -> std::collections::BTreeSet<String> {
        let stamps = self.input_stamps();
        self.inputs.listeners(|bus| stamps[usize::from(bus)])
    }
    pub(super) async fn close(&mut self, deadline: Instant) -> Result<(), String> {
        // Abort and actually join the helper before dropping its parent session.
        if let Some(mut job) = self.job.take() {
            job.0.abort();
            // WAIT: v2-hat-close
            let result = timeout_at(deadline, &mut job.0)
                .await
                .map_err(|_| "HAT helper did not finish shutdown")?;
            if let Err(error) = result {
                if error.is_panic() {
                    return Err(format!("HAT helper panicked during shutdown: {error}"));
                }
            }
        }
        if let Some(session) = self.session.as_ref() {
            session
                .link
                .close_until(deadline)
                .await
                .map_err(|error| format!("HAT shutdown: {error}"))?;
        }
        self.session = None;
        if let Some(physical) = self.physical.take() {
            physical.work.reject(ServiceError::Closed);
        }
        if let Some(completed) = self.completed.take() {
            completed.reject();
        }
        self.closed = true;
        self.terminal = true;
        self.error = "closed".into();
        if Instant::now() >= deadline {
            return Err("HAT shutdown exceeded deadline".into());
        }
        Ok(())
    }
}
fn remaining(deadline: Instant) -> Result<Duration, TransactionError> {
    let duration = deadline.saturating_duration_since(Instant::now());
    if duration.is_zero() {
        Err(TransactionError::Deadline)
    } else {
        Ok(duration)
    }
}
async fn query_health(
    link: &Transactions<Serial>,
    buses: &[u8],
    deadline: Instant,
) -> Result<Vec<BusHealth>, TransactionError> {
    let mut result = Vec::new();
    for &bus in buses {
        // WAIT: v2-hat-health
        match link
            .execute(Command::BusStatus { bus }, remaining(deadline)?)
            .await?
        {
            Response::Status { status, channels } => result.push(BusHealth {
                bus,
                status,
                channels,
            }),
            _ => return Err(TransactionError::StreamLoss),
        }
    }
    Ok(result)
}
async fn dormant<T>() -> T {
    // WAIT: v2-hat-loop
    pending().await
}

#[cfg(test)]
pub(super) mod tests;
