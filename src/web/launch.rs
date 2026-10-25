//! A weak reference to the running HTTP owner; never persisted or Debug-formatted.
use super::server::State;
use std::sync::{Arc, Weak};
use tokio::time::Instant;

#[derive(Clone)]
pub(crate) struct Launch(Weak<State>);
impl Launch {
    pub(super) fn new(state: &Arc<State>) -> Self {
        Self(Arc::downgrade(state))
    }
    fn state(&self) -> Option<Arc<State>> {
        self.0.upgrade().filter(|state| !state.stop.is_cancelled())
    }
    pub(crate) fn url(&self) -> Option<String> {
        self.state()
            .map(|state| format!("{}/login", state.settings.origin))
    }
    pub(crate) fn issue(&self) -> Result<serde_json::Value, String> {
        let state = self.state().ok_or("web access is disabled")?;
        // WAIT: web-auth
        let token = state
            .auth
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .issue_launch(Instant::now())
            .map_err(|_| "too many pending configuration links")?;
        Ok(
            serde_json::json!({"url":format!("{}/login#token={token}",state.settings.origin),"expires_in_ms":120000}),
        )
    }
}
