use super::{BusReply, RepeatPolicy, Response, SessionState, TransactionError};
use crate::atx::monitor::{Event, Monitor};
use crate::atx::stream::DrainRead;
use crate::atx::wire::{Command, DaliFrame, Kind, Packet, Version};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::AsyncWrite;
use tokio::sync::{broadcast, mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore};
use tokio::task::AbortHandle;
use tokio::time::{timeout_at, Instant};

struct Request {
    command: Command,
    initializing: bool,
    deadline: Instant,
    reply: oneshot::Sender<Result<Response, TransactionError>>,
    permit: OwnedSemaphorePermit,
}
struct Shared<S> {
    monitor: Monitor<S>,
    requests: mpsc::Sender<Request>,
    permit: Arc<Semaphore>,
    status: watch::Receiver<SessionState>,
    stop: watch::Sender<bool>,
    completed: watch::Receiver<bool>,
    worker: AbortHandle,
    repeat: RepeatPolicy,
}
impl<S> Drop for Shared<S> {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.worker.abort();
    }
}

/// Clones share the same actor and permit; the raw monitor is never exposed.
pub struct Transactions<S> {
    shared: Arc<Shared<S>>,
}
impl<S> Clone for Transactions<S> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}
struct Life {
    status: watch::Sender<SessionState>,
    completed: watch::Sender<bool>,
}
impl Drop for Life {
    fn drop(&mut self) {
        self.status.send_replace(SessionState::Closed);
        self.completed.send_replace(true);
    }
}
struct Pending {
    command: Command,
    initializing: bool,
    minimum: u64,
    remaining: u8,
    replies: Vec<BusReply>,
    result: Option<Response>,
}
enum Mode {
    Uninitialized,
    Ready,
    Late(Pending),
    Reset { after: u64 },
    Closed,
}
struct Actor<S> {
    monitor: Monitor<S>,
    events: broadcast::Receiver<Event>,
    mode: Mode,
    requests: mpsc::Receiver<Request>,
    stop: watch::Receiver<bool>,
    life: Life,
    repeat: RepeatPolicy,
}

