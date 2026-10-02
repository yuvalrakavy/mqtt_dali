//! Stopping the bridge (Store no-hang §14.3: shutdown is bounded as a whole; finding C-7).
//!
//! SIGTERM (systemd's stop) or SIGINT sets the session's stop flag, and `main` waits for the
//! session to end within [`SHUTDOWN_BOUND`]. The session sees the flag between commands and says
//! its goodbye within `mqtt::GOODBYE_BOUND`; a session held longer — inside a long DALI command,
//! or a publish against a stalled broker — is left behind: `main` returns anyway, its logging
//! guard is dropped (flushing what it holds), and the process exit ends the session's thread, after
//! which the broker's will reports the bridge inactive.

use std::future::Future;
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use tracing::{error, info, warn};

/// How long `main` waits for the session after asking it to stop: its goodbye (2 s) and the end
/// of a DALI command it may be inside.
pub const SHUTDOWN_BOUND: Duration = Duration::from_secs(5);

/// How the bridge ended.
#[derive(Debug, PartialEq)]
pub enum Ended {
    /// Asked to stop, and the session stopped in time.
    Stopped,
    /// Asked to stop, and the session did not stop within the bound.
    Overran,
    /// The session ended by itself with an error.
    Failed,
    /// The session's thread ended without a result: it panicked.
    Panicked,
}

/// What the session thread hands back (`mqtt::spawn`).
pub type Done = oneshot::Receiver<crate::mqtt::Result<()>>;

/// Waits for `signal`, or for the session to end by itself. On the signal, sets `stop` and waits
/// for the session for at most `bound`.
pub async fn supervise(
    mut done: Done,
    stop: watch::Sender<bool>,
    signal: impl Future<Output = ()>,
    bound: Duration,
) -> Ended {
    // WAIT: dali-supervise
    tokio::select! {
        ended = &mut done => return session_ended(ended, false),
        () = signal => {}
    }
    stop.send_replace(true);
    // WAIT: dali-shutdown
    match tokio::time::timeout(bound, done).await {
        Ok(ended) => session_ended(ended, true),
        Err(_) => {
            warn!(
                kind = "shutdown_timeout",
                bound_ms = bound.as_millis() as u64,
                "the DALI session did not stop in time; exiting without it"
            );
            Ended::Overran
        }
    }
}

fn session_ended(
    ended: Result<crate::mqtt::Result<()>, oneshot::error::RecvError>,
    asked: bool,
) -> Ended {
    match ended {
        Ok(Ok(())) if asked => {
            info!("the DALI bridge stopped");
            Ended::Stopped
        }
        Ok(Ok(())) => {
            info!("the DALI bridge stopped without being asked to");
            Ended::Stopped
        }
        Ok(Err(e)) => {
            // `run` reconnects after every session failure; ending with an error is a bug.
            error!(kind = "session_failed", error = ?e, "the DALI session ended with an error");
            Ended::Failed
        }
        // The session thread logged its panic.
        Err(_) => Ended::Panicked,
    }
}

/// Resolves on SIGTERM or SIGINT. A signal whose handler cannot be installed never resolves this.
pub async fn stop_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                // WAIT: dali-signal
                tokio::select! {
                    Some(()) = term.recv() => info!("SIGTERM: stopping"),
                    Ok(()) = tokio::signal::ctrl_c() => info!("SIGINT: stopping"),
                    // Both streams gone: no signal can come.
                    else => never().await,
                }
                return;
            }
            Err(e) => {
                warn!(kind = "external_failure", error = %e, "no SIGTERM handler; only SIGINT stops the bridge cleanly");
            }
        }
    }
    // WAIT: dali-signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("SIGINT: stopping"),
        Err(e) => {
            warn!(kind = "external_failure", error = %e, "no SIGINT handler either; the bridge stops only when killed");
            never().await;
        }
    }
}

/// Never resolves: `supervise` races the signal with the session's end, so this holds nothing.
async fn never() {
    // WAIT: dali-signal
    std::future::pending::<()>().await
}

#[cfg(test)]
mod tests;
