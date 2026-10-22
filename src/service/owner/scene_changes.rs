use super::*;
use scene_settings::{Action, Advance, Plan};

impl Owner {
    pub(super) async fn scene_setting(
        &mut self,
        service: &DaliService,
        completion: &mut scene_settings::Completion,
        context: &mut ServiceContext<DaliProxy>,
        hat: &hat::Hat,
    ) -> Result<Advance, ServiceError> {
        let q = &mut completion.request;
        if q.reply.is_closed() || Instant::now() >= q.deadline {
            return Err(ServiceError::Expired);
        }
        let current = scene_settings::resolve(
            &service.handle.snapshot().ok_or(ServiceError::Closed)?,
            &q.target,
            &q.action,
            &q.context,
            q.committed.is_some(),
        )?;
        let mut route = q.selection.clone();
        if current.origin != route.origin
            || current.bus != route.bus
            || current.address != route.address
            || current.slot != route.slot
            || current.fixture_generation != route.fixture_generation
        {
            return Err(ServiceError::Rejected(
                "scene fixture mapping changed".into(),
            ));
        }
        let mut observed = completion.observation.take().ok_or_else(|| {
            completion.error.take().unwrap_or(ServiceError::Rejected(
                "missing scene settings result".into(),
            ))
        })?;
        observed.stale |= !hat.evidence_current(route.bus, observed.stamp);
        if let Some(stored) = &mut observed.stored {
            stored.stale |= observed.stale;
            if stored.stale {
                stored.quality = scenes::Quality::Stale;
            }
        }
        if observed.programmed {
            let saved = q.committed.as_ref().expect("program continuation");
            let matching = scenes::settings_value::compare(saved.saved, observed.stored.as_ref())
                == scenes::settings_value::Comparison::Matching;
            let verified = matching && !observed.stale && completion.error.is_none();
            if !verified {
                observed.reason=Some(completion.error.take().map(|e|e.to_string())
                .unwrap_or_else(||"stored scene verification incomplete, changed or mismatching; inspect hardware".into()));
            }
            return Ok(Advance::Done(scene_settings::report(
                &self.model,
                q,
                &route,
                if verified { "verified" } else { "unconfirmed" },
                saved.configuration_changed,
                observed,
            )));
        }
        if let Some(error) = completion.error.take() {
            return Err(error);
        }
        if observed.stale {
            return Err(ServiceError::Rejected(
                "scene evidence changed before acceptance".into(),
            ));
        }
        if matches!(q.action, Action::Capture { .. }) {
            return Ok(Advance::Done(scene_settings::report(
                &self.model,
                q,
                &route,
                "observed",
                false,
                observed,
            )));
        }
        let desired = match q.action {
            Action::Set { .. } | Action::Adopt { .. } => observed.desired.ok_or_else(|| {
                ServiceError::Rejected("scene setting did not pass preflight".into())
            })?,
            Action::Repair { .. } => scenes::settings_value::saved(
                &self.model,
                route.bus,
                route.slot.raw(),
                route.address.raw(),
            )
            .ok_or_else(|| ServiceError::Rejected("scene fixture row is unmanaged".into()))?,
            Action::Capture { .. } => unreachable!(),
        };
        let before = self.model.inventory().revision;
        if !matches!(q.action, Action::Repair { .. }) {
            service.update_snapshot(self.model.clone(), Phase::Writing);
            // WAIT: v2-service-save
            let outcome = tokio::select! {
                biased;
                result=self.store.edit(q.action.expected().unwrap(),vec![Edit::SceneRow {
                    bus:route.bus, slot:route.slot.raw(), output_origin:route.origin.clone(), setting:desired
                }],q.deadline)=>result,
                _=context.cancelled()=>Outcome::Uncertain("service stopping during scene save".into()),
                _=q.reply.closed()=>Outcome::Uncertain("caller canceled during scene save".into()),
            };
            self.accept_save(service, outcome, q.completion.as_ref(), &q.context)?;
        }
        let changed = self.model.inventory().revision != before;
        if matches!(q.action, Action::Adopt { .. }) {
            if let Some(stored) = &mut observed.stored {
                stored.stale |= !hat.evidence_current(route.bus, observed.stamp);
                if stored.stale {
                    stored.quality = scenes::Quality::Stale;
                }
            }
            return Ok(Advance::Done(scene_settings::report(
                &self.model,
                q,
                &route,
                "adopted",
                changed,
                observed,
            )));
        }
        let stamp = observed.stamp;
        observed.stored = None;
        observed.reason = Some("desired scene saved; native programming is unconfirmed".into());
        q.committed = Some(scene_settings::report(
            &self.model,
            q,
            &route,
            "unconfirmed",
            changed,
            observed,
        ));
        if q.reply.is_closed() || Instant::now() >= q.deadline {
            return Err(ServiceError::Expired);
        }
        route.plan = Plan::Program {
            desired,
            original_stamp: stamp,
        };
        Ok(Advance::Program(route))
    }
}
