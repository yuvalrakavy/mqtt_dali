//! `main`'s side of a stop: bounded whatever the session does (finding C-7). Each test runs under
//! its own outer deadline, so a supervise that never returns fails instead of hanging.

use std::time::{Duration, Instant};

use tokio::sync::{oneshot, watch};

use super::*;

/// Longer than any bound under test: past it, `supervise` never returned.
const OUTER: Duration = Duration::from_secs(10);

async fn bounded(supervising: impl Future<Output = Ended>) -> Ended {
    tokio::time::timeout(OUTER, supervising)
        .await
        .unwrap_or_else(|_| {
            panic!("supervise did not return within {OUTER:?}: main's shutdown is unbounded")
        })
}

#[tokio::test]
async fn a_session_that_never_answers_its_stop_is_left_behind_in_bounded_time() {
    // The session holds its result sender and never sends: it is stuck (a long DALI command, a
    // publish against a stalled broker).
    let (_session, done) = oneshot::channel();
    let (stop, stop_rx) = watch::channel(false);
    let bound = Duration::from_millis(300);
    let start = Instant::now();
    let ended = bounded(supervise(done, stop, async {}, bound)).await;
    assert_eq!(ended, Ended::Overran);
    assert!(*stop_rx.borrow(), "the session was never asked to stop");
    assert!(
        start.elapsed() >= bound,
        "gave up before its bound ({:?})",
        start.elapsed()
    );
}

#[tokio::test]
async fn a_session_that_answers_its_stop_is_stopped() {
    let (session, done) = oneshot::channel();
    let (stop, mut stop_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = stop_rx.wait_for(|stop| *stop).await;
        let _ = session.send(Ok(()));
    });
    let ended = bounded(supervise(done, stop, async {}, Duration::from_secs(5))).await;
    assert_eq!(ended, Ended::Stopped);
}

#[tokio::test]
async fn a_session_that_dies_ends_the_bridge_without_a_signal() {
    let (session, done) = oneshot::channel::<crate::mqtt::Result<()>>();
    let (stop, _stop_rx) = watch::channel(false);
    drop(session);
    let ended = bounded(supervise(
        done,
        stop,
        std::future::pending(),
        Duration::from_secs(5),
    ))
    .await;
    assert_eq!(ended, Ended::Panicked);
}