impl<S: DrainRead + AsyncWrite + Unpin + Send + 'static> Transactions<S> {
    pub fn start(stream: S, repeat: RepeatPolicy) -> (Self, broadcast::Receiver<Event>) {
        let (monitor, events) = Monitor::start(stream);
        let public = monitor.subscribe();
        let (send, receive) = mpsc::channel(1);
        let (status_send, status) = watch::channel(SessionState::Uninitialized);
        let (stop, stop_receive) = watch::channel(false);
        let (complete, completed) = watch::channel(false);
        let actor = Actor {
            monitor: monitor.clone(),
            events,
            mode: Mode::Uninitialized,
            requests: receive,
            stop: stop_receive,
            life: Life {
                status: status_send,
                completed: complete,
            },
            repeat,
        };
        let task = tokio::spawn(actor.run());
        let worker = task.abort_handle();
        drop(task);
        (
            Self {
                shared: Arc::new(Shared {
                    monitor,
                    requests: send,
                    permit: Arc::new(Semaphore::new(1)),
                    status,
                    stop,
                    completed,
                    worker,
                    repeat,
                }),
            },
            public,
        )
    }
    pub fn status(&self) -> SessionState {
        *self.shared.status.borrow()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.shared.monitor.subscribe()
    }
    pub fn observe_status(&self) -> watch::Receiver<SessionState> {
        self.shared.status.clone()
    }
    /// Exclude already-readable boot records on a reopened descriptor. This
    /// receives only; it neither rearms poison nor establishes a physical reset.
    pub(crate) async fn fresh_boot_boundary(
        &self,
        within: Duration,
    ) -> Result<u64, TransactionError> {
        let deadline = Instant::now()
            .checked_add(within)
            .ok_or(TransactionError::Deadline)?;
        let _permit = self
            .shared
            .permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| TransactionError::Busy)?;
        if self.status() != SessionState::Uninitialized {
            return Err(TransactionError::Desynchronized);
        }
        let fence = self.shared.monitor.fence(within).await?;
        if Instant::now() >= deadline {
            return Err(TransactionError::Deadline);
        }
        if self.status() != SessionState::Uninitialized {
            return Err(TransactionError::Desynchronized);
        }
        Ok(fence.minimum_sequence)
    }
    pub async fn initialize(&self, within: Duration) -> Result<Version, TransactionError> {
        match self.submit(Command::Version, true, within).await? {
            Response::Version(version) => Ok(version),
            _ => Err(TransactionError::StreamLoss),
        }
    }
    pub async fn execute(
        &self,
        command: Command,
        within: Duration,
    ) -> Result<Response, TransactionError> {
        self.submit(command, false, within).await
    }
    async fn submit(
        &self,
        command: Command,
        initializing: bool,
        within: Duration,
    ) -> Result<Response, TransactionError> {
        let deadline = Instant::now()
            .checked_add(within)
            .ok_or(TransactionError::Deadline)?;
        command
            .encode()
            .map_err(crate::atx::monitor::TransportError::from)?;
        terminal_count(&command, self.shared.repeat)?;
        let permit = Arc::clone(&self.shared.permit)
            .try_acquire_owned()
            .map_err(|_| TransactionError::Busy)?;
        admission(self.status(), initializing)?;
        let (send, receive) = oneshot::channel();
        self.shared
            .requests
            .try_send(Request {
                command,
                initializing,
                deadline,
                reply: send,
                permit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => TransactionError::Busy,
                mpsc::error::TrySendError::Closed(_) => TransactionError::Closed,
            })?;
        // WAIT: atx-transaction-result
        let result = timeout_at(deadline, receive)
            .await
            .map_err(|_| TransactionError::Deadline)?
            .map_err(|_| TransactionError::Closed)?;
        if Instant::now() >= deadline {
            return Err(TransactionError::Deadline);
        }
        result
    }
    pub async fn close(&self) -> Result<(), TransactionError> {
        self.shared.stop.send_replace(true);
        let mut completed = self.shared.completed.clone();
        // WAIT: atx-transaction-close
        tokio::time::timeout(Duration::from_secs(1), async {
            self.shared.monitor.close().await?;
            while !*completed.borrow_and_update() {
                completed
                    .changed()
                    .await
                    .map_err(|_| TransactionError::Closed)?;
            }
            Ok(())
        })
        .await
        .map_err(|_| TransactionError::Deadline)?
    }
}

fn admission(status: SessionState, initializing: bool) -> Result<(), TransactionError> {
    match status {
        SessionState::Ready => Ok(()),
        SessionState::Uninitialized if initializing => Ok(()),
        SessionState::Uninitialized => Err(TransactionError::NeedsInitialization),
        SessionState::ResetRequired | SessionState::WaitingLateReply { .. } => {
            Err(TransactionError::Desynchronized)
        }
        SessionState::Closed => Err(TransactionError::Closed),
        _ => Err(TransactionError::Busy),
    }
}
fn terminal_count(command: &Command, policy: RepeatPolicy) -> Result<u8, TransactionError> {
    match command {
        Command::Send {
            frame: DaliFrame::Bits8(_),
            ..
        } => Err(TransactionError::UnsupportedMode),
        Command::Send { repeat: true, .. } => match policy {
            RepeatPolicy::Unverified => Err(TransactionError::UnsupportedMode),
            RepeatPolicy::OneTerminal => Ok(1),
            RepeatPolicy::TwoTerminals => Ok(2),
        },
        _ => Ok(1),
    }
}
async fn stop_requested(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow_and_update() {
        // WAIT: atx-transaction-stop
        if stop.changed().await.is_err() {
            return;
        }
    }
}

