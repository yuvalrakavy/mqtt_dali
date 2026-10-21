//! A task's result becomes visible only after its real JoinHandle completes.
use std::future::Future;
use tokio::{sync::watch, task::AbortHandle};

#[derive(Clone, Debug)]
pub(super) struct Failure {
    pub canceled: bool,
}
pub(super) type Completion<T> = watch::Receiver<Option<Result<T, Failure>>>;

pub(super) fn start<T, F>(operation: F) -> (AbortHandle, Completion<T>)
where
    T: Clone + Send + Sync + 'static,
    F: Future<Output = T> + Send + 'static,
{
    start_with_failure_cleanup(operation, || std::future::ready(()))
}

/// Keep failure teardown owned even when nobody is waiting for close. Cleanup
/// must use its owner's original absolute deadline and release captured resources
/// before returning. The join result is published only afterward.
pub(super) fn start_with_failure_cleanup<T, F, C, CF>(
    operation: F,
    cleanup: C,
) -> (AbortHandle, Completion<T>)
where
    T: Clone + Send + Sync + 'static,
    F: Future<Output = T> + Send + 'static,
    C: FnOnce() -> CF + Send + 'static,
    CF: Future<Output = ()> + Send,
{
    let task = tokio::spawn(operation);
    let abort = task.abort_handle();
    let (send, receive) = watch::channel(None);
    // No reference back to the caller. Even if a close future is canceled,
    // retain both the real join and any required failure cleanup through teardown.
    tokio::spawn(async move {
        // WAIT: atx-owner-join
        let result = task.await.map_err(|error| Failure {
            canceled: error.is_cancelled(),
        });
        if result.is_err() {
            // WAIT: atx-owner-failure-cleanup
            cleanup().await;
        } else {
            drop(cleanup);
        }
        send.send_replace(Some(result));
    });
    (abort, receive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Condvar, Mutex};
    use tokio::time::{timeout, Duration};

    struct Release(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0 .0.lock().unwrap() = true;
            self.0 .1.notify_one();
        }
    }
    struct HeldDrop {
        gate: Arc<(Mutex<bool>, Condvar)>,
        entered: Option<tokio::sync::oneshot::Sender<()>>,
    }
    impl Drop for HeldDrop {
        fn drop(&mut self) {
            let _ = self.entered.take().unwrap().send(());
            let (lock, signal) = &*self.gate;
            let released = lock.lock().unwrap();
            drop(signal.wait_while(released, |released| !*released).unwrap());
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn join_completion_waits_for_canceled_future_destructors() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let release = Release(Arc::clone(&gate));
        let (entered, dropping) = tokio::sync::oneshot::channel();
        let (ready, running) = tokio::sync::oneshot::channel();
        let owned = HeldDrop {
            gate,
            entered: Some(entered),
        };
        let (abort, mut completion) = start(async move {
            let _owned = owned;
            let _ = ready.send(());
            std::future::pending::<()>().await;
        });
        timeout(Duration::from_secs(1), running)
            .await
            .unwrap()
            .unwrap();
        abort.abort();
        timeout(Duration::from_secs(1), dropping)
            .await
            .unwrap()
            .unwrap();
        assert!(
            completion.borrow().is_none(),
            "task was declared joined while its owned resource was still dropping"
        );
        drop(release);
        timeout(Duration::from_secs(1), completion.wait_for(Option::is_some))
            .await
            .unwrap()
            .unwrap();
        assert!(
            completion
                .borrow()
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err()
                .canceled
        );
    }
}
