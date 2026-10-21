use super::{BusReply, RepeatPolicy, Response, SessionState, TransactionError};
use crate::atx::joined;
use crate::atx::monitor::{Event, Monitor, RecoveryCancellation};
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

#[derive(Clone)]
enum Operation {
    Command {
        command: Command,
        initializing: bool,
    },
    Resynchronize {
        quiet: Duration,
    },
}
struct CancelRecovery(Arc<RecoveryCancellation>);
impl Drop for CancelRecovery {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
struct Request {
    cancellation: Arc<RecoveryCancellation>,
    operation: Operation,
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
    completed: joined::Completion<Result<(), TransactionError>>,
    shutdown: watch::Sender<Option<Instant>>,
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
}
impl Drop for Life {
    fn drop(&mut self) {
        self.status.send_replace(SessionState::Closed);
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
    shutdown: watch::Receiver<Option<Instant>>,
}

impl<S: DrainRead + AsyncWrite + Unpin + Send + 'static> Transactions<S> {
    pub fn start(stream: S, repeat: RepeatPolicy) -> (Self, broadcast::Receiver<Event>) {
        let (monitor, events) = Monitor::start(stream);
        let public = monitor.subscribe();
        let (send, receive) = mpsc::channel(1);
        let (status_send, status) = watch::channel(SessionState::Uninitialized);
        let (stop, stop_receive) = watch::channel(false);
        let (shutdown, shutdown_receive) = watch::channel(None);
        let actor = Actor {
            monitor: monitor.clone(),
            events,
            mode: Mode::Uninitialized,
            requests: receive,
            stop: stop_receive,
            life: Life {
                status: status_send,
            },
            repeat,
            shutdown: shutdown_receive,
        };
        let failed_monitor = monitor.clone();
        let failed_shutdown = shutdown.subscribe();
        let (worker, completed) =
            joined::start_with_failure_cleanup(actor.run(), move || async move {
                let deadline = failed_shutdown
                    .borrow()
                    .unwrap_or_else(|| Instant::now() + Duration::from_millis(500));
                let _ = failed_monitor.force_until(deadline).await;
            });
        (
            Self {
                shared: Arc::new(Shared {
                    monitor,
                    requests: send,
                    permit: Arc::new(Semaphore::new(1)),
                    status,
                    stop,
                    completed,
                    shutdown,
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
    #[cfg(test)]
    pub(crate) fn abort_worker_for_test(&self) {
        self.shared.worker.abort();
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
    pub async fn resynchronize(
        &self,
        quiet: Duration,
        within: Duration,
    ) -> Result<(), TransactionError> {
        if quiet.is_zero() {
            return Err(TransactionError::Deadline);
        }
        match self
            .enqueue(Operation::Resynchronize { quiet }, within)
            .await?
        {
            Response::Synchronized => Ok(()),
            _ => Err(TransactionError::StreamLoss),
        }
    }
    /// Authoritative even after Closed; used only after actual teardown when
    /// transferring known partial-write poison across an in-process reopen.
    pub(crate) fn write_is_poisoned(&self) -> bool {
        self.shared.monitor.write_is_poisoned()
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
        command
            .encode()
            .map_err(crate::atx::monitor::TransportError::from)?;
        terminal_count(&command, self.shared.repeat)?;
        self.enqueue(
            Operation::Command {
                command,
                initializing,
            },
            within,
        )
        .await
    }
    async fn enqueue(
        &self,
        operation: Operation,
        within: Duration,
    ) -> Result<Response, TransactionError> {
        if *self.shared.stop.borrow() {
            return Err(TransactionError::Closed);
        }
        let deadline = Instant::now()
            .checked_add(within)
            .ok_or(TransactionError::Deadline)?;
        let permit = Arc::clone(&self.shared.permit)
            .try_acquire_owned()
            .map_err(|_| TransactionError::Busy)?;
        match &operation {
            Operation::Command { initializing, .. } => admission(self.status(), *initializing)?,
            Operation::Resynchronize { .. } => match self.status() {
                SessionState::Closed => return Err(TransactionError::Closed),
                SessionState::Writing
                | SessionState::Awaiting { .. }
                | SessionState::Resynchronizing => return Err(TransactionError::Busy),
                _ => {}
            },
        }
        let (send, receive) = oneshot::channel();
        let cancellation = Arc::new(RecoveryCancellation::default());
        let _cancel = CancelRecovery(cancellation.clone());
        self.shared
            .requests
            .try_send(Request {
                cancellation,
                operation,
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
        self.close_until(Instant::now() + Duration::from_secs(1))
            .await
    }
    pub(crate) async fn close_until(&self, deadline: Instant) -> Result<(), TransactionError> {
        self.shared.shutdown.send_if_modified(|request| {
            if request.is_some() {
                return false;
            }
            *request = Some(deadline);
            true
        });
        self.shared.permit.close();
        self.shared.stop.send_replace(true);
        let mut completed = self.shared.completed.clone();
        // WAIT: atx-transaction-close
        let result = timeout_at(deadline, async {
            while completed.borrow_and_update().is_none() {
                completed
                    .changed()
                    .await
                    .map_err(|_| TransactionError::Closed)?;
            }
            completed
                .borrow()
                .as_ref()
                .cloned()
                .ok_or(TransactionError::Closed)?
                .map_err(|_| TransactionError::StreamLoss)?
        })
        .await
        .map_err(|_| TransactionError::Deadline)?;
        if Instant::now() >= deadline {
            return Err(TransactionError::Deadline);
        }
        result
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
    async fn run(mut self) -> Result<(), TransactionError> {
        self.drive().await;
        self.requests.close();
        while let Ok(request) = self.requests.try_recv() {
            let _ = request.reply.send(Err(TransactionError::Closed));
        }
        let deadline = self
            .shutdown
            .borrow()
            .unwrap_or_else(|| Instant::now() + Duration::from_millis(500));
        self.monitor.force_until(deadline).await?;
        if Instant::now() >= deadline {
            return Err(TransactionError::Deadline);
        }
        Ok(())
    }
    async fn drive(&mut self) {
        loop {
            if *self.stop.borrow() {
                return;
            }
            // WAIT: atx-transaction-dispatch
            tokio::select! {
                biased;
                _ = stop_requested(&mut self.stop) => return,
                request = self.requests.recv() => {
                    let Some(mut request) = request else { return; };
                    let result = self.request(&mut request).await;
                    self.publish_mode();
                    let Request { reply, permit, .. } = request;
                    drop(permit); // Retire admission before waking the caller.
                    let _ = reply.send(result);
                },
                event = self.events.recv() => { self.idle_event(event); self.publish_mode(); },
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
        if request.reply.is_closed() || request.cancellation.is_canceled() {
            return Err(TransactionError::Canceled);
        }
        match request.operation.clone() {
            Operation::Command {
                command,
                initializing,
            } => self.command(request, command, initializing).await,
            Operation::Resynchronize { quiet } => {
                self.reset(self.monitor.receive_sequence() + 1);
                let result = self.recover(request, quiet).await;
                if result.is_err() && !matches!(self.mode, Mode::Closed) {
                    self.reset(self.monitor.receive_sequence() + 1);
                }
                result
            }
        }
    }
    async fn recover(
        &mut self,
        request: &mut Request,
        quiet: Duration,
    ) -> Result<Response, TransactionError> {
        let monitor = self.monitor.clone();
        loop {
            if monitor.write_is_poisoned() {
                return Err(crate::atx::monitor::TransportError::IndeterminateWrite.into());
            }
            self.life.status.send_replace(SessionState::Resynchronizing);
            // WAIT: atx-quiet
            let recovery =
                monitor.quiet_fence_using(quiet, request.deadline, request.cancellation.clone());
            tokio::pin!(recovery);
            let fence = loop {
                // A ready event branch can starve lower-priority timer polling,
                // including at cooperative budget exhaustion. Enforce the total
                // deadline independently of the caller's oneshot timeout.
                if Instant::now() >= request.deadline {
                    return Err(TransactionError::Deadline);
                }
                // WAIT: atx-quiet
                tokio::select! {
                    biased;
                    _ = stop_requested(&mut self.stop) => { self.mode = Mode::Closed; return Err(TransactionError::Closed); },
                    _ = request.reply.closed() => return Err(TransactionError::Canceled),
                    event = self.events.recv() => {
                        self.idle_event(event);
                        if matches!(self.mode, Mode::Closed) { return Err(TransactionError::Closed); }
                        if monitor.write_is_poisoned() { return Err(crate::atx::monitor::TransportError::IndeterminateWrite.into()); }
                        self.life.status.send_replace(SessionState::Resynchronizing);
                    },
                    fence = &mut recovery => break fence?,
                }
            };
            // Account for all queued old evidence through the sealed sequence.
            // Any newer raw activity starts another interval under this same
            // request deadline; closure and write poison cannot be retired.
            if self.try_complete_recovery(request, &fence)? {
                return Ok(Response::Synchronized);
            }
        }
    }
    fn try_complete_recovery(
        &mut self,
        request: &Request,
        fence: &crate::atx::monitor::RecoveryFence,
    ) -> Result<bool, TransactionError> {
        self.drain(request.deadline)?;
        if matches!(self.mode, Mode::Closed)
            || self.monitor.status() == crate::atx::monitor::LinkStatus::Closed
        {
            return Err(TransactionError::Closed);
        }
        if self.monitor.write_is_poisoned() {
            return Err(crate::atx::monitor::TransportError::IndeterminateWrite.into());
        }
        if self.monitor.activity_generation() != fence.activity_generation {
            return Ok(false);
        }
        if self.monitor.receive_sequence() < fence.sequence {
            return Err(TransactionError::StreamLoss);
        }
        if *self.stop.borrow() {
            self.mode = Mode::Closed;
            return Err(TransactionError::Closed);
        }
        if request.reply.is_closed() || request.cancellation.is_canceled() {
            return Err(TransactionError::Canceled);
        }
        if Instant::now() >= request.deadline {
            return Err(TransactionError::Deadline);
        }
        self.mode = Mode::Uninitialized;
        Ok(true)
    }
    async fn command(
        &mut self,
        request: &mut Request,
        command: Command,
        initializing: bool,
    ) -> Result<Response, TransactionError> {
        self.drain(request.deadline)?;
        self.publish_mode();
        admission(*self.life.status.borrow(), initializing)?;
        if Instant::now() >= request.deadline {
            return Err(TransactionError::Deadline);
        }
        let fence = self
            .monitor
            .fence(request.deadline.saturating_duration_since(Instant::now()))
            .await?;
        self.drain(request.deadline)?;
        self.publish_mode();
        admission(*self.life.status.borrow(), initializing)?;
        if request.reply.is_closed() {
            return Err(TransactionError::Canceled);
        }
        let mut pending = Pending {
            command: command.clone(),
            initializing,
            minimum: fence.minimum_sequence,
            remaining: terminal_count(&command, self.repeat)?,
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
        let command = pending.command.clone();
        // WAIT: atx-transaction-write
        let write = monitor.write_correlated(
            &command,
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
                _ = stop_requested(&mut self.stop) => { self.mode = Mode::Closed; return Err(TransactionError::Closed); },
                event = self.events.recv() => self.active_event(event, pending)?,
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
                _ = stop_requested(&mut self.stop) => { self.mode = Mode::Closed; return Err(TransactionError::Closed); },
                event = self.events.recv() => self.active_event(event, pending)?,
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
    use std::{future::Future, task::Poll};

    #[tokio::test(start_paused = true)]
    async fn canceled_and_expired_queued_callers_retire_under_continuous_input() {
        for canceled in [true, false] {
            let (stream, _hat) = tokio::io::duplex(128);
            let (monitor, _) = Monitor::start(stream);
            let (flood, events) = broadcast::channel(128);
            let (send, requests) = mpsc::channel(1);
            let (stop, stop_rx) = watch::channel(false);
            let (status, status_rx) = watch::channel(SessionState::Uninitialized);
            let (shutdown, shutdown_rx) = watch::channel(None);
            let actor = Actor {
                monitor: monitor.clone(),
                events,
                mode: Mode::Uninitialized,
                requests,
                stop: stop_rx,
                life: Life { status },
                repeat: RepeatPolicy::Unverified,
                shutdown: shutdown_rx,
            };
            let (begin, gated) = oneshot::channel();
            let (polled, first_poll) = oneshot::channel();
            let (worker, completed) = joined::start(async move {
                gated.await.unwrap();
                let operation = actor.run();
                tokio::pin!(operation);
                let mut polled = Some(polled);
                std::future::poll_fn(|cx| {
                    // Refill on every task poll, including after cooperative yield.
                    for _ in 0..128 {
                        flood
                            .send(Event::Packet {
                                sequence: 1,
                                packet: Ok(Packet {
                                    bus: 0,
                                    kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                                }),
                            })
                            .unwrap();
                    }
                    let result = operation.as_mut().poll(cx);
                    if let Some(polled) = polled.take() {
                        let _ = polled.send(());
                    }
                    result
                })
                .await
            });
            let link = Transactions {
                shared: Arc::new(Shared {
                    monitor,
                    requests: send,
                    permit: Arc::new(Semaphore::new(1)),
                    status: status_rx,
                    stop,
                    completed,
                    shutdown,
                    worker,
                    repeat: RepeatPolicy::Unverified,
                }),
            };
            let mut caller = Some(Box::pin(link.initialize(Duration::from_millis(20))));
            assert!(std::future::poll_fn(|cx| Poll::Ready(
                caller.as_mut().unwrap().as_mut().poll(cx)
            ))
            .await
            .is_pending());
            assert_eq!(link.shared.permit.available_permits(), 0);
            if canceled {
                drop(caller.take());
            } else {
                tokio::time::advance(Duration::from_millis(30)).await;
            }
            begin.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(1), first_poll)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                link.shared.permit.available_permits(),
                1,
                "continuous input stranded a canceled or expired request's admission permit"
            );
            drop(caller);
            link.close().await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn recovery_deadline_survives_a_ready_event_backlog_without_caller_polling() {
        let (stream, _hat) = tokio::io::duplex(128);
        let (monitor, _) = Monitor::start(stream);
        let (flood, events) = broadcast::channel(1024);
        let (_send, requests) = mpsc::channel(1);
        let (_stop, stop) = watch::channel(false);
        let (_shutdown, shutdown) = watch::channel(None);
        let (status, _status_rx) = watch::channel(SessionState::ResetRequired);
        let mut actor = Actor {
            monitor,
            events,
            mode: Mode::Reset { after: 0 },
            requests,
            stop,
            life: Life { status },
            repeat: RepeatPolicy::Unverified,
            shutdown,
        };
        // Keep the caller's receive channel alive throughout: its timeout/drop
        // cannot rescue a starved actor-side acquisition deadline.
        let (reply, _receive) = oneshot::channel();
        let mut request = Request {
            cancellation: Arc::new(RecoveryCancellation::default()),
            operation: Operation::Resynchronize {
                quiet: Duration::from_millis(10),
            },
            deadline: Instant::now() + Duration::from_millis(20),
            reply,
            permit: Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),
        };
        let (entered, waiting) = oneshot::channel();
        let worker = tokio::spawn(async move {
            let mut entered = Some(entered);
            let result = {
                let recovery = actor.request(&mut request);
                tokio::pin!(recovery);
                // Refill before every poll so a biased event branch remains
                // ready, including across Tokio cooperative budget exhaustion.
                tokio::time::timeout(
                    Duration::from_millis(100),
                    std::future::poll_fn(|cx| {
                        for _ in 0..256 {
                            flood
                                .send(Event::Packet {
                                    sequence: 1,
                                    packet: Ok(Packet {
                                        bus: 0,
                                        kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                                    }),
                                })
                                .unwrap();
                        }
                        let result = recovery.as_mut().poll(cx);
                        if let Some(entered) = entered.take() {
                            let _ = entered.send(());
                        }
                        result
                    }),
                )
                .await
            };
            assert_eq!(
                result,
                Ok(Err(TransactionError::Deadline)),
                "ready event backlog starved the actor recovery deadline"
            );
            assert!(matches!(actor.mode, Mode::Reset { .. }));
            actor.monitor.close().await.unwrap();
        });
        waiting.await.unwrap();
        tokio::time::advance(Duration::from_millis(200)).await;
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn caller_cancellation_reaches_reader_without_actor_polling() {
        use crate::atx::test_support::HeldDrain;
        use std::sync::atomic::AtomicBool;
        use tokio::io::AsyncWriteExt;
        let (stream, mut hat) = tokio::io::duplex(1024);
        let held = Arc::new(AtomicBool::new(false));
        let (entered, waiting) = oneshot::channel();
        let (monitor, events) = Monitor::start(HeldDrain {
            stream,
            held: held.clone(),
            entered: Some(entered),
        });
        let mut public = monitor.subscribe();
        hat.write_all(b"L0204").await.unwrap();
        monitor.fence(Duration::from_secs(1)).await.unwrap();
        let (send, requests) = mpsc::channel(1);
        let (stop, stop_rx) = watch::channel(false);
        let (status, status_rx) = watch::channel(SessionState::ResetRequired);
        let (shutdown, shutdown_rx) = watch::channel(None);
        let (_completion, completed) = watch::channel(None);
        let dummy = tokio::spawn(std::future::pending::<()>());
        let link = Transactions {
            shared: Arc::new(Shared {
                monitor: monitor.clone(),
                requests: send,
                permit: Arc::new(Semaphore::new(1)),
                status: status_rx,
                stop,
                completed,
                shutdown,
                worker: dummy.abort_handle(),
                repeat: RepeatPolicy::Unverified,
            }),
        };
        let mut actor = Actor {
            monitor: monitor.clone(),
            events,
            mode: Mode::Reset { after: 0 },
            requests,
            stop: stop_rx,
            life: Life { status },
            repeat: RepeatPolicy::Unverified,
            shutdown: shutdown_rx,
        };
        let mut caller =
            Box::pin(link.resynchronize(Duration::from_millis(20), Duration::from_secs(1)));
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(caller.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        let mut request = actor.requests.try_recv().unwrap();
        held.store(true, Ordering::Release);
        let mut recovery = Box::pin(actor.recover(&mut request, Duration::from_millis(20)));
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(recovery.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        tokio::time::advance(Duration::from_millis(20)).await;
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(recovery.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        waiting.await.unwrap();
        drop(caller);
        // The actor future remains alive but is deliberately NOT polled until
        // the reader finishes the old fence and parses the suffix.
        held.store(false, Ordering::Release);
        monitor.fence(Duration::from_secs(1)).await.unwrap();
        hat.write_all(b"01\n").await.unwrap();
        let event = tokio::time::timeout(Duration::from_millis(50), public.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                event,
                Event::Packet {
                    packet: Ok(Packet {
                        kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                        ..
                    }),
                    ..
                }
            ),
            "canceled caller discarded framing before actor observed cancellation"
        );
        assert_eq!(recovery.await, Err(TransactionError::Canceled));
        monitor.close().await.unwrap();
        drop(link);
        assert!(dummy.await.unwrap_err().is_cancelled());
    }

    #[tokio::test(start_paused = true)]
    async fn recovery_cannot_commit_a_reader_proof_after_new_activity() {
        use tokio::io::AsyncWriteExt;
        let (stream, mut hat) = tokio::io::duplex(1024);
        let (monitor, events) = Monitor::start(stream);
        let proof = monitor
            .quiet_fence(
                Duration::from_millis(20),
                Instant::now() + Duration::from_secs(1),
            )
            .await
            .unwrap();
        // This is newer than the returned reader boundary, before actor commit.
        hat.write_all(b"L020401\n").await.unwrap();
        monitor.fence(Duration::from_secs(1)).await.unwrap();
        let (_send, requests) = mpsc::channel(1);
        let (_stop, stop) = watch::channel(false);
        let (_shutdown, shutdown) = watch::channel(None);
        let (status, _status_rx) = watch::channel(SessionState::Resynchronizing);
        let mut actor = Actor {
            monitor,
            events,
            mode: Mode::Reset { after: 0 },
            requests,
            stop,
            life: Life { status },
            repeat: RepeatPolicy::Unverified,
            shutdown,
        };
        let (reply, _receive) = oneshot::channel();
        let mut request = Request {
            cancellation: Arc::new(RecoveryCancellation::default()),
            operation: Operation::Resynchronize {
                quiet: Duration::from_millis(20),
            },
            deadline: Instant::now() + Duration::from_secs(1),
            reply,
            permit: Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),
        };
        assert!(
            !actor.try_complete_recovery(&request, &proof).unwrap(),
            "stale proof permitted initialization without another quiet interval"
        );
        assert!(matches!(actor.mode, Mode::Reset { .. }));
        let fresh = actor
            .monitor
            .quiet_fence(Duration::from_millis(20), request.deadline)
            .await
            .unwrap();
        request.cancellation.cancel();
        assert!(!request.reply.is_closed());
        assert_eq!(
            actor.try_complete_recovery(&request, &fresh),
            Err(TransactionError::Canceled),
            "actor committed recovery despite caller cancellation flag"
        );
        assert!(matches!(actor.mode, Mode::Reset { .. }));
        request.cancellation = Arc::new(RecoveryCancellation::default());
        assert!(actor.try_complete_recovery(&request, &fresh).unwrap());
        assert!(matches!(actor.mode, Mode::Uninitialized));
        actor.monitor.close().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn actor_cannot_complete_an_expired_request_even_with_all_terminals_consumed() {
        let (stream, _hat) = tokio::io::duplex(128);
        let (monitor, events) = Monitor::start(stream);
        let (_send, requests) = mpsc::channel(1);
        let (_stop, stop) = watch::channel(false);
        let (status, _status_rx) = watch::channel(SessionState::Ready);
        let (_shutdown, shutdown) = watch::channel(None);
        let mut actor = Actor {
            monitor,
            events,
            mode: Mode::Ready,
            requests,
            stop,
            life: Life { status },
            repeat: RepeatPolicy::Unverified,
            shutdown,
        };
        let (reply, _receive) = oneshot::channel();
        let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
        let mut request = Request {
            cancellation: Arc::new(RecoveryCancellation::default()),
            operation: Operation::Command {
                command: Command::Version,
                initializing: true,
            },
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