impl<S: DrainRead + AsyncWrite + Unpin + Send + 'static> Actor<S> {
    fn publish_mode(&self) {
        self.life.status.send_replace(match &self.mode {
            Mode::Uninitialized => SessionState::Uninitialized,
            Mode::Ready => SessionState::Ready,
            Mode::Late(pending) => SessionState::WaitingLateReply {
                remaining: pending.remaining,
            },
            Mode::Reset { .. } => SessionState::ResetRequired,
            Mode::Closed => SessionState::Closed,
        });
    }
    fn reset(&mut self, after: u64) {
        self.mode = Mode::Reset { after };
    }
    async fn run(mut self) {
        loop {
            if *self.stop.borrow() {
                return;
            }
            // WAIT: atx-transaction-dispatch
            tokio::select! {
                _ = stop_requested(&mut self.stop) => return,
                event = self.events.recv() => { self.idle_event(event); self.publish_mode(); },
                request = self.requests.recv() => {
                    let Some(mut request) = request else { return; };
                    let result = self.request(&mut request).await;
                    self.publish_mode();
                    let Request { reply, permit, .. } = request;
                    drop(permit); // Retire admission before waking the caller.
                    let _ = reply.send(result);
                },
            }
            if matches!(self.mode, Mode::Closed) {
                return;
            }
        }
    }
    fn idle_event(&mut self, event: Result<Event, broadcast::error::RecvError>) {
        let mut late = match std::mem::replace(&mut self.mode, Mode::Uninitialized) {
            Mode::Late(pending) => Some(pending),
            mode => {
                self.mode = mode;
                None
            }
        };
        if let Some(pending) = late.as_mut() {
            match self.active_event(event, pending) {
                Ok(()) if pending.remaining == 0 => {
                    self.mode = if pending.initializing {
                        Mode::Uninitialized
                    } else {
                        Mode::Ready
                    }
                }
                Ok(()) => self.mode = Mode::Late(late.take().unwrap()),
                Err(_) => {}
            }
            return;
        }
        match event {
            Ok(Event::Packet {
                sequence,
                packet:
                    Ok(Packet {
                        kind: Kind::Boot(_),
                        ..
                    }),
            }) => {
                if !matches!(self.mode, Mode::Reset { after } if sequence < after) {
                    self.monitor.rearm_after_boot();
                    self.mode = Mode::Uninitialized;
                }
            }
            Ok(Event::Closed(_)) | Err(broadcast::error::RecvError::Closed) => {
                self.mode = Mode::Closed
            }
            Err(broadcast::error::RecvError::Lagged(_))
            | Ok(Event::Packet { packet: Err(_), .. }) => {
                self.reset(self.monitor.receive_sequence() + 1)
            }
            Ok(Event::WriteIndeterminate) if !matches!(self.mode, Mode::Reset { .. }) => {
                self.reset(self.monitor.receive_sequence() + 1)
            }
            _ => {}
        }
    }
    fn active_event(
        &mut self,
        event: Result<Event, broadcast::error::RecvError>,
        pending: &mut Pending,
    ) -> Result<(), TransactionError> {
        match event {
            Err(broadcast::error::RecvError::Lagged(_)) => {
                self.reset(self.monitor.receive_sequence() + 1);
                Err(TransactionError::StreamLoss)
            }
            Err(broadcast::error::RecvError::Closed) | Ok(Event::Closed(_)) => {
                self.mode = Mode::Closed;
                Err(TransactionError::Closed)
            }
            Ok(Event::WriteIndeterminate) => Ok(()),
            Ok(Event::Packet { sequence, packet }) => {
                let packet = match packet {
                    Ok(packet) => packet,
                    Err(_) => {
                        self.reset(sequence + 1);
                        return Err(TransactionError::StreamLoss);
                    }
                };
                if matches!(packet.kind, Kind::Boot(_)) {
                    self.mode = Mode::Uninitialized;
                    return Err(TransactionError::Reboot);
                }
                if matches!(packet.kind, Kind::UnattributedCollision) {
                    self.reset(sequence + 1);
                    return Err(TransactionError::StreamLoss);
                }
                if interferes(&pending.command, &packet) {
                    self.reset(sequence + 1);
                    return Err(TransactionError::Interference);
                }
                if sequence < pending.minimum {
                    return Ok(());
                }
                if let Some(response) = response(&pending.command, &packet) {
                    if pending.remaining == 0 {
                        self.reset(sequence + 1);
                        return Err(TransactionError::StreamLoss);
                    }
                    pending.remaining -= 1;
                    match response {
                        Response::Dali(mut replies) => pending.replies.append(&mut replies),
                        response => pending.result = Some(response),
                    }
                    if pending.remaining == 0 && matches!(pending.command, Command::Send { .. }) {
                        pending.result = Some(Response::Dali(pending.replies.clone()));
                    }
                }
                Ok(())
            }
        }
    }
    fn drain(&mut self, deadline: Instant) -> Result<(), TransactionError> {
        for _ in 0..256 {
            if Instant::now() >= deadline {
                return Err(TransactionError::Deadline);
            }
            match self.events.try_recv() {
                Err(broadcast::error::TryRecvError::Empty) => return Ok(()),
                Err(broadcast::error::TryRecvError::Closed) => {
                    self.idle_event(Err(broadcast::error::RecvError::Closed))
                }
                Err(broadcast::error::TryRecvError::Lagged(count)) => {
                    self.idle_event(Err(broadcast::error::RecvError::Lagged(count)))
                }
                Ok(event) => self.idle_event(Ok(event)),
            }
        }
        Err(TransactionError::Busy)
    }
    async fn request(&mut self, request: &mut Request) -> Result<Response, TransactionError> {
        if request.reply.is_closed() {
            return Err(TransactionError::Canceled);
        }
        self.drain(request.deadline)?;
        self.publish_mode();
        admission(*self.life.status.borrow(), request.initializing)?;
        if Instant::now() >= request.deadline {
            return Err(TransactionError::Deadline);
        }
        let fence = self
            .monitor
            .fence(request.deadline.saturating_duration_since(Instant::now()))
            .await?;
        self.drain(request.deadline)?;
        self.publish_mode();
        admission(*self.life.status.borrow(), request.initializing)?;
        if request.reply.is_closed() {
            return Err(TransactionError::Canceled);
        }
        let mut pending = Pending {
            command: request.command.clone(),
            initializing: request.initializing,
            minimum: fence.minimum_sequence,
            remaining: terminal_count(&request.command, self.repeat)?,
            replies: Vec::new(),
            result: None,
        };
        let started = Arc::new(AtomicU64::new(u64::MAX));
        self.life.status.send_replace(SessionState::Writing);
        // The helper drops the write future before returning. Cancellation is
        // stamped only afterward, including boots already buffered at that time.
        // WAIT: atx-transaction-write
        let written = self
            .write(request, &mut pending, Arc::clone(&started))
            .await;
        if let Err(error) = written {
            if started.load(Ordering::Acquire) != u64::MAX && !matches!(self.mode, Mode::Closed) {
                self.reset(self.monitor.receive_sequence() + 1);
            }
            return Err(error);
        }
        let result = self.await_response(request, &mut pending).await;
        match result {
            Ok(response) => {
                self.mode = Mode::Ready;
                Ok(response)
            }
            Err(error @ (TransactionError::Deadline | TransactionError::Canceled)) => {
                self.mode = if pending.remaining == 0 {
                    if pending.initializing {
                        Mode::Uninitialized
                    } else {
                        Mode::Ready
                    }
                } else {
                    Mode::Late(pending)
                };
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
    async fn write(
        &mut self,
        request: &mut Request,
        pending: &mut Pending,
        started: Arc<AtomicU64>,
    ) -> Result<(), TransactionError> {
        let monitor = self.monitor.clone();
        // WAIT: atx-transaction-write
        let write = monitor.write_correlated(
            &request.command,
            request.deadline.saturating_duration_since(Instant::now()),
            Arc::clone(&started),
        );
        tokio::pin!(write);
        let base_minimum = pending.minimum;
        loop {
            if Instant::now() >= request.deadline {
                return Err(TransactionError::Deadline);
            }
            pending.minimum = base_minimum.max(started.load(Ordering::Acquire));
            // WAIT: atx-transaction-write
            tokio::select! {
                biased;
                event = self.events.recv() => self.active_event(event, pending)?,
                _ = stop_requested(&mut self.stop) => { self.mode = Mode::Closed; return Err(TransactionError::Closed); },
                _ = request.reply.closed() => return Err(TransactionError::Canceled),
                _ = tokio::time::sleep_until(request.deadline) => return Err(TransactionError::Deadline),
                written = &mut write => {
                    written?;
                    pending.minimum = base_minimum.max(started.load(Ordering::Acquire));
                    return Ok(());
                },
            }
        }
    }
    async fn await_response(
        &mut self,
        request: &mut Request,
        pending: &mut Pending,
    ) -> Result<Response, TransactionError> {
        loop {
            if Instant::now() >= request.deadline {
                return Err(TransactionError::Deadline);
            }
            if pending.remaining == 0 {
                return pending.result.take().ok_or(TransactionError::StreamLoss);
            }
            self.life.status.send_replace(SessionState::Awaiting {
                remaining: pending.remaining,
            });
            // WAIT: atx-transaction-response
            tokio::select! {
                biased;
                event = self.events.recv() => self.active_event(event, pending)?,
                _ = stop_requested(&mut self.stop) => { self.mode = Mode::Closed; return Err(TransactionError::Closed); },
                _ = request.reply.closed() => return Err(TransactionError::Canceled),
                _ = tokio::time::sleep_until(request.deadline) => return Err(TransactionError::Deadline),
            }
        }
    }
}

fn interferes(command: &Command, packet: &Packet) -> bool {
    let Command::Send { bus, frame, .. } = command else {
        return false;
    };
    if *bus != packet.bus {
        return false;
    }
    match &packet.kind {
        Kind::Dali(observed @ DaliFrame::Bits16(_))
        | Kind::Dali(observed @ DaliFrame::Bits25(_)) => observed != frame,
        Kind::Dali(observed @ DaliFrame::Bits24(value)) if value & (1 << 16) != 0 => {
            observed != frame
        }
        _ => false,
    }
}
fn response(command: &Command, packet: &Packet) -> Option<Response> {
    match (command, &packet.kind) {
        (Command::Version, Kind::Version(version)) if packet.bus == 0 => {
            Some(Response::Version(version.clone()))
        }
        (Command::BusStatus { bus }, Kind::BusStatus { status, channels })
            if *bus == packet.bus =>
        {
            Some(Response::Status {
                status: *status,
                channels: *channels,
            })
        }
        (Command::Send { bus, .. }, kind) if *bus == packet.bus => {
            Some(Response::Dali(vec![match kind {
                Kind::Dali(DaliFrame::Bits8(value)) => BusReply::Value(*value),
                Kind::NoResponse => BusReply::NoResponse,
                Kind::ReceiveCollision(code) => BusReply::ReceiveCollision(*code),
                Kind::TransmitCollision(code) => BusReply::TransmitCollision(*code),
                _ => return None,
            }]))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn actor_cannot_complete_an_expired_request_even_with_all_terminals_consumed() {
        let (stream, _hat) = tokio::io::duplex(128);
        let (monitor, events) = Monitor::start(stream);
        let (_send, requests) = mpsc::channel(1);
        let (_stop, stop) = watch::channel(false);
        let (status, _status_rx) = watch::channel(SessionState::Ready);
        let (completed, _completed_rx) = watch::channel(false);
        let mut actor = Actor {
            monitor,
            events,
            mode: Mode::Ready,
            requests,
            stop,
            life: Life { status, completed },
            repeat: RepeatPolicy::Unverified,
        };
        let (reply, _receive) = oneshot::channel();
        let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
        let mut request = Request {
            command: Command::Version,
            initializing: true,
            deadline: Instant::now(),
            reply,
            permit,
        };
        let mut pending = Pending {
            command: Command::Version,
            initializing: true,
            minimum: 1,
            remaining: 0,
            replies: Vec::new(),
            result: Some(Response::Status {
                status: 1,
                channels: 4,
            }),
        };
        assert_eq!(
            actor.await_response(&mut request, &mut pending).await,
            Err(TransactionError::Deadline),
            "actor accepted completion after its total deadline"
        );
        actor.monitor.close().await.unwrap();
    }
}
