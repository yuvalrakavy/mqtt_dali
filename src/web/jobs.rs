//! Bounded process-local results around existing service command ownership.
use crate::service::{PreparedCommand, ServiceError, ServiceHandle};
use futures_util::{stream::FuturesUnordered, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

const ACTIVE: usize = 4;
const HISTORY: usize = 32;
const RESULT_BYTES: usize = 32 * 1024;
const SCENE_RESULT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Serialize)]
pub(super) struct Record {
    pub id: String,
    pub command: String,
    pub target: Option<String>,
    pub status: &'static str,
    pub result: Option<Value>,
    pub error: Option<Value>,
}
struct Request {
    id: String,
    command: PreparedCommand,
    _permit: OwnedSemaphorePermit,
}
pub(super) struct Receiver(mpsc::Receiver<Request>);
pub(super) struct Jobs {
    send: mpsc::Sender<Request>,
    pub(super) slots: Arc<Semaphore>,
    records: Mutex<VecDeque<Record>>,
    pub(super) changed: watch::Sender<u64>,
    #[cfg(test)]
    pub(super) observed_contexts: Mutex<Vec<mqtt_bridge_kit::RequestContext>>,
}
impl Jobs {
    #[cfg(test)]
    fn observe_context(&self, context: &mqtt_bridge_kit::RequestContext) {
        self.observed_contexts.lock().unwrap().push(context.clone());
    }
    pub(super) fn new() -> (Self, Receiver) {
        let (send, receive) = mpsc::channel(ACTIVE);
        (
            Self {
                send,
                slots: Arc::new(Semaphore::new(ACTIVE)),
                records: Mutex::new(VecDeque::new()),
                changed: watch::channel(0).0,
                #[cfg(test)]
                observed_contexts: Mutex::new(Vec::new()),
            },
            Receiver(receive),
        )
    }
    pub(super) fn list(&self) -> Vec<Record> {
        // WAIT: web-jobs-book
        self.records
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|record| Record {
                id: record.id.clone(),
                command: record.command.clone(),
                target: record.target.clone(),
                status: record.status,
                result: None,
                error: record.error.clone(),
            })
            .collect()
    }
    pub(super) fn get(&self, id: &str) -> Option<Record> {
        if id.len() != 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        // WAIT: web-jobs-book
        self.records
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|r| r.id == id)
            .cloned()
    }
    pub(super) fn admit(
        &self,
        name: String,
        target: Option<String>,
        command: PreparedCommand,
    ) -> Option<Record> {
        let permit = self.slots.clone().try_acquire_owned().ok()?;
        // WAIT: web-jobs-book
        let mut records = self.records.lock().unwrap_or_else(|p| p.into_inner());
        // Reserve mailbox capacity before changing history; closed/full admission
        // must not erase an earlier outcome.
        let slot = self.send.try_reserve().ok()?;
        if records.len() >= HISTORY {
            let oldest = records.iter().position(|r| r.status != "running")?;
            records.remove(oldest);
        }
        let id = loop {
            let id = format!("{:032x}", rand::random::<u128>());
            if !records.iter().any(|r| r.id == id) {
                break id;
            }
        };
        let record = Record {
            id: id.clone(),
            command: name,
            target,
            status: "running",
            result: None,
            error: None,
        };
        records.push_back(record.clone());
        slot.send(Request {
            id,
            command,
            _permit: permit,
        });
        drop(records);
        self.changed
            .send_modify(|value| *value = value.wrapping_add(1));
        Some(record)
    }
    fn finish(&self, id: String, result: Result<Value, ServiceError>) {
        // Preserve the private cause in test output before public sanitization.
        #[cfg(test)]
        if let Err(error) = &result {
            eprintln!("web job {id} failed: {error:?}");
        }
        // WAIT: web-jobs-book
        let mut records = self.records.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            let (result, error) = if matches!(
                record.command.as_str(),
                "read_scene_settings" | "read_group_output" | "set_group_output"
            ) {
                public_result_with_budget(result, SCENE_RESULT_BYTES)
            } else {
                public_result(result)
            };
            record.status = if error.is_none() {
                "succeeded"
            } else {
                "failed"
            };
            record.result = result;
            record.error = error;
            drop(records);
            self.changed
                .send_modify(|value| *value = value.wrapping_add(1));
        }
    }
}
pub(super) fn public_result(result: Result<Value, ServiceError>) -> (Option<Value>, Option<Value>) {
    public_result_with_budget(result, RESULT_BYTES)
}
fn public_result_with_budget(
    result: Result<Value, ServiceError>,
    limit: usize,
) -> (Option<Value>, Option<Value>) {
    let code = match result {
        Ok(mut value) => {
            // Reports, revisions and accepted scalar states reach here. A
            // report's free-form reason can contain a private save failure.
            if value.get("reason").is_some_and(|r| !r.is_null()) {
                value["reason"] = json!("Operation was not fully confirmed; inspect configuration and hardware before retrying.");
            }
            if serde_json::to_vec(&value).is_ok_and(|bytes| bytes.len() <= limit) {
                return (Some(value), None);
            }
            "result_unavailable"
        }
        Err(ServiceError::Busy) => "busy",
        Err(ServiceError::Closed) => "closed",
        Err(ServiceError::Expired) => "expired",
        Err(ServiceError::Rejected(_)) => "refused",
        Err(ServiceError::Uncertain(_)) => "uncertain",
    };
    (
        None,
        Some(
            json!({"code":code,"message":"Operation did not return a confirmed result. Inspect current configuration and hardware before retrying."}),
        ),
    )
}
pub(super) async fn run(
    jobs: Arc<Jobs>,
    mut receiver: Receiver,
    handle: ServiceHandle,
    stop: CancellationToken,
) {
    let mut active = FuturesUnordered::new();
    loop {
        // WAIT: web-jobs-loop
        tokio::select! {
            biased;
            _ = stop.cancelled() => return,
            Some((id, result, permit)) = active.next(), if !active.is_empty() => {
                jobs.finish(id, result);
                drop(permit);
            }
            request = receiver.0.recv() => {
                let Some(request) = request else { return; };
                let instance = handle.snapshot().map(|s| s.model.inventory().name.clone()).unwrap_or_default();
                let span = request.command.context.operation_span(&instance, "http_command");
                #[cfg(test)]
                jobs.observe_context(&request.command.context);
                let handle = handle.clone();
                active.push(async move {
                    // WAIT: web-job-request
                    let result = request.command.run(handle).await;
                    (request.id, result, request._permit)
                }.instrument(span));
            }
        }
    }
}
#[cfg(test)]
mod tests;
