//! Continuous HAT monitoring. Writes report bytes accepted, never DALI ACKs.
use super::wire::{Command, DecodeError, Framer, Packet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::sync::{broadcast, watch, Mutex};
use tokio::task::AbortHandle;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
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

struct State {
    alive: AtomicBool,
    poisoned: AtomicBool,
    events: broadcast::Sender<Event>,
    stop: watch::Sender<bool>,
}

struct Inner<W> {
    writer: Mutex<Option<W>>,
    state: Arc<State>,
    reader: AbortHandle,
    stopped: watch::Receiver<bool>,
}

impl<W> Drop for Inner<W> {
    fn drop(&mut self) {
        self.state.stop.send_replace(true);
        self.reader.abort();
    }
}

pub struct Monitor<S> {
    inner: Arc<Inner<WriteHalf<S>>>,
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
    stopped: watch::Sender<bool>,
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
        self.stopped.send_replace(true);
    }
}

struct WriteLife {
    state: Arc<State>,
    completed: bool,
}

impl Drop for WriteLife {
    fn drop(&mut self) {
        if !self.completed {
            self.state.poisoned.store(true, Ordering::Release);
            let _ = self.state.events.send(Event::WriteIndeterminate);
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Monitor<S> {
    pub fn start(stream: S) -> (Self, broadcast::Receiver<Event>) {
        let (events, receive) = broadcast::channel(128);
        let (stopped, stopped_rx) = watch::channel(false);
        let (stop, _) = watch::channel(false);
        let state = Arc::new(State {
            alive: AtomicBool::new(true),
            poisoned: AtomicBool::new(false),
            events,
            stop,
        });
        let (reader, writer) = tokio::io::split(stream);
        let life = ReadLife {
            state: Arc::clone(&state),
            stopped,
            reason: CloseReason::WorkerStopped,
        };
        let task = tokio::spawn(read_loop(reader, life));
        let reader = task.abort_handle();
        drop(task); // Lifetime is held by Inner; its final drop aborts the reader.
        (
            Self {
                inner: Arc::new(Inner {
                    writer: Mutex::new(Some(writer)),
                    state,
                    reader,
                    stopped: stopped_rx,
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
        };
        let writer = writer.as_mut().ok_or(TransportError::Closed)?;
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
        self.inner.state.alive.store(false, Ordering::Release);
        self.inner.state.stop.send_replace(true);
        self.inner.reader.abort();
        let mut stopped = self.inner.stopped.clone();
        // WAIT: atx-close
        tokio::time::timeout(Duration::from_millis(500), async {
            let mut writer = self.inner.writer.lock().await;
            writer.take();
            drop(writer);
            while !*stopped.borrow_and_update() {
                stopped
                    .changed()
                    .await
                    .map_err(|_| TransportError::Closed)?;
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

async fn read_loop<R: AsyncRead + Unpin>(mut reader: R, mut life: ReadLife) {
    let mut bytes = [0; 512];
    let mut framer = Framer::default();
    let mut sequence = 0;
    loop {
        // WAIT: atx-reader
        let packets = tokio::select! {
            read = reader.read(&mut bytes) => match read {
                Ok(0) => { life.reason = CloseReason::Eof; return; },
                Ok(count) => framer.feed(&bytes[..count], tokio::time::Instant::now().into_std()),
                Err(error) => { life.reason = CloseReason::Io(error.to_string()); return; },
            },
            _ = line_deadline(framer.deadline()) => framer.expire(tokio::time::Instant::now().into_std()).into_iter().map(Err).collect(),
        };
        for packet in packets {
            sequence += 1;
            let _ = life.state.events.send(Event::Packet { sequence, packet });
        }
    }
}

#[cfg(test)]
mod tests;
