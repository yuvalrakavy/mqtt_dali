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
            input_listeners: self
                .handle
                .snapshot()
                .map(|s| input::retained_listeners(&model, &s))
                .unwrap_or_default(),
            inputs: input::retained(
                &model,
                self.handle.snapshot().map(|s| s.inputs).unwrap_or_default(),
            ),
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
        self.accept_save(
            service,
            outcome,
            request.completion.as_ref(),
            &request.context,
        )
    }
    fn accept_save(
        &mut self,
        service: &DaliService,
        outcome: Outcome,
        completion: Option<&mqtt_bridge_kit::CommandCompletion>,
        cause: &RequestContext,
    ) -> Result<Snapshot, ServiceError> {
        match outcome {
            Outcome::Committed { .. } => {
                let disk = self.store.snapshot();
                if disk.phase != DiskPhase::Ready {
                    return self.freeze(service, "filesystem not ready after commit".into());
                }
                // Recovery may report changed:false for a different mapping.
                // Activate the entire confirmed model, never that flag.
                self.activate(service, disk.model, completion, cause)
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
    async fn discover(
        &mut self,
        service: &DaliService,
        completion: &mut discovery::Completion,
        context: &mut ServiceContext<DaliProxy>,
        hat: &hat::Hat,
    ) -> Result<discovery::Report, ServiceError> {
        if let Some(error) = completion.error.take() {
            return Err(error);
        }
        let request = &mut completion.request;
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        discovery::resolve(
            &service.handle.snapshot().ok_or(ServiceError::Closed)?,
            request.bus,
            request.space,
            request.address,
            request.expected,
            &request.context,
        )?;
        let mut probe = completion
            .observation
            .take()
            .ok_or_else(|| ServiceError::Rejected("missing discovery result".into()))?;
        hat.invalidate_discovery(&mut probe);
        if probe.stale {
            return Err(ServiceError::Rejected(
                "discovery evidence changed before save".into(),
            ));
        }
        let edits = discovery::additions(&self.model, &mut probe)?;
        let added = edits
            .iter()
            .map(|edit| match edit {
                Edit::AddInput { input, .. } => 1 + input.instances.len(),
                _ => 1,
            })
            .sum();
        if !edits.is_empty() {
            service.update_snapshot(self.model.clone(), Phase::Writing);
            // WAIT: v2-service-save
            let outcome = tokio::select! {
                biased;
                result=self.store.edit(request.expected,edits,request.deadline)=>result,
                _=context.cancelled()=>Outcome::Uncertain("service stopping during discovery save".into()),
                _=request.reply.closed()=>Outcome::Uncertain("caller canceled during discovery save".into()),
            };
            self.accept_save(
                service,
                outcome,
                request.completion.as_ref(),
                &request.context,
            )?;
        }
        Ok(discovery::report(&self.model, probe, added))
    }
    async fn commission(
        &mut self,
        service: &DaliService,
        completion: &mut commissioning::Completion,
        context: &mut ServiceContext<DaliProxy>,
        hat: &hat::Hat,
    ) -> Result<commissioning::Advance, ServiceError> {
        use commissioning::{Advance, AssignmentOutcome, Step};
        if let Some(error) = completion.error.take() {
            return Err(error);
        }
        let request = &mut completion.request;
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        let mut observation = completion
            .observation
            .take()
            .ok_or_else(|| ServiceError::Rejected("missing commissioning result".into()))?;
        if observation.stale || !hat.evidence_current(observation.bus, observation.stamp) {
            return Err(ServiceError::Rejected(
                "commissioning physical evidence changed".into(),
            ));
        }
        if let Some(reservation) = &request.reservation {
            if service
                .handle
                .snapshot()
                .is_none_or(|s| s.phase != Phase::Ready)
                || self.model.inventory().revision != reservation.revision
            {
                return Err(ServiceError::Rejected(
                    "reserved mapping changed before assignment completion".into(),
                ));
            }
            return match observation.step {
                Some(Step::Assigned) => Ok(Advance::Done(commissioning::report(
                    request,
                    AssignmentOutcome::Assigned,
                    reservation.revision,
                    None,
                ))),
                _ => Err(ServiceError::Rejected(
                    "programming did not yield verified assignment".into(),
                )),
            };
        }
        commissioning::resolve(
            &service.handle.snapshot().ok_or(ServiceError::Closed)?,
            request.bus,
            request.expected,
            &request.context,
        )?;
        let (address, random) = match observation.step.take() {
            Some(Step::NoCapacity) => {
                return Ok(Advance::Done(commissioning::report(
                    request,
                    AssignmentOutcome::NoFreeAddress,
                    self.model.inventory().revision,
                    None,
                )))
            }
            Some(Step::NoCandidate) => {
                return Ok(Advance::Done(commissioning::report(
                    request,
                    AssignmentOutcome::NoUnaddressedOutput,
                    self.model.inventory().revision,
                    None,
                )))
            }
            Some(Step::Found { address, random }) => (address, random),
            _ => {
                return Err(ServiceError::Rejected(
                    "search did not yield a candidate".into(),
                ))
            }
        };
        let saved = self
            .model
            .inventory()
            .buses
            .iter()
            .find(|b| b.bus == request.bus)
            .ok_or(ServiceError::Closed)?;
        if saved.outputs.iter().any(|o| o.short_address == address) {
            return Err(ServiceError::Rejected(
                "destination is already reserved".into(),
            ));
        }
        let name = discovery::unused_name("Light", saved.outputs.iter().map(|o| o.name.as_str()));
        let edit = Edit::AddOutput {
            bus: request.bus,
            output: crate::config::Output {
                short_address: address,
                name,
                description: None,
                color_representation: None,
            },
        };
        service.update_snapshot(self.model.clone(), Phase::Writing);
        // WAIT: v2-service-save
        let outcome = tokio::select! {
            biased;
            result=self.store.edit(request.expected,vec![edit],request.deadline)=>result,
            _=context.cancelled()=>Outcome::Uncertain("service stopping during address reservation".into()),
            _=request.reply.closed()=>Outcome::Uncertain("caller canceled during address reservation".into()),
        };
        self.accept_save(
            service,
            outcome,
            request.completion.as_ref(),
            &request.context,
        )?;
        let target = self
            .model
            .projections()
            .into_iter()
            .find(|p| {
                p.location
                    == crate::inventory::Location::Output {
                        bus: request.bus,
                        short_address: address,
                    }
            })
            .expect("confirmed reserved output")
            .target;
        request.reservation = Some(commissioning::Reservation {
            address,
            target: target.clone(),
            revision: self.model.inventory().revision,
        });
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        let internal = RequestContext::new(self.table.targets[&target].clone())
            .expect("confirmed generation")
            .with_traceparent(request.context.traceparent());
        Ok(Advance::Program(commissioning::Route {
            bus: request.bus,
            controller: self.model.inventory().name.clone(),
            context: internal,
            plan: commissioning::Plan::Program(commissioning::Candidate {
                address,
                random,
                stamp: observation.stamp,
            }),
        }))
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
            read_cursor: 0,
            #[cfg(test)]
            read_pause: self.proxy_read_pause.clone(),
            #[cfg(test)]
            snapshot_pause: self.proxy_snapshot_pause.clone(),
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
            // Sync before polling any public frame after durable activation.
            hat.sync_inputs(&owner.model);
            // WAIT: v2-service-loop
            let message = tokio::select! {
                biased;
                _=context.cancelled()=>break,
                message=self.receive.recv()=>match message {Some(message)=>Some(message),None=>break},
                result=hat.changed()=>{
                    if let Err(error)=result { failure=Some(error);break; }
                    let mut snapshot=self.handle.snapshot().expect("service started");
                    snapshot.hardware=hat.hardware();
                    snapshot.input_listeners=hat.input_listeners();
                    let mut completion=hat.take_completed_physical();
                    if matches!(completion,Some(physical::Completion::Commission(_))) {
                        let Some(physical::Completion::Commission(mut commissioned))=completion.take() else {unreachable!()};
                        match owner.commission(&self,&mut commissioned,&mut context,&hat).await {
                            Ok(commissioning::Advance::Program(route))=>hat.enqueue_commission(commissioned.request,route),
                            Ok(commissioning::Advance::Done(report))=>commissioning::finish(commissioned.request,Ok(report)),
                            Err(error)=>commissioning::finish(commissioned.request,Err(error)),
                        }
                        snapshot=self.handle.snapshot().expect("service started");hat.sync_inputs(&owner.model);
                    }
                    if matches!(completion,Some(physical::Completion::Discovery(_))) {
                        let Some(physical::Completion::Discovery(mut discovered))=completion.take() else {unreachable!()};
                        let result=owner.discover(&self,&mut discovered,&mut context,&hat).await;
                        let discovery::Request{reply,_permit,..}=discovered.request;
                        drop(_permit);
                        let _=reply.send(result);
                        snapshot=self.handle.snapshot().expect("service started");
                        hat.sync_inputs(&owner.model);
                    }
                    snapshot.hardware=hat.hardware();
                    snapshot.input_listeners=hat.input_listeners();
                    if let Some(physical::Completion::Output(completion))=&mut completion {
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
                    if let Some(physical::Completion::Output(completion))=&mut completion {
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
                    snapshot.inputs=hat.input_observations();
                    if let Some(physical::Completion::Input(completion))=&mut completion {
                        if let Some(observation)=&mut completion.observation {
                            if let Some(accepted)=snapshot.inputs.get(&completion.request.target)
                                .filter(|sample|sample.context.generation()==observation.context.generation()) {
                                *observation=accepted.clone();
                            }
                            if observation.stale && completion.error.is_none() {
                                completion.error=Some(ServiceError::Rejected("input evidence changed before acceptance".into()));
                            }
                        }
                    }
                    let input_actions_allowed=snapshot.phase==Phase::Ready;
                    self.state.send_replace(Some(snapshot.clone()));
                    let _=self.handle.updates.send(snapshot);
                    let (notices,dropped)=hat.take_input_notices();
                    if dropped>0 {tracing::warn!(kind="input_event_loss",dropped,"Input notice batch overflowed");}
                    if input_actions_allowed {for notice in notices {let _=self.handle.input_events.send(notice);}}
                    if let Some(completion)=completion { match completion {
                      physical::Completion::Commission(_)=>unreachable!("commissioning advanced before cache publication"),
                      physical::Completion::Discovery(_)=>unreachable!("discovery accepted before cache publication"),
                      physical::Completion::Output(completion)=>{
                        let result=match (completion.error, completion.observation) {
                            (Some(error),_)=>Err(error),
                            (None,Some(observation)) if owner.table.targets.get(&completion.request.target).map(String::as_str)==observation.context.generation()=>Ok(observation),
                            _=>Err(ServiceError::Rejected("output mapping changed before completion".into())),
                        };
                        let output::Request {reply,_permit,..}=completion.request;
                        drop(_permit);
                        let _=reply.send(result);
                      },
                      physical::Completion::Input(completion)=>{
                        let result=match (completion.error,completion.observation) {
                            (Some(error),_)=>Err(error),
                            (None,Some(observation)) if owner.table.targets.get(&completion.request.target).map(String::as_str)==observation.context.generation()=>Ok(observation),
                            _=>Err(ServiceError::Rejected("input mapping changed before completion".into())),
                        };
                        let input::readback::Request{reply,_permit,..}=completion.request;
                        drop(_permit);
                        let _=reply.send(result);
                      },
                    } }
                    None
                },
            };
            let Some(message) = message else {
                continue;
            };
            match message {
                Message::Commission(request) => {
                    if request.reply.is_closed() || Instant::now() >= request.deadline {
                        commissioning::finish(request, Err(ServiceError::Expired));
                    } else {
                        let snapshot = self.handle.snapshot().expect("service started");
                        match commissioning::resolve(
                            &snapshot,
                            request.bus,
                            request.expected,
                            &request.context,
                        ) {
                            Ok(route) => hat.enqueue_commission(request, route),
                            Err(error) => commissioning::finish(request, Err(error)),
                        }
                    }
                }
                Message::Discovery(request) => {
                    if request.reply.is_closed() || Instant::now() >= request.deadline {
                        let _ = request.reply.send(Err(ServiceError::Expired));
                    } else {
                        let snapshot = self.handle.snapshot().expect("service started");
                        match discovery::resolve(
                            &snapshot,
                            request.bus,
                            request.space,
                            request.address,
                            request.expected,
                            &request.context,
                        ) {
                            Ok(route) => hat.enqueue_discovery(request, route),
                            Err(error) => {
                                let _ = request.reply.send(Err(error));
                            }
                        }
                    }
                }
                Message::Input(request) => {
                    if request.reply.is_closed() || Instant::now() >= request.deadline {
                        let _ = request.reply.send(Err(ServiceError::Expired));
                    } else {
                        let snapshot = self.handle.snapshot().expect("service started");
                        match input::readback::resolve(&snapshot, &request.target, &request.context)
                        {
                            Ok(route) => hat.enqueue_input(request, route),
                            Err(error) => {
                                let _ = request.reply.send(Err(error));
                            }
                        }
                    }
                }
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
                Message::Commission(request) => {
                    commissioning::finish(request, Err(ServiceError::Closed))
                }
                Message::Discovery(request) => {
                    let _ = request.reply.send(Err(ServiceError::Closed));
                }
                Message::Input(request) => {
                    let _ = request.reply.send(Err(ServiceError::Closed));
                }
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
        snapshot.inputs = hat.input_observations();
        snapshot.input_listeners = hat.input_listeners();
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
