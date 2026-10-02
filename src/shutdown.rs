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

/// SIGTERM (systemd's stop) and SIGINT (Ctrl-C), registered at once by [`StopSignals::install`],
/// first thing in `run` — not on the first wait — so a stop during start-up takes the bridge's own
/// path instead of the signal's default action, which ends the process with nothing logged and
/// the log unflushed (fleet class F3).
pub struct StopSignals {
    #[cfg(unix)]
    term: Option<tokio::signal::unix::Signal>,
    #[cfg(unix)]
    int: Option<tokio::signal::unix::Signal>,
}

/// A stop signal whose handler could not be installed: it keeps its default action.
#[derive(Debug)]
pub struct Unavailable {
    pub signal: &'static str,
    pub error: std::io::Error,
}

impl StopSignals {
    /// Installs both handlers; the ones that could not be installed come back, to be logged once
    /// logging is up ([`report_unavailable`]).
    pub fn install() -> (StopSignals, Vec<Unavailable>) {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut unavailable = Vec::new();
            let mut install = |name: &'static str, kind: SignalKind| match signal(kind) {
                Ok(handler) => Some(handler),
                Err(error) => {
                    unavailable.push(Unavailable { signal: name, error });
                    None
                }
            };
            let term = install("SIGTERM", SignalKind::terminate());
            let int = install("SIGINT", SignalKind::interrupt());
            (StopSignals { term, int }, unavailable)
        }
        #[cfg(not(unix))]
        {
            (StopSignals {}, Vec::new())
        }
    }

    /// The next stop signal's name. A signal without a handler never comes; with neither, this
    /// never returns — `supervise` races it with the session's end, so it holds nothing.
    pub async fn recv(&mut self) -> &'static str {
        #[cfg(unix)]
        {
            // WAIT: dali-signal
            tokio::select! {
                Some(()) = next(&mut self.term) => "SIGTERM",
                Some(()) = next(&mut self.int) => "SIGINT",
                else => never().await,
            }
        }
        #[cfg(not(unix))]
        {
            // WAIT: dali-signal
            match tokio::signal::ctrl_c().await {
                Ok(()) => "SIGINT",
                Err(_) => never().await,
            }
        }
    }
}

/// One handler's next signal; never, without the handler.
#[cfg(unix)]
async fn next(handler: &mut Option<tokio::signal::unix::Signal>) -> Option<()> {
    match handler {
        Some(handler) => {
            // WAIT: dali-signal
            handler.recv().await
        }
        None => never().await,
    }
}

/// Logs each stop signal whose handler could not be installed (`signal_handler_unavailable`).
pub fn report_unavailable(unavailable: &[Unavailable]) {
    for missing in unavailable {
        warn!(kind = "signal_handler_unavailable", signal = missing.signal, error = %missing.error,
              "a stop signal's handler could not be installed; that signal ends the bridge without its bounded shutdown");
    }
}

/// Never resolves.
async fn never<T>() -> T {
    // WAIT: dali-signal
    std::future::pending::<T>().await
}

#[cfg(test)]
mod tests;
