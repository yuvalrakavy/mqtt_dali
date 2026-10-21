//! UART lifecycle only. Fixture operations must join the service admission path.
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

pub(super) struct Settings {
    path: PathBuf,
    repeat: RepeatPolicy,
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
    Query,
    Close,
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
    Close(Result<(), TransactionError>),
}
struct Session {
    link: Transactions<Serial>,
    events: broadcast::Receiver<Event>,
    status: watch::Receiver<SessionState>,
}
pub(super) struct Hat {
    settings: Option<Settings>,
    buses: Vec<u8>,
    session: Option<Session>,
    job: Option<Job>,
    discard_job: bool,
    closing: bool,
    closed: bool,
    published: Hardware,
    next: Instant,
    terminal: bool,
    reset: bool,
    needs_boundary: bool,
    boundary: Option<u64>,
    boot: Option<u64>,
    version: Option<Version>,
    observations: Vec<BusHealth>,
    error: String,
}
enum Wake {
    Job(Result<Done, tokio::task::JoinError>),
    Event(Result<Event, broadcast::error::RecvError>),
    Status,
    Timer,
}
impl Hat {
    pub(super) fn new(settings: Option<Settings>, mut buses: Vec<u8>) -> Self {
        if buses.is_empty() {
            buses.push(0);
        }
        Self {
            settings,
            buses,
            session: None,
            job: None,
            discard_job: false,
            closing: false,
            closed: false,
            published: Hardware::NotStarted,
            next: Instant::now(),
            terminal: false,
            reset: false,
            needs_boundary: false,
            boundary: None,
            boot: None,
            version: None,
            observations: Vec::new(),
            error: String::new(),
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
        self.advance()?;
        if self.hardware() != self.published {
            self.published = self.hardware();
            return Ok(());
        }
        let wake = {
            let closing = self.closing;
            let timer = self.job.is_none()
                && (self.session.is_none() || self.needs_boundary || self.can_query());
            let job = &mut self.job;
            let (events, status) = match self.session.as_mut() {
                Some(s) => (Some(&mut s.events), Some(&mut s.status)),
                None => (None, None),
            };
            // WAIT: v2-hat-loop
            tokio::select! {
                // Public transport evidence must be consumed before classifying
                // a Closed status or the command helper's Closed error.
                biased;
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
            }
            Wake::Event(event) => self.event(event)?,
            Wake::Status => {
                if self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.link.status() == SessionState::Closed)
                {
                    // The biased public branch above has consumed every already
                    // queued event; no normal transport close was observed.
                    return Err(
                        "HAT transaction actor closed without transport-loss evidence".into(),
                    );
                }
            }
            Wake::Timer => {}
        }
        self.advance()?;
        self.published = self.hardware();
        Ok(())
    }
    fn can_query(&self) -> bool {
        !self.closing
            && !self.reset
            && self.session.as_ref().is_some_and(|s| {
                matches!(
                    s.link.status(),
                    SessionState::Ready | SessionState::Uninitialized
                )
            })
    }
    fn advance(&mut self) -> Result<(), String> {
        if self.job.is_some() || self.terminal || self.settings.is_none() {
            return Ok(());
        }
        if self.closing {
            let link = self.session.as_ref().expect("closing session").link.clone();
            self.spawn(JobKind::Close, async move {
                // WAIT: v2-hat-close
                let result = link.close().await;
                Done::Close(result)
            });
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
        if self.needs_boundary {
            if Instant::now() < self.next {
                return Ok(());
            }
            self.spawn(JobKind::Boundary, async move {
                Done::Boundary(link.fresh_boot_boundary(Duration::from_secs(1)).await)
            });
            return Ok(());
        }
        if self.reset {
            if self
                .boot
                .is_some_and(|sequence| self.boundary.is_none_or(|min| sequence >= min))
                && link.status() == SessionState::Uninitialized
            {
                self.reset = false;
                self.boot = None;
                self.boundary = None;
                self.version = None;
                self.observations.clear();
                self.next = Instant::now();
            } else {
                return Ok(());
            }
        }
        match link.status() {
            SessionState::ResetRequired => {
                self.reset = true;
                self.version = None;
                self.observations.clear();
            }
            SessionState::WaitingLateReply { .. }
            | SessionState::Writing
            | SessionState::Awaiting { .. } => {}
            // Let the biased public event branch consume transport evidence
            // before the status branch classifies an unexplained actor exit.
            SessionState::Closed => {}
            SessionState::Uninitialized | SessionState::Ready => {
                if Instant::now() < self.next {
                    return Ok(());
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
                let (link, events) =
                    Transactions::start(serial, self.settings.as_ref().unwrap().repeat);
                let status = link.observe_status();
                self.session = Some(Session {
                    link,
                    events,
                    status,
                });
                self.needs_boundary = self.reset;
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
            }
            Done::Poll(Ok(buses)) => self.fill_missing(buses),
            Done::Boundary(Ok(minimum)) => {
                self.boundary = Some(minimum);
                self.needs_boundary = false;
            }
            Done::Boundary(Err(error)) => {
                self.error = error.to_string();
                self.next = Instant::now() + retry;
            }
            Done::Setup(Err(error)) | Done::Poll(Err(error)) => {
                if error == TransactionError::Closed && !self.closing {
                    return Err("HAT request owner unexpectedly closed".into());
                }
                self.error = error.to_string();
                self.version = None;
                self.observations.clear();
                self.next = Instant::now() + retry;
            }
            Done::Close(result) => {
                result.map_err(|error| format!("HAT close incomplete: {error}"))?;
                self.session = None;
                self.closing = false;
                self.next = Instant::now() + retry;
            }
        }
        Ok(())
    }
    fn event(&mut self, event: Result<Event, broadcast::error::RecvError>) -> Result<(), String> {
        match event {
            Ok(Event::Closed(CloseReason::Eof | CloseReason::Io(_))) => {
                self.error = "serial transport lost; fresh boot required after reopen".into();
                self.reset = true;
                self.version = None;
                self.observations.clear();
                self.boot = None;
                // Inspect the canceled helper before starting close/recovery.
                self.cancel_job();
                self.closing = true;
            }
            Ok(Event::Closed(reason)) => {
                return Err(format!("HAT monitor unexpectedly stopped: {reason:?}"))
            }
            Ok(Event::Packet {
                sequence,
                packet: Ok(packet),
            }) => match packet.kind {
                Kind::Boot(_) => {
                    self.boot = Some(sequence);
                    self.version = None;
                    self.observations.clear();
                    self.cancel_query();
                    self.next = Instant::now();
                }
                Kind::BusStatus { status, channels } => {
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
    fn cancel_query(&mut self) {
        // A receive fence is still valid across Boot: its returned minimum
        // decides whether that Boot was fresh. Only query observations expire.
        if self.job.as_ref().is_some_and(|job| job.1 == JobKind::Query) {
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
            // WAIT: v2-hat-close
            timeout_at(deadline, session.link.close())
                .await
                .map_err(|_| "HAT shutdown deadline")?
                .map_err(|error| format!("HAT shutdown: {error}"))?;
        }
        self.session = None;
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
mod tests;
