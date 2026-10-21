use super::model::{Model, Reply};
use super::scenario::{ClockMode, Repeat, Scenario};
use super::wire::{record, Command, Framer};
use crate::atx::stream::DrainRead;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::AbortHandle;
use tokio::time::{timeout_at, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fault {
    Delay { millis: u64 },
    Drop,
    ReceiveCollision,
    TransmitCollision,
    Truncate { length: usize },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    Snapshot,
    AdvanceTime {
        millis: u64,
    },
    InputEvent {
        bus: u8,
        frame: u32,
    },
    Raw {
        bytes: Vec<u8>,
    },
    Reboot,
    SetAddress {
        id: String,
        short_address: Option<u8>,
    },
    BusPower {
        bus: u8,
        powered: bool,
    },
    NextFault {
        effect: Fault,
    },
}
struct Request {
    action: Control,
    deadline: Instant,
    reply: oneshot::Sender<Result<serde_json::Value, String>>,
}
struct Shared {
    controls: mpsc::Sender<Request>,
    stop: watch::Sender<bool>,
    done: watch::Receiver<Option<Result<(), String>>>,
    worker: AbortHandle,
}
impl Drop for Shared {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.worker.abort();
    }
}
#[derive(Clone)]
pub struct Emulator {
    shared: Arc<Shared>,
}
struct Life(watch::Sender<Option<Result<(), String>>>);
impl Drop for Life {
    fn drop(&mut self) {
        if self.0.borrow().is_none() {
            self.0
                .send_replace(Some(Err("emulator worker stopped".into())));
        }
    }
}
struct Output {
    bytes: Vec<u8>,
    offset: usize,
    deadline: Instant,
}
struct Scheduled {
    at: Instant,
    order: u64,
    bytes: Vec<u8>,
}
struct State {
    model: Model,
    clock: ClockMode,
    repeat: Repeat,
    started: Instant,
    offset_ms: u64,
    output: VecDeque<Output>,
    scheduled: Vec<Scheduled>,
    order: u64,
    next_fault: Option<Fault>,
    framer: Framer,
    unterminated_output: bool,
}
impl Emulator {
    pub fn start<S: DrainRead + AsyncWrite + Unpin + Send + 'static>(
        stream: S,
        scenario: Scenario,
    ) -> Result<Self, String> {
        Self::start_retained(stream, scenario, None)
    }
    pub(crate) fn start_retained<S: DrainRead + AsyncWrite + Unpin + Send + 'static>(
        stream: S,
        scenario: Scenario,
        slave: Option<OwnedFd>,
    ) -> Result<Self, String> {
        let state = State {
            model: Model::new(scenario.clone())?,
            clock: scenario.clock,
            repeat: scenario.repeat,
            started: Instant::now(),
            offset_ms: 0,
            output: VecDeque::new(),
            scheduled: Vec::new(),
            order: 0,
            next_fault: None,
            framer: Framer::default(),
            unterminated_output: false,
        };
        let (controls, receive) = mpsc::channel(32);
        let (stop, stopping) = watch::channel(false);
        let (done, completed) = watch::channel(None);
        let worker = tokio::spawn(async move {
            let life = Life(done);
            let _slave = slave;
            let result = run(stream, state, receive, stopping).await;
            drop(_slave);
            life.0.send_replace(Some(result));
        });
        let abort = worker.abort_handle();
        drop(worker);
        Ok(Self {
            shared: Arc::new(Shared {
                controls,
                stop,
                done: completed,
                worker: abort,
            }),
        })
    }
    pub async fn control(
        &self,
        action: Control,
        within: Duration,
    ) -> Result<serde_json::Value, String> {
        let deadline = Instant::now()
            .checked_add(within)
            .ok_or("control deadline overflow")?;
        let (reply, receive) = oneshot::channel();
        self.shared
            .controls
            .try_send(Request {
                action,
                deadline,
                reply,
            })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => "emulator control queue is busy",
                mpsc::error::TrySendError::Closed(_) => "emulator is closed",
            })?;
        // WAIT: emulator-control
        let result = timeout_at(deadline, receive)
            .await
            .map_err(|_| "control deadline; a previously applied mutation is not undone")?
            .map_err(|_| "emulator closed during control")?;
        if Instant::now() >= deadline {
            return Err("control deadline; a previously applied mutation is not undone".into());
        }
        result
    }
    pub fn status(&self) -> Option<Result<(), String>> {
        self.shared.done.borrow().clone()
    }
    pub fn observe_status(&self) -> watch::Receiver<Option<Result<(), String>>> {
        self.shared.done.clone()
    }
    pub async fn close(&self) -> Result<(), String> {
        self.shared.stop.send_replace(true);
        let mut done = self.shared.done.clone();
        // WAIT: emulator-close
        tokio::time::timeout(Duration::from_secs(1), done.wait_for(Option::is_some))
            .await
            .map_err(|_| "emulator shutdown deadline")?
            .map_err(|_| "emulator worker disappeared")?;
        self.status()
            .unwrap_or_else(|| Err("emulator completion missing".into()))
    }
}
impl State {
    fn tick(&mut self) {
        let elapsed = if self.clock == ClockMode::Realtime {
            self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
        } else {
            0
        };
        self.model
            .advance_to(self.offset_ms.saturating_add(elapsed));
    }
    fn model_time(&self) -> u64 {
        self.offset_ms
            .saturating_add(if self.clock == ClockMode::Realtime {
                self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
            } else {
                0
            })
    }
    fn enqueue(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let total: usize = self.output.iter().map(|o| o.bytes.len()).sum();
        if self.output.len() >= 256 || total.saturating_add(bytes.len()) > 65536 {
            return Err("emulator UART output queue overflow".into());
        }
        self.output.push_back(Output {
            bytes,
            offset: 0,
            deadline: Instant::now() + Duration::from_secs(1),
        });
        Ok(())
    }
    fn terminal(&mut self, bus: u8, reply: Reply) -> Result<(), String> {
        let bytes = record(
            bus,
            &match reply {
                Reply::None => "N".into(),
                Reply::Value(v) => format!("J{v:02X}"),
                Reply::Collision => "X2".into(),
            },
        );
        match self.next_fault.take() {
            None => self.enqueue(bytes),
            Some(Fault::Drop) => Ok(()),
            Some(Fault::ReceiveCollision) => self.enqueue(record(bus, "X2")),
            Some(Fault::TransmitCollision) => self.enqueue(record(bus, "Z")),
            Some(Fault::Truncate { length }) => {
                self.enqueue(bytes.into_iter().take(length).collect())
            }
            Some(Fault::Delay { millis }) => {
                if self.scheduled.len() >= 64 {
                    return Err("emulator timer queue overflow".into());
                }
                self.order += 1;
                self.scheduled.push(Scheduled {
                    at: Instant::now() + Duration::from_millis(millis),
                    order: self.order,
                    bytes,
                });
                self.scheduled.sort_by_key(|entry| (entry.at, entry.order));
                Ok(())
            }
        }
    }
    fn command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Version => self.enqueue(b"V000004\n".to_vec()),
            Command::Status(bus) => self.enqueue(record(
                bus,
                if self.model.powered(bus) {
                    "D24"
                } else {
                    "D04"
                },
            )),
            Command::Send {
                bus,
                header,
                value,
                repeat,
            } => {
                let width = match header {
                    b'J' => 2,
                    b'L' => 6,
                    b'M' => 8,
                    _ => 4,
                };
                for index in 0..if repeat { 2 } else { 1 } {
                    self.enqueue(record(bus, &format!("{}{value:0width$X}", header as char)))?;
                    let now = self.model_time();
                    let reply = if header == b'H' {
                        self.model.transmit(bus, value as u16, now)
                    } else {
                        self.model.intervening_frame(bus);
                        Reply::None
                    };
                    if !repeat || self.repeat == Repeat::TwoTerminals || index == 1 {
                        self.terminal(bus, reply)?;
                    }
                }
                Ok(())
            }
        }
    }
    fn control(&mut self, action: Control) -> Result<serde_json::Value, String> {
        match action {
            Control::Snapshot => {
                let mut snapshot = self.model.snapshot();
                snapshot["transport"] = serde_json::json!({
                    "output_frames":self.output.len(),
                    "queued_bytes":self.output.iter().map(|o| o.bytes.len()-o.offset).sum::<usize>(),
                    "current_offset":self.output.front().map_or(0,|o| o.offset),
                    "scheduled_frames":self.scheduled.len(),
                });
                return Ok(snapshot);
            }
            Control::AdvanceTime { millis } => {
                self.offset_ms = self
                    .offset_ms
                    .checked_add(millis)
                    .ok_or("model time overflow")?;
                self.tick();
            }
            Control::InputEvent { bus, frame } => {
                if bus > 3 || frame > 0xffffff || frame & (1 << 16) != 0 {
                    return Err("invalid DALI-2 event frame".into());
                }
                self.enqueue(record(bus, &format!("L{frame:06X}")))?;
            }
            Control::Raw { bytes } => {
                if bytes.len() > 4096 {
                    return Err("raw injection exceeds 4096 bytes".into());
                }
                self.enqueue(bytes)?;
            }
            Control::Reboot => {
                let partial = self.unterminated_output;
                self.output.clear();
                self.scheduled.clear();
                self.next_fault = None;
                self.framer = Framer::default();
                self.enqueue(if partial {
                    b"\nB000004\n".to_vec()
                } else {
                    b"B000004\n".to_vec()
                })?;
            }
            Control::SetAddress { id, short_address } => {
                self.model.set_address(&id, short_address)?
            }
            Control::BusPower { bus, powered } => {
                self.model.set_power(bus, powered)?;
                self.enqueue(record(bus, if powered { "D24" } else { "D04" }))?;
            }
            Control::NextFault { effect } => {
                if self.next_fault.is_some() {
                    return Err("a next-response fault is already armed".into());
                }
                if matches!(effect, Fault::Delay { millis } if millis > 60_000)
                    || matches!(effect, Fault::Truncate { length } if length > 64)
                {
                    return Err("fault parameter exceeds bound".into());
                }
                self.next_fault = Some(effect);
            }
        }
        Ok(serde_json::json!({"ok":true}))
    }
}
async fn due(deadline: Option<Instant>) {
    // WAIT: emulator-loop
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
async fn run<S: DrainRead + AsyncWrite + Unpin>(
    stream: S,
    mut state: State,
    mut controls: mpsc::Receiver<Request>,
    mut stop: watch::Receiver<bool>,
) -> Result<(), String> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let mut bytes = [0; 512];
    state.enqueue(b"B000004\n".to_vec())?;
    loop {
        if *stop.borrow() {
            return Ok(());
        }
        state.tick();
        while state
            .scheduled
            .first()
            .is_some_and(|e| e.at <= Instant::now())
        {
            let entry = state.scheduled.remove(0);
            state.enqueue(entry.bytes)?;
        }
        let deadline = [
            state.framer.deadline(),
            state.scheduled.first().map(|s| s.at),
            state.output.front().map(|o| o.deadline),
        ]
        .into_iter()
        .flatten()
        .min();
        if state
            .output
            .front()
            .is_some_and(|o| Instant::now() >= o.deadline)
        {
            return Err("emulator UART write deadline".into());
        }
        // WAIT: emulator-loop
        tokio::select! {
            _ = stop.changed() => return Ok(()),
            request = controls.recv() => {
                let Some(request) = request else { return Ok(()); };
                if !request.reply.is_closed() && Instant::now() < request.deadline {
                    let result = state.control(request.action);
                    let fatal = result.as_ref().err().is_some_and(|e| e.contains("queue overflow"));
                    let _ = request.reply.send(result);
                    if fatal { return Err("emulator control caused queue overflow".into()); }
                }
            },
            read = reader.read(&mut bytes) => {
                let count = read.map_err(|e| e.to_string())?;
                if count == 0 { return Ok(()); }
                for command in state.framer.feed(&bytes[..count],Instant::now()) {
                    match command { Ok(command) => state.command(command)?, Err(_) => state.enqueue(b"X4\n".to_vec())? }
                }
            },
            written = async {
                match state.output.front() {
                    Some(frame) => {
                        // WAIT: emulator-loop
                        writer.write(&frame.bytes[frame.offset..]).await
                    },
                    None => {
                        // WAIT: emulator-loop
                        std::future::pending().await
                    },
                }
            } => {
                let count = written.map_err(|e| e.to_string())?;
                if count == 0 { return Err("emulator UART write returned zero".into()); }
                let frame = state.output.front_mut().unwrap();
                state.unterminated_output = frame.bytes[frame.offset+count-1] != b'\n';
                frame.offset += count;
                if frame.offset == frame.bytes.len() { state.output.pop_front(); }
            },
            _ = due(deadline) => {
                for _ in state.framer.feed(&[],Instant::now()) { state.enqueue(b"X4\n".to_vec())?; }
            },
        }
    }
}

#[cfg(test)]
mod tests;
