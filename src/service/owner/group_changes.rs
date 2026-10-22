use super::*;

impl Owner {
    pub(super) async fn groups(
        &mut self,
        service: &DaliService,
        completion: &mut groups::Completion,
        context: &mut ServiceContext<DaliProxy>,
        hat: &hat::Hat,
    ) -> Result<groups::Advance, ServiceError> {
        let request = &mut completion.request;
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        let snapshot = service.handle.snapshot().ok_or(ServiceError::Closed)?;
        let expected = if request.committed.is_some() {
            Some(self.model.inventory().revision)
        } else {
            request.expected
        };
        let mut route = groups::resolve(
            &snapshot,
            &request.target,
            request.action,
            expected,
            &request.context,
        )?;
        let mut observation = completion.observation.take().ok_or_else(|| {
            completion
                .error
                .take()
                .unwrap_or(ServiceError::Rejected("missing group result".into()))
        })?;
        let current = hat.evidence_current(observation.bus, observation.stamp);
        if let Some(probe) = &mut observation.probe {
            probe.stale |= !current;
        }
        if observation.programmed {
            let saved = request.committed.as_ref().expect("program continuation");
            let verified = completion.error.is_none()
                && observation.probe.as_ref().is_some_and(|p| {
                    !p.stale
                        && p.quality == groups::Quality::Complete
                        && p.mask.is_some_and(|mask| match request.action {
                            groups::Action::Assign { group, member } => {
                                crate::dali::groups::Mask(mask).contains(group) == member
                            }
                            groups::Action::Repair { .. } => mask == saved.expected_mask,
                            _ => false,
                        })
                });
            let reason = if verified {
                None
            } else {
                Some(completion.error.take().map(|e|e.to_string()).unwrap_or_else(||"group verification incomplete, changed or mismatching; inspect hardware".into()))
            };
            return Ok(groups::Advance::Done(groups::report(
                &self.model,
                request,
                &route,
                if verified { "verified" } else { "unconfirmed" },
                saved.configuration_changed,
                observation.probe,
                reason,
            )));
        }
        if let Some(error) = completion.error.take() {
            return Err(error);
        }
        if request.action == groups::Action::Read {
            return Ok(groups::Advance::Done(groups::report(
                &self.model,
                request,
                &route,
                "observed",
                false,
                observation.probe,
                None,
            )));
        }
        let mask = observation
            .probe
            .as_ref()
            .filter(|p| !p.stale && p.quality == groups::Quality::Complete)
            .and_then(|p| p.mask)
            .ok_or_else(|| {
                ServiceError::Rejected("complete current group evidence required".into())
            })?;
        match request.action {
            groups::Action::Adopt { reviewed_mask } | groups::Action::Repair { reviewed_mask }
                if reviewed_mask != mask =>
            {
                return Err(ServiceError::Rejected(
                    "hardware group mask changed; review again".into(),
                ))
            }
            _ => {}
        }
        let before = self.model.inventory().revision;
        let desired = match request.action {
            groups::Action::Assign { group, member } => {
                crate::dali::groups::Mask(groups::expected_mask(
                    &self.model,
                    route.bus,
                    route.address.raw(),
                ))
                .with(group, member)
                .0
            }
            groups::Action::Adopt { .. } => mask,
            groups::Action::Repair { .. } => {
                groups::expected_mask(&self.model, route.bus, route.address.raw())
            }
            groups::Action::Read => unreachable!(),
        };
        if matches!(
            request.action,
            groups::Action::Assign { .. } | groups::Action::Adopt { .. }
        ) {
            let edits = groups::edits(&self.model, &route, desired);
            if !edits.is_empty() {
                service.update_snapshot(self.model.clone(), Phase::Writing);
                // WAIT: v2-service-save
                let outcome = tokio::select! {
                    biased;
                    result=self.store.edit(request.expected.expect("mutating revision"),edits,request.deadline)=>result,
                    _=context.cancelled()=>Outcome::Uncertain("service stopping during group save".into()),
                    _=request.reply.closed()=>Outcome::Uncertain("caller canceled during group save".into()),
                };
                self.accept_save(
                    service,
                    outcome,
                    request.completion.as_ref(),
                    &request.context,
                )?;
            }
        }
        let changed = self.model.inventory().revision != before;
        if matches!(request.action, groups::Action::Adopt { .. }) {
            if let Some(probe) = &mut observation.probe {
                probe.stale |= !hat.evidence_current(route.bus, observation.stamp);
            }
            return Ok(groups::Advance::Done(groups::report(
                &self.model,
                request,
                &route,
                "adopted",
                changed,
                observation.probe,
                None,
            )));
        }
        request.committed = Some(groups::report(
            &self.model,
            request,
            &route,
            "unconfirmed",
            changed,
            None,
            None,
        ));
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            return Err(ServiceError::Expired);
        }
        route.plan = groups::Plan::Program {
            desired,
            original_stamp: observation.stamp,
        };
        Ok(groups::Advance::Program(route))
    }
}
