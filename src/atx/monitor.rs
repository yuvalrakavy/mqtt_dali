//! Continuous HAT monitoring. Writes report bytes accepted, never DALI ACKs.
use super::joined;
use super::stream::{Drain, DrainRead};
use super::wire::{Command, DecodeError, Framer, Packet};
use std::future::poll_fn;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::task::Poll;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf, WriteHalf};
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex};
use tokio::task::AbortHandle;

mod recovery;
pub(crate) use recovery::RecoveryCancellation;
use recovery::{Activity, FenceRequest, Reset};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("ATX receive fence is busy")]
    Busy,
    #[error("ATX link is closed")]
    Closed,
    #[error("a potentially partial ATX write requires reinitialization")]
    IndeterminateWrite,
    #[error("ATX operation exceeded its deadline")]
    Deadline,
    #[error("ATX I/O: {0}")]
    Io(String),
    #[error(transparent)]
    Invalid(#[from] DecodeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    Eof,
    Io(String),
    Stopped,
    WorkerStopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Packet {
        sequence: u64,
        packet: Result<Packet, DecodeError>,
    },
    WriteIndeterminate,
    Closed(CloseReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteReceipt {
    pub bytes_accepted: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Monitoring,
    WriteIndeterminate,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceiveFence {
    pub minimum_sequence: u64,
    sequence: u64,
    activity_generation: u64,
    reset: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RecoveryFence {
    pub sequence: u64,
    pub activity_generation: u64,
}

struct State {
    alive: AtomicBool,
    poisoned: AtomicBool,
    events: broadcast::Sender<Event>,
    stop: watch::Sender<bool>,
    sequence: AtomicU64,
    minimum: AtomicU64,
    draining: AtomicBool,
    deadline: watch::Sender<Option<std::time::Instant>>,
    marker: watch::Sender<Option<Arc<AtomicU64>>>,
    activity: watch::Sender<Activity>,
    reset: std::sync::Mutex<Option<Arc<Reset>>>,
}

struct Inner<W> {
    writer: Mutex<Option<W>>,
    state: Arc<State>,
    reader: AbortHandle,
    stopped: joined::Completion<()>,
    fences: mpsc::Sender<FenceRequest>,
}

impl<W> Drop for Inner<W> {
    fn drop(&mut self) {
        self.state.stop.send_replace(true);
        self.reader.abort();
    }
}

pub struct Monitor<S> {
    inner: Arc<Inner<WriteHalf<ReadMode<S>>>>,
}

impl<S> Clone for Monitor<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct ReadLife {
    state: Arc<State>,
    reason: CloseReason,
}

impl Drop for ReadLife {
    fn drop(&mut self) {
        self.state.alive.store(false, Ordering::Release);
        if self.reason == CloseReason::WorkerStopped && *self.state.stop.borrow() {
            self.reason = CloseReason::Stopped;
        }
        self.state.stop.send_replace(true);
        let _ = self.state.events.send(Event::Closed(self.reason.clone()));
    }
}

struct WriteLife {
    state: Arc<State>,
    completed: bool,
    started: Arc<AtomicU64>,
}

impl Drop for WriteLife {
    fn drop(&mut self) {
        self.state.marker.send_replace(None);
        if !self.completed && self.started.load(Ordering::Acquire) != u64::MAX {
            self.state.poisoned.store(true, Ordering::Release);
            let _ = self.state.events.send(Event::WriteIndeterminate);
        }
    }
}

impl<S: DrainRead + AsyncWrite + Unpin + Send + 'static> Monitor<S> {
    pub(crate) async fn fence(&self, within: Duration) -> Result<ReceiveFence, TransportError> {
        let deadline = tokio::time::Instant::now()
            .checked_add(within)
            .ok_or(TransportError::Deadline)?;
        self.fence_until(deadline, None).await
    }
    async fn fence_until(
        &self,
        deadline: tokio::time::Instant,
        reset: Option<Arc<Reset>>,
    ) -> Result<ReceiveFence, TransportError> {
        if tokio::time::Instant::now() >= deadline {
            return Err(TransportError::Deadline);
        }
        if self.status() == LinkStatus::Closed {
            return Err(TransportError::Closed);
        }
        let (send, receive) = oneshot::channel();
        self.inner
            .fences
            .try_send(FenceRequest {
                deadline,
                reset,
                reply: send,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => TransportError::Busy,
                mpsc::error::TrySendError::Closed(_) => TransportError::Closed,
            })?;
        // WAIT: atx-receive-fence
        tokio::time::timeout_at(deadline, receive)
            .await
            .map_err(|_| TransportError::Deadline)?
            .map_err(|_| TransportError::Closed)?
    }

    pub(crate) fn receive_sequence(&self) -> u64 {
        self.inner.state.sequence.load(Ordering::Acquire)
    }

    pub(crate) fn rearm_after_boot(&self) {
        // Only the transaction owner may call this after its trusted boot fence.
        self.inner.state.poisoned.store(false, Ordering::Release);
    }
    pub fn start(stream: S) -> (Self, broadcast::Receiver<Event>) {
        let (events, receive) = broadcast::channel(128);
        let (stop, _) = watch::channel(false);
        let (deadline, _) = watch::channel(None);
        let (marker, _) = watch::channel(None);
        let (fences, fence_requests) = mpsc::channel(1);
        let state = Arc::new(State {
            alive: AtomicBool::new(true),
            poisoned: AtomicBool::new(false),
            events,
            stop,
            sequence: AtomicU64::new(0),
            minimum: AtomicU64::new(1),
            draining: AtomicBool::new(false),
            deadline,
            marker,
            activity: watch::channel(Activity {
                generation: 0,
                at: tokio::time::Instant::now(),
            })
            .0,
            reset: std::sync::Mutex::new(None),
        });
        // Framing runs inside the same split-stream lock as the first write
        // poll: an observed partial line cannot race the write watermark.
        let (reader, writer) = tokio::io::split(ReadMode {
            stream,
            state: Arc::clone(&state),
            framer: Framer::default(),
            sequence: 0,
        });
        let life = ReadLife {
            state: Arc::clone(&state),
            reason: CloseReason::WorkerStopped,
        };
        // WAIT: atx-reader
        let (reader, stopped_rx) = joined::start(read_loop(reader, life, fence_requests));
        (
            Self {
                inner: Arc::new(Inner {
                    writer: Mutex::new(Some(writer)),
                    state,
                    reader,
                    stopped: stopped_rx,
                    fences,
                }),
            },
            receive,
        )
    }

    /// Queue one complete encoded line under a total lock+write deadline. This
    /// does not correlate responses or prove that the HAT/device accepted it.
    /// Dropping this future during a write leaves admission poisoned.
    pub async fn write(
        &self,
        command: &Command,
        within: Duration,
    ) -> Result<WriteReceipt, TransportError> {
        self.write_correlated(command, within, Arc::new(AtomicU64::new(u64::MAX)))
            .await
    }

    pub(crate) async fn write_correlated(
        &self,
        command: &Command,
        within: Duration,
        started: Arc<AtomicU64>,
    ) -> Result<WriteReceipt, TransportError> {
        let bytes = command.encode()?;
        let deadline = tokio::time::Instant::now()
            .checked_add(within)
            .ok_or(TransportError::Deadline)?;
        // WAIT: atx-write-lock
        let mut writer = tokio::time::timeout_at(deadline, self.inner.writer.lock())
            .await
            .map_err(|_| TransportError::Deadline)?;
        if !self.inner.state.alive.load(Ordering::Acquire) {
            return Err(TransportError::Closed);
        }
        if self.inner.state.poisoned.load(Ordering::Acquire) {
            return Err(TransportError::IndeterminateWrite);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(TransportError::Deadline);
        }
        let mut life = WriteLife {
            state: Arc::clone(&self.inner.state),
            completed: false,
            started: Arc::clone(&started),
        };
        let writer = writer.as_mut().ok_or(TransportError::Closed)?;
        self.inner.state.marker.send_replace(Some(started));
        let mut stop = self.inner.state.stop.subscribe();
        // WAIT: atx-write
        let written = tokio::select! {
            biased;
            _ = stopped(&mut stop) => return Err(TransportError::Closed),
            written = tokio::time::timeout_at(deadline, writer.write_all(&bytes)) => written,
        };
        match written {
            Ok(Ok(())) => {
                life.completed = true;
                Ok(WriteReceipt {
                    bytes_accepted: bytes.len(),
                })
            }
            Ok(Err(error)) => Err(TransportError::Io(error.to_string())),
            Err(_) => Err(TransportError::Deadline),
        }
    }

    /// Start observing future records. A lagging receiver gets explicit Lagged;
    /// it must invalidate inferred state before relying on later events. Subscribe
    /// first, then inspect status(), so an already terminal link cannot be missed.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.state.events.subscribe()
    }

    pub fn status(&self) -> LinkStatus {
        if !self.inner.state.alive.load(Ordering::Acquire) {
            LinkStatus::Closed
        } else if self.inner.state.poisoned.load(Ordering::Acquire) {
            LinkStatus::WriteIndeterminate
        } else {
            LinkStatus::Monitoring
        }
    }

    pub async fn close(&self) -> Result<(), TransportError> {
        self.force_until(tokio::time::Instant::now() + Duration::from_millis(500))
            .await
    }

    pub(crate) async fn force_until(
        &self,
        deadline: tokio::time::Instant,
    ) -> Result<(), TransportError> {
        self.inner.state.alive.store(false, Ordering::Release);
        self.inner.state.stop.send_replace(true);
        self.inner.reader.abort();
        let mut stopped = self.inner.stopped.clone();
        // WAIT: atx-close
        tokio::time::timeout_at(deadline, async {
            let mut writer = self.inner.writer.lock().await;
            writer.take();
            drop(writer);
            while stopped.borrow_and_update().is_none() {
                stopped
                    .changed()
                    .await
                    .map_err(|_| TransportError::Closed)?;
            }
            if stopped.borrow().as_ref().is_some_and(|result| {
                result
                    .as_ref()
                    .err()
                    .is_some_and(|failure| !failure.canceled)
            }) {
                return Err(TransportError::Closed);
            }
            Ok(())
        })
        .await
        .map_err(|_| TransportError::Deadline)?
    }
}

async fn stopped(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow_and_update() {
        // WAIT: atx-write
        if stop.changed().await.is_err() {
            return;
        }
    }
}

async fn line_deadline(deadline: Option<std::time::Instant>) {
    if let Some(deadline) = deadline {
        tokio::time::sleep_until(deadline.into()).await;
    } else {
        // WAIT: atx-reader
        std::future::pending::<()>().await;
    }
}

struct ReadMode<S> {
    stream: S,
    state: Arc<State>,
    framer: Framer,
    sequence: u64,
}

impl<S> ReadMode<S> {
    fn process(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.state.activity.send_modify(|activity| {
                activity.generation = activity
                    .generation
                    .checked_add(1)
                    .expect("ATX receive generation exhausted");
                activity.at = tokio::time::Instant::now();
            });
        }
        let packets = self
            .framer
            .feed(bytes, tokio::time::Instant::now().into_std());
        self.state.minimum.store(
            self.sequence
                + packets.len() as u64
                + if self.framer.has_incomplete_line() {
                    2
                } else {
                    1
                },
            Ordering::Release,
        );
        self.state.deadline.send_replace(self.framer.deadline());
        publish(&self.state, &mut self.sequence, packets);
    }
}

impl<S: DrainRead + Unpin> AsyncRead for ReadMode<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.process(&[]);
        let before = buffer.filled().len();
        let result = if self.state.draining.load(Ordering::Acquire) {
            match Pin::new(&mut self.stream).poll_drain(context, buffer) {
                Poll::Ready(Ok(Drain::Data)) => Poll::Ready(Ok(())),
                Poll::Ready(Ok(Drain::Empty)) => {
                    // WAIT: atx-reset-guard
                    let reset = self.state.reset.lock().unwrap().clone();
                    if let Some(reset) = reset {
                        let activity = *self.state.activity.borrow();
                        if self.state.alive.load(Ordering::Acquire)
                            && !self.state.poisoned.load(Ordering::Acquire)
                        {
                            if let Some(_authorization) = reset.authorize(activity) {
                                self.framer = Framer::default();
                                self.process(&[]);
                            }
                        }
                    }
                    Poll::Ready(Err(std::io::ErrorKind::WouldBlock.into()))
                }
                Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Pin::new(&mut self.stream).poll_read(context, buffer)
        };
        if matches!(result, Poll::Ready(Ok(()))) {
            self.process(&buffer.filled()[before..]);
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ReadMode<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let minimum = self.state.minimum.load(Ordering::Acquire);
        let result = Pin::new(&mut self.stream).poll_write(context, bytes);
        if matches!(result, Poll::Ready(Ok(count)) if count > 0)
            || matches!(result, Poll::Ready(Err(_)))
        {
            if let Some(marker) = self.state.marker.borrow().clone() {
                let _ =
                    marker.compare_exchange(u64::MAX, minimum, Ordering::AcqRel, Ordering::Acquire);
            }
        }
        result
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}

fn publish(state: &State, sequence: &mut u64, packets: Vec<Result<Packet, DecodeError>>) {
    for packet in packets {
        *sequence += 1;
        state.sequence.store(*sequence, Ordering::Release);
        let _ = state.events.send(Event::Packet {
            sequence: *sequence,
            packet,
        });
    }
}

/// Retry/yield remains Pending. Only explicit would-block completes the drain.
async fn read_available<R: AsyncRead + Unpin>(
    reader: &mut R,
    bytes: &mut [u8],
) -> std::io::Result<Option<usize>> {
    // WAIT: atx-receive-fence
    poll_fn(|context| {
        let mut buffer = tokio::io::ReadBuf::new(bytes);
        match Pin::new(&mut *reader).poll_read(context, &mut buffer) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(Some(buffer.filled().len()))),
            Poll::Ready(Err(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Poll::Ready(Ok(None))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}

async fn receive_fence<R: AsyncRead + Unpin>(
    reader: &mut R,
    life: &mut ReadLife,
) -> Result<ReceiveFence, TransportError> {
    let mut bytes = [0; 512];
    for _ in 0..8 {
        match read_available(reader, &mut bytes).await {
            Ok(None) => {
                // WAIT: atx-reset-guard
                let reset = life
                    .state
                    .reset
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|reset| reset.applied());
                return Ok(ReceiveFence {
                    minimum_sequence: life.state.minimum.load(Ordering::Acquire),
                    sequence: life.state.sequence.load(Ordering::Acquire),
                    activity_generation: life.state.activity.borrow().generation,
                    reset,
                });
            }
            Ok(Some(0)) => {
                life.reason = CloseReason::Eof;
                return Err(TransportError::Closed);
            }
            Ok(Some(_)) => {}
            Err(error) => {
                life.reason = CloseReason::Io(error.to_string());
                return Err(TransportError::Closed);
            }
        }
    }
    Err(TransportError::Busy)
}

async fn read_loop<R: AsyncRead + Unpin>(
    mut reader: R,
    mut life: ReadLife,
    mut fences: mpsc::Receiver<FenceRequest>,
) {
    let mut bytes = [0; 512];
    loop {
        let deadline = *life.state.deadline.borrow();
        // WAIT: atx-reader
        tokio::select! {
            request = fences.recv() => {
                let Some(request) = request else { return; };
                if request.reply.is_closed() { continue; }
                // WAIT: atx-reset-guard
                *life.state.reset.lock().unwrap() = request.reset;
                life.state.draining.store(true, Ordering::Release);
                let drain_deadline = request.deadline.min(tokio::time::Instant::now() + Duration::from_millis(100));
                // WAIT: atx-receive-fence
                let result = tokio::time::timeout_at(drain_deadline, receive_fence(&mut reader, &mut life)).await.unwrap_or(Err(TransportError::Busy));
                life.state.draining.store(false, Ordering::Release);
                // WAIT: atx-reset-guard
                life.state.reset.lock().unwrap().take();
                let closed = matches!(result, Err(TransportError::Closed));
                let _ = request.reply.send(result);
                if closed { return; }
                continue;
            },
            read = reader.read(&mut bytes) => match read {
                Ok(0) => { life.reason = CloseReason::Eof; return; },
                Ok(_) => {},
                Err(error) => { life.reason = CloseReason::Io(error.to_string()); return; },
            },
            _ = line_deadline(deadline) => {},
        };
        tokio::task::yield_now().await;
    }
}

#[cfg(test)]
mod tests;
