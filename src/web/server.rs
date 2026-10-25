use super::{
    Settings,
    auth::{Auth, Policy},
    handlers, jobs, transport,
};
use crate::service::ServiceHandle;
use std::{
    future::IntoFuture,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, watch},
    task::JoinHandle,
    time::{Instant, timeout_at},
};
use tokio_util::sync::CancellationToken;

pub(super) struct State {
    pub settings: Settings,
    pub handle: ServiceHandle,
    pub auth: Mutex<Auth>,
    pub stop: CancellationToken,
    pub revoked: watch::Sender<u64>,
    pub streams: Arc<Semaphore>,
    pub handlers: Arc<Semaphore>,
    pub jobs: Arc<jobs::Jobs>,
}
pub(crate) struct Server {
    task: Option<JoinHandle<std::io::Result<()>>>,
    pub(super) state: Arc<State>,
    force: CancellationToken,
    pub(super) connections: Arc<Semaphore>,
    #[cfg(test)]
    pub(super) address: std::net::SocketAddr,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.state.stop.cancel();
        self.force_connections();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl Server {
    fn force_connections(&self) {
        self.force.cancel();
    }
    pub(crate) async fn start(
        settings: Settings,
        handle: ServiceHandle,
        deadline: Instant,
    ) -> Result<Self, String> {
        if Instant::now() >= deadline {
            return Err("web listener startup expired".into());
        }
        // WAIT: web-bind
        let listener = timeout_at(deadline, TcpListener::bind(settings.bind))
            .await
            .map_err(|_| "web listener startup expired")?
            .map_err(|e| format!("web listener could not bind: {e}"))?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err("web listener startup expired".into());
        }
        let (jobs, receiver) = jobs::Jobs::new();
        let state = Arc::new(State {
            auth: Mutex::new(Auth::new(settings.password, Policy::default())),
            settings,
            handle,
            stop: CancellationToken::new(),
            revoked: watch::channel(0).0,
            streams: Arc::new(Semaphore::new(8)),
            handlers: Arc::new(Semaphore::new(32)),
            jobs: Arc::new(jobs),
        });
        let force = CancellationToken::new();
        let connections = Arc::new(Semaphore::new(transport::CONNECTIONS as usize));
        let listener = transport::Listener {
            listener,
            slots: connections.clone(),
            force: force.clone(),
        };
        let router = handlers::router(state.clone());
        let stop = state.stop.clone();
        let job_loop = jobs::run(
            state.jobs.clone(),
            receiver,
            state.handle.clone(),
            stop.clone(),
        );
        let task = tokio::spawn(async move {
            let http = axum::serve(listener, router)
                .with_graceful_shutdown(stop.clone().cancelled_owned())
                .into_future();
            tokio::pin!(http, job_loop);
            // Both futures belong to this one task. Dropping/aborting it also
            // drops every command receiver; no job task survives HTTP ownership.
            // WAIT: web-serve
            tokio::select! {
                result = &mut http => result,
                _ = &mut job_loop => {
                    if !stop.is_cancelled() {
                        return Err(std::io::Error::other("web command owner ended unexpectedly"));
                    }
                    // WAIT: web-serve
                    http.await
                }
            }
        });
        state.handle.set_web_launch(super::Launch::new(&state));
        tracing::info!(listen_address=%address,"Web access enabled");
        Ok(Self {
            task: Some(task),
            state,
            force,
            connections,
            #[cfg(test)]
            address,
        })
    }
    pub(crate) fn begin_stop(&self) {
        self.state.stop.cancel();
    }
    pub(crate) async fn finished(&mut self) -> Result<(), String> {
        let Some(task) = self.task.as_mut() else {
            // WAIT: web-serve
            return std::future::pending().await;
        };
        // WAIT: web-serve
        let result = task.await;
        self.task.take();
        result
            .map_err(|e| format!("web task failed: {e}"))?
            .map_err(|e| e.to_string())
    }
    pub(crate) async fn close(&mut self, deadline: Instant) -> bool {
        self.begin_stop();
        let soft = deadline
            .checked_sub(Duration::from_millis(100))
            .unwrap_or_else(Instant::now);
        let mut clean = true;
        if let Some(mut task) = self.task.take() {
            // WAIT: web-close
            match timeout_at(soft, &mut task).await {
                Ok(result) => clean = result.is_ok_and(|r| r.is_ok()),
                Err(_) => {
                    self.force_connections();
                    task.abort();
                    // WAIT: web-close
                    clean = timeout_at(deadline, &mut task)
                        .await
                        .is_ok_and(|r| r.is_ok() || r.is_err_and(|e| e.is_cancelled()));
                }
            }
        }
        self.force_connections();
        // WAIT: web-close
        let destroyed = timeout_at(
            deadline,
            self.connections
                .clone()
                .acquire_many_owned(transport::CONNECTIONS),
        )
        .await
        .is_ok_and(|r| r.is_ok());
        clean && destroyed && Instant::now() <= deadline
    }
}
pub(crate) async fn changed(server: &mut Option<Server>) -> Result<(), String> {
    match server {
        Some(server) => server.finished().await,
        None => {
            // WAIT: web-serve
            std::future::pending().await
        }
    }
}
