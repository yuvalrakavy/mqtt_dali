use super::*;
use crate::config_store::{ConfigStore, Outcome, Phase as DiskPhase};
use mqtt_bridge_kit::{ProcessService, Reporter, ServiceContext};
use std::time::Duration;
use tracing::Instrument;

struct Owner {
    store: ConfigStore,
    model: Arc<Model>,
    table: TargetGenerations,
    reporter: Option<Reporter>,
    // Only a confirmed durable candidate may retry the exact original CAS.
    pending: Option<Arc<Model>>,
}
struct AdmissionGuard(ServiceHandle, watch::Sender<Option<Snapshot>>);
impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        self.0.permit.close();
        let snapshot = self.0.snapshot();
        if let Some(mut snapshot) = snapshot {
            if snapshot.phase != Phase::Closed {
                snapshot.phase = Phase::Stopping;
                self.1.send_replace(Some(snapshot.clone()));
                let _ = self.0.updates.send(snapshot);
            }
        }
    }
}
impl DaliService {
    fn update_snapshot(&self, model: Arc<Model>, phase: Phase) -> Snapshot {
        let snapshot = Snapshot {
            outputs: output::retained(
                &model,
                self.handle
                    .snapshot()
                    .map(|snapshot| snapshot.outputs)
                    .unwrap_or_default(),
            ),
            model,
            phase,
            hardware: self
                .handle
                .snapshot()
                .map_or(Hardware::NotStarted, |snapshot| snapshot.hardware),
            version: env!("CARGO_PKG_VERSION"),
            build_utc: build_info::BUILT_TIME_UTC,
        };
        self.state.send_replace(Some(snapshot.clone()));
        let _ = self.handle.updates.send(snapshot.clone());
        snapshot
    }
}
impl Owner {
    fn publish_inventory(&self, cause: Option<&RequestContext>) {
        let values = serde_json::from_slice::<mqtt_bridge_kit::PropertyMap>(
            &self.model.state_bytes().expect("preflighted model"),
        )
        .expect("inventory State is an object");
        self.reporter
            .as_ref()
            .expect("attached reporter")
            .state_with_context(
                "__inventory",
                values,
                false,
                &RequestContext::new(self.table.instance.clone())
                    .expect("validated generation")
                    .with_traceparent(cause.and_then(RequestContext::traceparent)),
            );
    }
    fn activate(
        &mut self,
        service: &DaliService,
        candidate: Arc<Model>,
        completion: Option<&mqtt_bridge_kit::CommandCompletion>,
        cause: &RequestContext,
    ) -> Result<Snapshot, ServiceError> {
        let next = routing::generations(&candidate);
        let reporter = self
            .reporter
            .as_ref()
            .ok_or(ServiceError::Rejected("runtime not attached".into()))?;
        #[cfg(test)]
        if let Some(hook) = &service.activation_hook {
            hook(reporter, &self.table);
        }
        let result = match completion {
            Some(completion) => {
                reporter.replace_generations_for_command(&self.table, next.clone(), completion)
            }
            None => reporter.replace_generations(&self.table, next.clone()),
        };
        if let Err(error) = result {
            self.pending = Some(candidate);
            service.update_snapshot(
                self.model.clone(),
                Phase::Uncertain(format!("runtime authority: {error}")),
            );
            return Err(ServiceError::Uncertain(error));
        }
        self.table = next;
        self.model = candidate;
        self.pending = None;
        self.publish_inventory(Some(cause));
        Ok(service.update_snapshot(self.model.clone(), Phase::Ready))
    }
    async fn change(
        &mut self,
        service: &DaliService,
        request: &mut Request,
        context: &mut ServiceContext<DaliProxy>,
    ) -> Result<Snapshot, ServiceError> {
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        routing::admit(
            &service.handle.snapshot().ok_or(ServiceError::Closed)?,
            &request.change,
            &request.context,
        )?;
        if matches!(request.change, Change::Recover) {
            if let Some(candidate) = self.pending.clone() {
                return self.activate(
                    service,
                    candidate,
                    request.completion.as_ref(),
                    &request.context,
                );
            }
        }
        service.update_snapshot(self.model.clone(), Phase::Writing);
        let operation = async {
            match &request.change {
                Change::Edit { expected, edit } => {
                    self.store
                        .edit(*expected, vec![edit.clone().into_edit()], request.deadline)
                        .await
                }
                Change::Recover => self.store.recover(request.deadline).await,
            }
        };
        // WAIT: v2-service-save
        let outcome = tokio::select! {
            biased;
            result=operation=>result,
            _=context.cancelled()=>Outcome::Uncertain("service stopping during save".into()),
            _=request.reply.closed()=>Outcome::Uncertain("caller canceled during save".into()),
        };
        match outcome {
            Outcome::Committed { .. } => {
                let disk = self.store.snapshot();
                if disk.phase != DiskPhase::Ready {
                    return self.freeze(service, "filesystem not ready after commit".into());
                }
                // Recovery may report changed:false for a different mapping.
                // Activate the entire confirmed model, never that flag.
                self.activate(
                    service,
                    disk.model,
                    request.completion.as_ref(),
                    &request.context,
                )
            }
            Outcome::NotCommitted(reason) => {
                let disk = self.store.snapshot();
                if disk.phase == DiskPhase::Ready
                    && disk.model.state_bytes() == self.model.state_bytes()
                {
                    self.model = disk.model;
                    service.update_snapshot(self.model.clone(), Phase::Ready);
                    Err(ServiceError::Rejected(reason))
                } else {
                    self.freeze(service, reason)
                }
            }
            Outcome::Uncertain(reason) => self.freeze(service, reason),
        }
    }
    fn freeze(&self, service: &DaliService, reason: String) -> Result<Snapshot, ServiceError> {
        service.update_snapshot(self.model.clone(), Phase::Uncertain(reason.clone()));
        Err(ServiceError::Uncertain(reason))
    }
}
impl ProcessService for DaliService {
    type Driver = DaliProxy;
    async fn run(mut self, mut context: ServiceContext<DaliProxy>) -> Result<(), String> {
        let _admission = AdmissionGuard(self.handle.clone(), self.state.clone());
        let deadline = Instant::now() + Duration::from_secs(5);
        #[cfg(not(test))]
        let opening = ConfigStore::open(self.paths.clone(), self.instance.clone(), deadline);
        #[cfg(test)]
        let opening = crate::config_store::test_hooks::open(
            self.paths.clone(),
            self.instance.clone(),
            deadline,
            self.hooks.clone(),
        );
        // WAIT: v2-service-start
        let store = tokio::select! {
            result=opening=>result?,
            _=context.cancelled()=>return Ok(()),
        };
        let model = store.snapshot().model;
        if model.inventory().name != self.instance {
            // WAIT: v2-store-close
            store.close(Instant::now() + Duration::from_secs(1)).await;
            return Err("--instance must match the persisted controller name".into());
        }
        let table = routing::generations(&model);
        self.update_snapshot(model.clone(), Phase::AwaitingRuntime);
        if let Err(error) = context.ready(DaliProxy {
            handle: self.handle.clone(),
            initial: table.clone(),
            reporter: None,
        }) {
            // WAIT: v2-store-close
            store.close(Instant::now() + Duration::from_secs(1)).await;
            return Err(error);
        }
        let mut owner = Owner {
            store,
            model,
            table,
            reporter: None,
            pending: None,
        };
        let buses = owner
            .model
            .inventory()
            .buses
            .iter()
            .filter(|bus| bus.enabled)
            .map(|bus| bus.bus)
            .collect();
        let mut hat = hat::Hat::new(self.uart.take(), buses);
        let mut failure = None;
        loop {
            // WAIT: v2-service-loop
            let message = tokio::select! {
                biased;
                _=context.cancelled()=>break,
                message=self.receive.recv()=>match message {Some(message)=>Some(message),None=>break},
                result=hat.changed()=>{
                    if let Err(error)=result { failure=Some(error);break; }
                    let mut snapshot=self.handle.snapshot().expect("service started");
                    snapshot.hardware=hat.hardware();
                    let mut completion=hat.take_completed_output();
                    if let Some(completion)=&mut completion {
                        if let Some(observation)=&mut completion.observation {
                            if owner.table.targets.get(&completion.request.target).map(String::as_str)==observation.context.generation() {
                                snapshot.outputs.insert(completion.request.target.clone(), observation.clone());
                            }
                        }
                    }
                    #[cfg(test)]
                    if completion.is_some() {
                        if let Some(hook)=&self.output_acceptance_hook { hook(&mut hat); }
                    }
                    hat.invalidate_outputs(&mut snapshot.outputs);
                    // Cache and reply share the final validated sample. A second
                    // independent validation could otherwise change only one.
                    if let Some(completion)=&mut completion {
                        if let Some(observation)=&mut completion.observation {
                            if let Some(accepted)=snapshot.outputs.get(&completion.request.target)
                                .filter(|sample| sample.context.generation()==observation.context.generation()) {
                                *observation=accepted.clone();
                            }
                            if observation.quality==output::Quality::Stale && completion.error.is_none() {
                                completion.error=Some(if completion.request.intent.is_some() {ServiceError::Uncertain("output evidence changed before acceptance".into())} else {ServiceError::Rejected("output evidence changed before acceptance".into())});
                            }
                        }
                    }
                    self.state.send_replace(Some(snapshot.clone()));
                    let _=self.handle.updates.send(snapshot);
                    if let Some(completion)=completion {
                        let result=match (completion.error, completion.observation) {
                            (Some(error),_)=>Err(error),
                            (None,Some(observation)) if owner.table.targets.get(&completion.request.target).map(String::as_str)==observation.context.generation()=>Ok(observation),
                            _=>Err(ServiceError::Rejected("output mapping changed before completion".into())),
                        };
                        let output::Request {reply,_permit,..}=completion.request;
                        drop(_permit);
                        let _=reply.send(result);
                    }
                    None
                },
            };
            let Some(message) = message else {
                continue;
            };
            match message {
                Message::Output(request) => {
                    if request.reply.is_closed() || Instant::now() >= request.deadline {
                        let _ = request.reply.send(Err(ServiceError::Expired));
                    } else {
                        let snapshot = self.handle.snapshot().expect("service started");
                        match output::resolve(
                            &snapshot,
                            &request.target,
                            &request.context,
                            request.intent.is_some(),
                        ) {
                            Ok(route) => hat.enqueue_output(request, route),
                            Err(error) => {
                                let _ = request.reply.send(Err(error));
                            }
                        }
                    }
                }
                Message::Attach(reporter, reply) => {
                    if owner.reporter.is_none() {
                        owner.reporter = Some(reporter);
                        owner.publish_inventory(None);
                        self.update_snapshot(owner.model.clone(), Phase::Ready);
                    }
                    let _ = reply.send(Ok(()));
                }
                Message::Change(mut request) => {
                    let span = request
                        .context
                        .operation_span(&owner.model.inventory().name, "inventory_change");
                    let result = owner
                        .change(&self, &mut request, &mut context)
                        .instrument(span)
                        .await;
                    let _ = request.reply.send(result);
                }
            }
        }
        self.handle.permit.close();
        self.receive.close();
        self.update_snapshot(owner.model.clone(), Phase::Stopping);
        while let Ok(message) = self.receive.try_recv() {
            match message {
                Message::Output(request) => {
                    let _ = request.reply.send(Err(ServiceError::Closed));
                }
                Message::Attach(_, reply) => {
                    let _ = reply.send(Err("service stopping".into()));
                }
                Message::Change(request) => {
                    let _ = request.reply.send(Err(ServiceError::Closed));
                }
            }
        }
        // Hardware and disk share one local absolute cleanup budget. The parent
        // managed runtime's cancellation/actual destruction remains authoritative.
        let deadline = Instant::now() + Duration::from_millis(1500);
        // WAIT: v2-hat-close
        let hardware = hat.close(deadline).await;
        let mut snapshot = self.handle.snapshot().expect("service started");
        snapshot.hardware = hat.hardware();
        hat.invalidate_outputs(&mut snapshot.outputs);
        self.state.send_replace(Some(snapshot.clone()));
        let _ = self.handle.updates.send(snapshot);
        // WAIT: v2-store-close
        if !owner.store.close(deadline).await {
            return Err("configuration owner did not finish shutdown".into());
        }
        hardware?;
        if let Some(error) = failure {
            return Err(error);
        }
        self.update_snapshot(owner.model, Phase::Closed);
        Ok(())
    }
}
