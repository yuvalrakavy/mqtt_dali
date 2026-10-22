use super::*;

/// Cut current values/actions without changing the continuing saved identities.
pub(super) fn cut(service: &DaliService, hat: &mut hat::Hat, route: &restoration::Route) {
    let Some(mut snapshot) = service.handle.snapshot() else {
        return;
    };
    if route.space == discovery::Space::Inputs {
        let mut addresses = vec![route.address];
        if let Some(candidate) = &route.candidate {
            addresses.push(candidate.address);
        }
        hat.cut_restoration_inputs(route.bus, &addresses);
        snapshot.inputs = hat.input_observations();
        snapshot.input_listeners = hat.input_listeners();
    } else {
        for target in std::iter::once(route.target.as_str())
            .chain(route.candidate.as_ref().map(|c| c.target.as_str()))
        {
            if let Some(observation) = snapshot.outputs.get_mut(target) {
                observation.invalidate("address restoration changed the receive boundary");
            }
        }
    }
    service.state.send_replace(Some(snapshot.clone()));
    let _ = service.handle.updates.send(snapshot);
}

impl Owner {
    pub(super) async fn restore(
        &mut self,
        service: &DaliService,
        completed: &mut restoration::Completion,
        context: &mut ServiceContext<DaliProxy>,
        hat: &mut hat::Hat,
    ) -> Result<restoration::Advance, ServiceError> {
        use restoration::{Advance, Plan, ResultKind, Step};
        let q = &mut completed.request;
        let route = q.route.clone().expect("admitted restoration route");
        cut(service, hat, &route);
        if q.reply.is_closed() || Instant::now() >= q.deadline {
            return Err(ServiceError::Expired);
        }
        let observed = completed.observation.take().ok_or_else(|| {
            completed.error.take().unwrap_or_else(|| {
                ServiceError::Uncertain("missing address restoration evidence".into())
            })
        })?;
        if observed.stale
            || Some(observed.stamp) != route.original_stamp
            || !hat.evidence_current(route.bus, observed.stamp)
        {
            return Err(ServiceError::Uncertain(
                "address restoration physical evidence changed".into(),
            ));
        }
        if let Some(error) = completed.error.take() {
            return Err(error);
        }
        let snapshot = service.handle.snapshot().ok_or(ServiceError::Closed)?;
        let report = q.report.as_ref().expect("admitted restoration report");
        if !restoration::current(&snapshot, &route, report.revision) {
            return Err(ServiceError::Rejected(
                "saved fixture or restoration root changed".into(),
            ));
        }
        match observed.step {
            Some(Step::Cleared) if matches!(route.plan, Plan::Clear) => {
                let mut report = q.report.clone().unwrap();
                report.outcome = ResultKind::Cleared;
                report.reason = None;
                Ok(Advance::Done(report))
            }
            Some(Step::ReadyToRebind) if !q.committed => {
                let fresh = restoration::resolve(&snapshot, &q.target, &q.action, &q.context)?;
                let candidate = route.candidate.as_ref().expect("identified candidate");
                if fresh.candidate.as_ref().map(|c| &c.origin) != Some(&candidate.origin) {
                    return Err(ServiceError::Rejected(
                        "identified candidate origin changed".into(),
                    ));
                }
                let edits = match route.space {
                    discovery::Space::Outputs => vec![
                        Edit::RemoveOutput {
                            bus: route.bus,
                            address: candidate.address,
                        },
                        Edit::ReaddressOutputs {
                            bus: route.bus,
                            addresses: vec![(route.address, candidate.address)],
                        },
                    ],
                    discovery::Space::Inputs => vec![
                        Edit::RemoveInput {
                            bus: route.bus,
                            address: candidate.address,
                        },
                        Edit::ReaddressInput {
                            bus: route.bus,
                            from: route.address,
                            to: candidate.address,
                        },
                    ],
                };
                service.update_snapshot(self.model.clone(), Phase::Writing);
                // WAIT: v2-service-save
                let outcome = tokio::select! {biased;
                    outcome=self.store.edit(q.action.expected(),edits,q.deadline)=>outcome,
                    _=context.cancelled()=>Outcome::Uncertain("service stopping during candidate retirement".into()),
                    _=q.reply.closed()=>Outcome::Uncertain("caller cancelled during candidate retirement".into()),
                };
                let accepted =
                    self.accept_save(service, outcome, q.completion.as_ref(), &q.context)?;
                q.committed = true;
                let report = q.report.as_mut().unwrap();
                report.revision = accepted.model.inventory().revision;
                report.configuration_changed = true;
                report.address = candidate.address;
                report.current_generation =
                    routing::generations(&accepted.model).targets[&route.target].clone();
                report.outcome = ResultKind::ConfigurationUpdatedButUnconfirmed;
                cut(service, hat, &route);
                if q.reply.is_closed() || Instant::now() >= q.deadline {
                    return Err(ServiceError::Expired);
                }
                let location = match route.space {
                    discovery::Space::Outputs => crate::inventory::Location::Output {
                        bus: route.bus,
                        short_address: candidate.address,
                    },
                    discovery::Space::Inputs => crate::inventory::Location::Input {
                        bus: route.bus,
                        short_address: candidate.address,
                    },
                };
                if accepted.phase != Phase::Ready
                    || accepted.model.origin(location) != Some(&route.origin)
                {
                    return Err(ServiceError::Uncertain(
                        "saved address mapping changed before acceptance".into(),
                    ));
                }
                if !hat.evidence_current(route.bus, observed.stamp) {
                    return Err(ServiceError::Uncertain(
                        "mapping updated; current fixture evidence changed".into(),
                    ));
                }
                report.outcome = ResultKind::Restored;
                Ok(Advance::Done(report.clone()))
            }
            Some(Step::Restored) => {
                let mut report = q.report.clone().unwrap();
                report.outcome = ResultKind::Restored;
                report.reason = None;
                Ok(Advance::Done(report))
            }
            Some(Step::NoCandidate | Step::Multiple) if matches!(route.plan, Plan::Unique) => {
                let mut report = q.report.clone().unwrap();
                report.outcome = if matches!(observed.step, Some(Step::NoCandidate)) {
                    ResultKind::NoUnaddressedFixture
                } else {
                    ResultKind::MultipleUnaddressedFixtures
                };
                Ok(Advance::Done(report))
            }
            _ => Err(ServiceError::Uncertain(
                "address restoration did not complete its native plan".into(),
            )),
        }
    }
}
