//! Serialized durable inventory ownership; no hardware or broker access.
//!
//! Close before the managed service returns. The runtime owns the blocking
//! worker, so a stalled filesystem counts as incomplete service shutdown.
use crate::{
    config::ConfigPaths,
    inventory::{Edit, Model},
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc,
};
use tokio::{
    sync::{oneshot, watch},
    time::{timeout_at, Instant},
};
mod disk;
mod worker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Writing,
    Uncertain(String),
    Stopping,
    Closed,
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub model: Arc<Model>,
    pub phase: Phase,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Committed { revision: u64, changed: bool },
    NotCommitted(String),
    Uncertain(String),
}
struct Control {
    state: watch::Sender<Option<Snapshot>>,
    done: watch::Sender<bool>,
    closing: AtomicBool,
    busy: AtomicBool,
    active: AtomicU64,
    next: AtomicU64,
}
impl Control {
    fn publish(&self, model: Arc<Model>, phase: Phase) {
        self.state.send_replace(Some(Snapshot { model, phase }));
    }
    fn cancel(&self, id: u64, canceled: &AtomicBool) {
        self.state.send_if_modified(|snapshot| {
            if self.active.load(Ordering::SeqCst) != id {
                return false;
            }
            // Cancellation and successful finalization share this lock. If
            // finalization won, the active result is already durably known.
            canceled.store(true, Ordering::SeqCst);
            if let Some(snapshot) = snapshot {
                if snapshot.phase == Phase::Writing {
                    snapshot.phase =
                        Phase::Uncertain("caller ended before the filesystem worker".into());
                    return true;
                }
            }
            false
        });
    }
}
struct Client {
    send: mpsc::SyncSender<Request>,
    control: Arc<Control>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.control.closing.store(true, Ordering::SeqCst);
    }
}
#[derive(Clone)]
pub struct ConfigStore {
    client: Arc<Client>,
}
enum Operation {
    Edit { expected: u64, edits: Vec<Edit> },
    Recover,
}
struct Request {
    operation: Operation,
    deadline: Instant,
    canceled: Arc<AtomicBool>,
    reply: oneshot::Sender<Outcome>,
}
impl Request {
    fn expired(&self, control: &Control) -> bool {
        self.canceled.load(Ordering::SeqCst)
            || control.closing.load(Ordering::SeqCst)
            || Instant::now() >= self.deadline
    }
}
struct CancelRequest {
    control: Arc<Control>,
    id: u64,
    canceled: Arc<AtomicBool>,
    armed: bool,
}
impl Drop for CancelRequest {
    fn drop(&mut self) {
        if self.armed {
            self.control.cancel(self.id, &self.canceled);
        }
    }
}
struct CancelOpen {
    control: Arc<Control>,
    armed: bool,
}
impl Drop for CancelOpen {
    fn drop(&mut self) {
        if self.armed {
            self.control.closing.store(true, Ordering::SeqCst);
        }
    }
}
impl ConfigStore {
    /// Acquire ownership before import/admission. All filesystem work runs on
    /// one blocking worker, including recovery and releasing the lifetime lock.
    pub async fn open(paths: ConfigPaths, name: String, deadline: Instant) -> Result<Self, String> {
        Self::open_with(paths, name, deadline, disk::Hooks::default()).await
    }
    async fn open_with(
        paths: ConfigPaths,
        name: String,
        deadline: Instant,
        hooks: disk::Hooks,
    ) -> Result<Self, String> {
        if Instant::now() >= deadline {
            return Err("configuration startup deadline".into());
        }
        let (send, receive) = mpsc::sync_channel(1);
        let control = Arc::new(Control {
            state: watch::channel(None).0,
            done: watch::channel(false).0,
            closing: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            active: AtomicU64::new(0),
            next: AtomicU64::new(1),
        });
        let mut guard = CancelOpen {
            control: control.clone(),
            armed: true,
        };
        let (ready, result) = oneshot::channel();
        let task_control = control.clone();
        // Runtime teardown must include this worker and its lifetime lock.
        drop(tokio::task::spawn_blocking(move || {
            worker::run(paths, name, hooks, task_control, receive, ready)
        }));
        // WAIT: v2-store-open
        let result = timeout_at(deadline, result)
            .await
            .map_err(|_| "configuration startup deadline")?;
        if Instant::now() >= deadline {
            return Err("configuration startup deadline".into());
        }
        result.map_err(|_| "configuration worker ended during startup")??;
        guard.armed = false;
        Ok(Self {
            client: Arc::new(Client { send, control }),
        })
    }
    /// A detached clone; no watch borrow/lock escapes into caller code.
    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = self
            .client
            .control
            .state
            .borrow()
            .as_ref()
            .expect("opened configuration")
            .clone();
        if self.client.control.closing.load(Ordering::SeqCst) && snapshot.phase != Phase::Closed {
            snapshot.phase = Phase::Stopping;
        }
        snapshot
    }
    pub async fn edit(&self, expected: u64, edits: Vec<Edit>, deadline: Instant) -> Outcome {
        self.submit(Operation::Edit { expected, edits }, deadline)
            .await
    }
    pub async fn recover(&self, deadline: Instant) -> Outcome {
        self.submit(Operation::Recover, deadline).await
    }
    async fn submit(&self, operation: Operation, deadline: Instant) -> Outcome {
        let control = &self.client.control;
        if Instant::now() >= deadline || control.closing.load(Ordering::SeqCst) {
            return Outcome::NotCommitted("closed or expired request".into());
        }
        if control
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Outcome::NotCommitted("configuration worker is busy".into());
        }
        let previous = self.snapshot();
        let allowed = previous.phase == Phase::Ready
            || (matches!(operation, Operation::Recover)
                && matches!(previous.phase, Phase::Uncertain(_)));
        if !allowed || control.closing.load(Ordering::SeqCst) {
            control.busy.store(false, Ordering::SeqCst);
            return Outcome::NotCommitted("configuration requires recovery or is closed".into());
        }
        let id = match control
            .next
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
        {
            Ok(id) => id,
            Err(_) => {
                control.busy.store(false, Ordering::SeqCst);
                return Outcome::NotCommitted("request counter exhausted".into());
            }
        };
        control.active.store(id, Ordering::SeqCst);
        control.publish(previous.model.clone(), Phase::Writing);
        let canceled = Arc::new(AtomicBool::new(false));
        let (reply, receive) = oneshot::channel();
        let request = Request {
            operation,
            deadline,
            canceled: canceled.clone(),
            reply,
        };
        if self.client.send.try_send(request).is_err() {
            control.active.store(0, Ordering::SeqCst);
            control.publish(previous.model, previous.phase);
            control.busy.store(false, Ordering::SeqCst);
            return Outcome::NotCommitted("configuration worker ended".into());
        }
        let mut guard = CancelRequest {
            control: control.clone(),
            id,
            canceled,
            armed: true,
        };
        // WAIT: v2-store-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Outcome::Uncertain(
                "configuration request deadline; inspect status and recover".into(),
            );
        }
        match result {
            Ok(Ok(outcome)) => {
                guard.armed = false;
                outcome
            }
            _ => Outcome::Uncertain(
                "configuration worker result unavailable; inspect status and recover".into(),
            ),
        }
    }
    /// True means the worker released ownership even across retained handles.
    /// False requires the supervisor's incomplete-stop path.
    pub async fn close(&self, deadline: Instant) -> bool {
        self.client.control.closing.store(true, Ordering::SeqCst);
        let mut done = self.client.control.done.subscribe();
        // WAIT: v2-store-close
        let result = timeout_at(deadline, async {
            while !*done.borrow_and_update() {
                if done.changed().await.is_err() {
                    return false;
                }
            }
            true
        })
        .await;
        Instant::now() < deadline && result == Ok(true)
    }
}
#[cfg(test)]
mod tests;
