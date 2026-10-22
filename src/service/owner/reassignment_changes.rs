use super::*;

impl Owner {
    pub(super) async fn reset_space(
        &mut self,
        service: &DaliService,
        request: &mut reassignment::Request,
        context: &mut ServiceContext<DaliProxy>,
        hat: &hat::Hat,
    ) -> Result<reassignment::Route, ServiceError> {
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        let snapshot = service.handle.snapshot().ok_or(ServiceError::Closed)?;
        let selected = commissioning::resolve_space(
            &snapshot,
            request.bus,
            request.space,
            request.expected,
            &request.context,
        )?;
        if identification::blocks_bus(&service.handle, request.bus) {
            return Err(ServiceError::Rejected(
                "stop identification before reassignment".into(),
            ));
        }
        let stamp = hat.reassignment_stamp(request.bus, request.space)?;
        let saved = snapshot
            .model
            .inventory()
            .buses
            .iter()
            .find(|b| b.bus == request.bus)
            .unwrap();
        let (edits, retired_devices, retired_instances) = match request.space {
            discovery::Space::Outputs => (
                saved
                    .outputs
                    .iter()
                    .map(|o| Edit::RemoveOutput {
                        bus: request.bus,
                        address: o.short_address,
                    })
                    .collect(),
                saved.outputs.len(),
                0,
            ),
            discovery::Space::Inputs => (
                saved
                    .inputs
                    .iter()
                    .map(|i| Edit::RemoveInput {
                        bus: request.bus,
                        address: i.short_address,
                    })
                    .collect::<Vec<_>>(),
                saved.inputs.len(),
                saved.inputs.iter().map(|i| i.instances.len()).sum(),
            ),
        };
        service.update_snapshot(self.model.clone(), Phase::Writing);
        // WAIT: v2-service-save
        let outcome = tokio::select! {biased;
            outcome=self.store.edit(request.expected,edits,request.deadline)=>outcome,
            _=context.cancelled()=>Outcome::Uncertain("service stopping during reassignment save".into()),
            _=request.reply.closed()=>Outcome::Uncertain("caller cancelled during reassignment save".into()),
        };
        let accepted = self.accept_save(
            service,
            outcome,
            request.completion.as_ref(),
            &request.context,
        )?;
        request.committed = Some(reassignment::Report {
            bus: request.bus,
            space: request.space,
            outcome: reassignment::ResetOutcome::RetiredButUnconfirmed,
            revision: accepted.model.inventory().revision,
            retired_devices,
            retired_instances,
            checked_addresses: 0,
            reason: None,
        });
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        if !hat.evidence_current(request.bus, stamp) {
            return Err(ServiceError::Rejected(
                "physical evidence changed during reassignment save".into(),
            ));
        }
        Ok(reassignment::Route {
            bus: request.bus,
            space: request.space,
            controller: selected.controller,
            context: request.context.clone(),
            original_stamp: stamp,
        })
    }
}

pub(super) fn accept(
    service: &DaliService,
    completed: &mut reassignment::Completion,
    hat: &hat::Hat,
) -> Result<reassignment::Report, ServiceError> {
    let request = &mut completed.request;
    let mut report = request
        .committed
        .clone()
        .expect("reset work follows committed retirement");
    if let Some(sample) = &completed.observation {
        report.checked_addresses = sample.checked;
    }
    let snapshot = service.handle.snapshot().ok_or(ServiceError::Closed)?;
    let verified = !request.reply.is_closed()
        && Instant::now() < request.deadline
        && completed.error.is_none()
        && snapshot.phase == Phase::Ready
        && snapshot.model.inventory().revision == report.revision
        && snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|b| b.bus == request.bus && b.enabled)
        && completed.observation.as_ref().is_some_and(|o| {
            !o.stale && o.checked == 64 && hat.evidence_current(request.bus, o.stamp)
        });
    if verified {
        report.outcome = reassignment::ResetOutcome::Cleared;
    } else {
        report.reason = Some(
            completed
                .error
                .take()
                .map(|e| e.to_string())
                .unwrap_or_else(|| {
                    "address clear evidence incomplete or changed; inspect inventory and refresh"
                        .into()
                }),
        );
    }
    Ok(report)
}
