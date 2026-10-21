//! Quiet acquisition observes raw reader activity; it never reads a second time.
use super::*;
use std::sync::atomic::AtomicU8;
use tokio::time::{timeout_at, Instant};

/// Shared by the caller and reader: canceling a caller never needs actor polling.
#[derive(Default)]
pub(crate) struct RecoveryCancellation(std::sync::Mutex<bool>);
impl RecoveryCancellation {
    pub(crate) fn cancel(&self) {
        // WAIT: atx-recovery-cancel
        *self.0.lock().unwrap() = true;
    }
    pub(crate) fn is_canceled(&self) -> bool {
        // WAIT: atx-recovery-cancel
        *self.0.lock().unwrap()
    }
}

#[derive(Clone, Copy)]
pub(super) struct Activity {
    pub generation: u64,
    pub at: Instant,
}
pub(super) struct FenceRequest {
    pub deadline: Instant,
    pub reset: Option<Arc<Reset>>,
    pub reply: oneshot::Sender<Result<ReceiveFence, TransportError>>,
}
pub(super) struct Reset {
    generation: u64,
    deadline: Instant,
    phase: AtomicU8,
    cancellation: Arc<RecoveryCancellation>,
}
impl Reset {
    // Called only at explicit Empty under the split-stream lock. Cancellation
    // and reader authorization compete for the same pending transition.
    pub fn authorize(&self, activity: Activity) -> Option<std::sync::MutexGuard<'_, bool>> {
        // Hold through the framing reset. A caller's synchronous Drop uses this
        // same lock, independently of whether the actor has observed reply closure.
        // WAIT: atx-recovery-cancel
        let canceled = self.cancellation.0.lock().unwrap();
        let authorized = !*canceled
            && Instant::now() < self.deadline
            && activity.generation == self.generation
            && self
                .phase
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
        authorized.then_some(canceled)
    }
    pub fn applied(&self) -> bool {
        self.phase.load(Ordering::Acquire) == 2
    }
}
struct CancelReset(Arc<Reset>);
impl Drop for CancelReset {
    fn drop(&mut self) {
        let _ = self
            .0
            .phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }
}
impl<S: DrainRead + AsyncWrite + Unpin + Send + 'static> Monitor<S> {
    pub(crate) fn activity_generation(&self) -> u64 {
        self.inner.state.activity.borrow().generation
    }
    pub(crate) fn write_is_poisoned(&self) -> bool {
        self.inner.state.poisoned.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub(crate) async fn quiet_fence(
        &self,
        quiet: Duration,
        deadline: Instant,
    ) -> Result<RecoveryFence, TransportError> {
        self.quiet_fence_using(quiet, deadline, Arc::new(RecoveryCancellation::default()))
            .await
    }
    pub(crate) async fn quiet_fence_using(
        &self,
        quiet: Duration,
        deadline: Instant,
        cancellation: Arc<RecoveryCancellation>,
    ) -> Result<RecoveryFence, TransportError> {
        if quiet.is_zero() || Instant::now() >= deadline {
            return Err(TransportError::Deadline);
        }
        let started = Instant::now();
        let mut activity = self.inner.state.activity.subscribe();
        let mut stop = self.inner.state.stop.subscribe();
        // WAIT: atx-quiet
        let result = timeout_at(deadline, async {
            loop {
                match self.status() {
                    LinkStatus::Closed => return Err(TransportError::Closed),
                    LinkStatus::WriteIndeterminate => return Err(TransportError::IndeterminateWrite),
                    LinkStatus::Monitoring => {},
                }
                let current = *activity.borrow_and_update();
                let until = current.at.max(started).checked_add(quiet).ok_or(TransportError::Deadline)?;
                tokio::select! {
                    biased;
                    _ = stopped(&mut stop) => return Err(TransportError::Closed),
                    changed = activity.changed() => { changed.map_err(|_| TransportError::Closed)?; continue; },
                    _ = tokio::time::sleep_until(until) => {},
                }
                let reset = Arc::new(Reset { generation: current.generation, deadline, phase: AtomicU8::new(0), cancellation: cancellation.clone() });
                let _cancel = CancelReset(reset.clone());
                let fence = self.fence_until(deadline, Some(reset)).await?;
                if fence.reset && fence.activity_generation == current.generation {
                    return Ok(RecoveryFence { sequence: fence.sequence, activity_generation: fence.activity_generation });
                }
            }
        }).await.map_err(|_| TransportError::Deadline)?;
        if Instant::now() >= deadline {
            return Err(TransportError::Deadline);
        }
        result
    }
}
