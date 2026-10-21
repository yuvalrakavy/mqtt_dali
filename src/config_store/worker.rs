use super::{
    disk::{self, Hooks, Stage},
    Control, Operation, Outcome, Phase, Request,
};
use crate::{
    config::{self, ConfigPaths},
    inventory::Model,
};
use std::{
    sync::{
        atomic::Ordering,
        mpsc::{Receiver, RecvTimeoutError},
        Arc,
    },
    time::Duration,
};
use tokio::sync::oneshot;

// Declared before the file owner: unwinding closes the file before completion.
struct Completion(Arc<Control>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.closing.store(true, Ordering::SeqCst);
        self.0.state.send_if_modified(|snapshot| {
            if let Some(snapshot) = snapshot {
                snapshot.phase = Phase::Closed;
                return true;
            }
            false
        });
        self.0.done.send_replace(true);
    }
}
struct Writer {
    paths: ConfigPaths,
    model: Arc<Model>,
    pending: Vec<Arc<Model>>,
    hooks: Hooks,
}
pub(super) fn run(
    paths: ConfigPaths,
    name: String,
    hooks: Hooks,
    control: Arc<Control>,
    receive: Receiver<Request>,
    ready: oneshot::Sender<Result<(), String>>,
) {
    let _completion = Completion(control.clone());
    let _lease = match disk::acquire(&paths) {
        Ok(lease) => lease,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if control.closing.load(Ordering::SeqCst) {
        return;
    }
    let startup = || {
        hooks.check(disk::Step::StartupBeforeLoad)?;
        if control.closing.load(Ordering::SeqCst) {
            return Err("configuration startup canceled".into());
        }
        let model = Arc::new(Model::new(
            config::load_or_migrate(&paths, &name)
                .map_err(|e| e.to_string())?
                .inventory,
        )?);
        hooks.check(disk::Step::StartupSync)?;
        // A previous process may have ended after rename but before dirsync.
        // Confirm the loaded mapping's durability before publishing it as Ready.
        let model = disk::recover(&paths.current, &[model], &Hooks::default())?;
        if control.closing.load(Ordering::SeqCst) {
            return Err("configuration startup canceled".into());
        }
        Ok(model)
    };
    let model = match startup() {
        Ok(model) => model,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut writer = Writer {
        paths,
        model: model.clone(),
        pending: vec![],
        hooks,
    };
    control.publish(model, Phase::Ready);
    if ready.send(Ok(())).is_err() {
        return;
    }
    while !control.closing.load(Ordering::SeqCst) {
        // WAIT: v2-store-worker
        let request = match receive.recv_timeout(Duration::from_millis(50)) {
            Ok(request) => request,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let result = writer.handle(&request, &control);
        control.active.store(0, Ordering::SeqCst);
        control.busy.store(false, Ordering::SeqCst);
        let _ = request.reply.send(result);
    }
}
impl Writer {
    fn commit(
        &mut self,
        model: Arc<Model>,
        request: &Request,
        control: &Control,
        changed: bool,
    ) -> Outcome {
        let mut accepted = false;
        control.state.send_if_modified(|snapshot| {
            if request.expired(control)
                || snapshot.as_ref().is_none_or(|s| s.phase != Phase::Writing)
            {
                return false;
            }
            *snapshot = Some(super::Snapshot {
                model: model.clone(),
                phase: Phase::Ready,
            });
            accepted = true;
            true
        });
        if !accepted {
            return self.uncertain(
                "caller ended before durable finalization; reload required".into(),
                control,
            );
        }
        let revision = model.inventory().revision;
        self.model = model;
        self.pending.clear();
        Outcome::Committed { revision, changed }
    }
    fn ready(&mut self, model: Arc<Model>, control: &Control) {
        self.model = model.clone();
        self.pending.clear();
        control.publish(model, Phase::Ready);
    }
    fn uncertain(&mut self, reason: String, control: &Control) -> Outcome {
        control.publish(self.model.clone(), Phase::Uncertain(reason.clone()));
        Outcome::Uncertain(reason)
    }
    fn handle(&mut self, request: &Request, control: &Control) -> Outcome {
        if let Err(error) = self.hooks.check(disk::Step::BeforeRequest) {
            if self.pending.is_empty() {
                self.ready(self.model.clone(), control);
                return Outcome::NotCommitted(error);
            }
            return self.uncertain(error, control);
        }
        if request.expired(control) {
            if self.pending.is_empty() {
                self.ready(self.model.clone(), control);
                return Outcome::NotCommitted(
                    "request canceled or expired before filesystem work".into(),
                );
            }
            return self.uncertain("recovery canceled or expired".into(), control);
        }
        match &request.operation {
            Operation::Recover => {
                if self.pending.is_empty() {
                    self.ready(self.model.clone(), control);
                    return Outcome::Committed {
                        revision: self.model.inventory().revision,
                        changed: false,
                    };
                }
                match disk::recover(&self.paths.current, &self.pending, &self.hooks) {
                    Ok(model) if !request.expired(control) => {
                        if let Err(error) = self.hooks.check(disk::Step::BeforeFinalize) {
                            return self.uncertain(error, control);
                        }
                        self.commit(model, request, control, false)
                    }
                    Ok(_) => self.uncertain(
                        "recovery deadline; durability must be reconfirmed".into(),
                        control,
                    ),
                    Err(error) => self.uncertain(error, control),
                }
            }
            Operation::Edit { expected, edits } => self.edit(*expected, edits, request, control),
        }
    }
    fn edit(
        &mut self,
        expected: u64,
        edits: &[crate::inventory::Edit],
        request: &Request,
        control: &Control,
    ) -> Outcome {
        let proposed = match self.model.edit(expected, edits) {
            Ok(model) => Arc::new(model),
            Err(error) => {
                self.ready(self.model.clone(), control);
                return Outcome::NotCommitted(error);
            }
        };
        if proposed.inventory() == self.model.inventory() {
            self.ready(self.model.clone(), control);
            return Outcome::Committed {
                revision: self.model.inventory().revision,
                changed: false,
            };
        }
        let mut reservation = self.model.inventory().clone();
        reservation.revision_high_water = Some(proposed.inventory().revision);
        reservation.identity_ledger = Some(self.model.ledger().clone());
        let preflight = Model::new(reservation).and_then(|reservation| {
            let reservation_bytes = disk::bytes(&reservation)?;
            let proposed_bytes = disk::bytes(&proposed)?;
            Ok((Arc::new(reservation), reservation_bytes, proposed_bytes))
        });
        let (reservation, reservation_bytes, proposed_bytes) = match preflight {
            Ok(values) => values,
            Err(error) => {
                self.ready(self.model.clone(), control);
                return Outcome::NotCommitted(error);
            }
        };
        if request.expired(control) {
            self.ready(self.model.clone(), control);
            return Outcome::NotCommitted("request expired during preflight".into());
        }
        self.pending = vec![self.model.clone(), reservation.clone()];
        if let Err(error) = disk::replace(
            &self.paths.current,
            self.model.inventory(),
            &reservation_bytes,
            Stage::Reservation,
            &self.hooks,
        ) {
            if error.uncertain {
                return self.uncertain(error.reason, control);
            }
            self.ready(self.model.clone(), control);
            return Outcome::NotCommitted(error.reason);
        }
        self.model = reservation;
        if request.expired(control) {
            return self.uncertain(
                "request ended after revision reservation; reload required".into(),
                control,
            );
        }
        // Only a durably reserved revision may now become a public mapping.
        self.pending = vec![self.model.clone(), proposed.clone()];
        if let Err(error) = disk::replace(
            &self.paths.current,
            self.model.inventory(),
            &proposed_bytes,
            Stage::Mapping,
            &self.hooks,
        ) {
            if error.uncertain {
                return self.uncertain(error.reason, control);
            }
            self.ready(self.model.clone(), control);
            return Outcome::NotCommitted(error.reason);
        }
        if request.expired(control) {
            return self.uncertain(
                "mapping save finished after caller ended; reload required".into(),
                control,
            );
        }
        if let Err(error) = self.hooks.check(disk::Step::BeforeFinalize) {
            return self.uncertain(error, control);
        }
        self.commit(proposed, request, control, true)
    }
}
