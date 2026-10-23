use super::*;
use mqtt_bridge_kit::*;
use std::time::Duration;

mod colour;
mod feedback;
mod group;
mod group_color;
mod state;

#[derive(Clone, Copy)]
enum ReadKind {
    Output,
    Input,
    Group,
    Feedback,
}

pub struct DaliProxy {
    pub(super) handle: ServiceHandle,
    pub(super) initial: TargetGenerations,
    pub(super) reporter: Option<Reporter>,
    pub(super) read_cursor: usize,
    #[cfg(test)]
    pub(super) read_pause: Option<std::sync::Arc<super::tests::Pause>>,
    #[cfg(test)]
    pub(super) snapshot_pause: Option<std::sync::Arc<super::tests::Pause>>,
}
impl DeviceDriver for DaliProxy {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            commands: [
                "blink",
                "stop_blink",
                "create_group",
                "create_scene",
                "recall_scene",
                "set_scene_row",
                "adopt_scene_row",
                "repair_scene_row",
                "assign_group",
                "configure_input",
                "configure_output",
                "set_input_feedback",
                "set_group_output",
                "set_group_color",
                "adopt_groups",
                "repair_groups",
                "restore_unaddressed_address",
                "restore_identified_address",
                "clear_short_address",
                "reset_address_space",
                "assign_next_output",
                "assign_next_input",
                "configure_bus",
                "discover_address",
                "rename",
                "rename_bus",
                "remove_from_list",
                "describe",
                "recover",
                "set_color_representation",
            ]
            .into_iter()
            .map(|name| CommandSpec::new(name, CommandClass::WithReply))
            .chain(std::iter::once(CommandSpec::new(
                "read_output_settings",
                CommandClass::Read,
            )))
            .chain(
                [
                    "read_memory",
                    "read_identity",
                    "read_luminaire_data",
                    "read_energy_data",
                    "read_diagnostics",
                ]
                .into_iter()
                .map(|name| CommandSpec::new(name, CommandClass::Read)),
            )
            .chain(std::iter::once(CommandSpec::new(
                "read_group_color",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_group_output",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_input_feedback",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_input_settings",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_groups",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_scene",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "capture_scene_row",
                CommandClass::Read,
            )))
            .chain(std::iter::once(CommandSpec::new(
                "read_scene_settings",
                CommandClass::Read,
            )))
            .collect(),
            timing: Timing {
                operation: Duration::from_secs(20),
                poll: Some(Duration::from_secs(30)),
                ..Timing::default()
            },
            ..DriverInfo::default()
        }
    }
    fn request_policy(&self) -> RequestPolicy {
        RequestPolicy::Fenced(self.initial.clone())
    }
    fn validate_request(
        &self,
        target: Option<&str>,
        context: &RequestContext,
        kind: RequestKind<'_>,
    ) -> Result<(), OpError> {
        if target == Some("__inventory") {
            return Err(OpError::Unsupported("inventory is read-only".into()));
        }
        match kind {
            RequestKind::Apply(values) => {
                let target =
                    target.ok_or_else(|| OpError::Rejected("output target required".into()))?;
                let snapshot = self.snapshot()?;
                if feedback::is_input(&snapshot, target) {
                    feedback::route(&snapshot, target, context, true)?;
                    feedback::intent(values)?;
                    return Ok(());
                }
                if group::is_group(&snapshot, target) {
                    group::route(&snapshot, target, context, true)?;
                    group::intent(values, colour::configured(&snapshot, target))?;
                    return Ok(());
                }
                Self::output_route(&snapshot, target, context, true)?;
                state::intent(values, colour::configured(&snapshot, target))?;
                Ok(())
            }
            RequestKind::Command(call) => {
                let snapshot = self
                    .handle
                    .snapshot()
                    .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
                let change = parse(&snapshot, target, call)?;
                change.admit(&snapshot, target, context).map_err(op_error)
            }
            _ => Err(OpError::Unsupported("target configuration pending".into())),
        }
    }
    async fn connect(&mut self, ctx: &mut LinkContext) -> Result<(), LinkError> {
        let input_events = self.handle.subscribe_inputs();
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| LinkError::new("service is starting"))?;
        if ctx.instance() != snapshot.model.inventory().name {
            return Err(LinkError::new(
                "runtime instance must match persisted controller name",
            ));
        }
        self.handle
            .attach(ctx.report.clone())
            .await
            .map_err(LinkError::new)?;
        self.reporter = Some(ctx.report.clone());
        let helper = state::Watcher::new(self.handle.state.clone(), ctx.report.clone());
        #[cfg(test)]
        let helper = helper.with_pause(self.snapshot_pause.clone());
        ctx.spawn(helper.run());
        ctx.spawn(super::input::events(
            self.handle.clone(),
            input_events,
            ctx.report.clone(),
        ));
        Ok(())
    }
    async fn disconnect(&mut self) {
        self.reporter = None;
    }
    async fn read_state(&mut self, _scope: Scope) -> Result<Vec<TargetState>, OpError> {
        Err(OpError::Unsupported("context required".into()))
    }
    async fn read_state_with_context(
        &mut self,
        scope: Scope,
    ) -> Result<Vec<ContextualState>, OpError> {
        self.read_scope(scope, &RequestContext::default().with_traceparent(None))
            .await
    }
    async fn read_state_with_request_context(
        &mut self,
        scope: Scope,
        context: &RequestContext,
    ) -> Result<Vec<ContextualState>, OpError> {
        self.read_scope(scope, context).await
    }
    async fn apply(&mut self, _target: &str, _values: &PropertyMap) -> Result<(), OpError> {
        Err(OpError::Rejected(
            "original request context required".into(),
        ))
    }
    async fn apply_with_context(
        &mut self,
        target: &str,
        values: &PropertyMap,
        context: &RequestContext,
    ) -> Result<(), OpError> {
        let snapshot = self.snapshot()?;
        if feedback::is_input(&snapshot, target) {
            return feedback::apply(self, &snapshot, target, values, context).await;
        }
        if group::is_group(&snapshot, target) {
            return group::apply(self, &snapshot, target, values, context).await;
        }
        Self::output_route(&snapshot, target, context, true)?;
        let intent = state::intent(values, colour::configured(&snapshot, target))?;
        let deadline = Instant::now()
            + Self::output_budget(
                &snapshot,
                target,
                !matches!(intent, output::WriteIntent::Base(_)),
            );
        let observed = match intent {
            output::WriteIntent::Base(base) => {
                self.handle
                    .apply_output(target, base, context.clone(), deadline)
                    .await
            }
            output::WriteIntent::Colour(colour) => {
                self.handle
                    .apply_colour(target, colour, context.clone(), deadline)
                    .await
            }
            output::WriteIntent::Combined(base, colour) => {
                self.handle
                    .apply_colour_with_output(target, colour, base, context.clone(), deadline)
                    .await
            }
        }
        .map_err(op_error)?;
        if Instant::now() >= deadline {
            return Err(OpError::Rejected(
                "write completion exceeded its original deadline; inspect state before retrying"
                    .into(),
            ));
        }
        let state = state::accepted(&self.snapshot()?, target, &observed)?;
        self.reporter
            .as_ref()
            .ok_or_else(|| OpError::Link(LinkError::new("proxy is disconnected")))?
            .state_with_context(target, state.state.values, false, &state.context);
        Ok(())
    }
    async fn execute(
        &mut self,
        _target: Option<&str>,
        _call: &CommandCall,
    ) -> Result<Option<Value>, OpError> {
        Err(OpError::Rejected(
            "original request context required".into(),
        ))
    }
    async fn execute_with_context(
        &mut self,
        target: Option<&str>,
        call: &CommandCall,
        context: &RequestContext,
    ) -> Result<Option<Value>, OpError> {
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
        let change = parse(&snapshot, target, call)?;
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        if let Operation::Memory(action, revision) = &change {
            let report = self
                .handle
                .memory(
                    target.ok_or_else(|| OpError::Rejected("memory target required".into()))?,
                    *action,
                    context.clone(),
                    Instant::now() + Duration::from_secs(15),
                    Some(*revision),
                )
                .await
                .map_err(op_error)?;
            return serde_json::to_value(report)
                .map(Some)
                .map_err(|error| OpError::Unusable(error.to_string()));
        }
        if let Operation::GroupOutput(action) = &change {
            if !action.writing() {
                let result = self
                    .handle
                    .group_output(
                        target.ok_or_else(|| OpError::Rejected("group target required".into()))?,
                        action.clone(),
                        context.clone(),
                        Instant::now() + Duration::from_secs(15),
                        None,
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        }
        if let Operation::InputFeedback(action) = &change {
            if !action.writing() {
                let result = self
                    .handle
                    .input_feedback(
                        target.ok_or_else(|| OpError::Rejected("input target required".into()))?,
                        action.clone(),
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        None,
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        }
        if let Operation::OutputSettings(action) = &change {
            if !action.writing() {
                let result = self
                    .handle
                    .output_settings(
                        target.ok_or_else(|| {
                            OpError::Rejected("individual output target required".into())
                        })?,
                        action.clone(),
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        None,
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        }
        if let Operation::InputSettings(action) = &change {
            if !action.writing() {
                let result = self
                    .handle
                    .input_settings(
                        target.ok_or_else(|| {
                            OpError::Rejected("input instance target required".into())
                        })?,
                        action.clone(),
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        None,
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        }
        if let Operation::SavedScene = change {
            return serde_json::to_value(
                scene_settings::view::read(
                    &snapshot,
                    target.ok_or_else(|| OpError::Rejected("named scene required".into()))?,
                    context,
                )
                .map_err(op_error)?,
            )
            .map(Some)
            .map_err(|e| OpError::Unusable(e.to_string()));
        }
        if let Operation::SceneSettings(action) = &change {
            if !action.writing() {
                let result = self
                    .handle
                    .scene_setting(
                        target.ok_or_else(|| {
                            OpError::Rejected("scene operation target required".into())
                        })?,
                        action.clone(),
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        None,
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        }
        if let Operation::Scenes(action) = change {
            let budget = if action.writing() { 3 } else { 8 };
            let result = self
                .handle
                .scene(
                    target.ok_or_else(|| {
                        OpError::Rejected("scene operation target required".into())
                    })?,
                    action,
                    context.clone(),
                    Instant::now() + Duration::from_secs(budget),
                )
                .await
                .map_err(op_error)?;
            return serde_json::to_value(result)
                .map(Some)
                .map_err(|e| OpError::Unusable(e.to_string()));
        }
        if let Operation::Identify(stop) = change {
            let result = self
                .handle
                .identify_output(
                    target.ok_or_else(|| OpError::Rejected("output target required".into()))?,
                    stop,
                    context.clone(),
                    Instant::now() + Operation::Identify(stop).budget(),
                )
                .await
                .map_err(op_error)?;
            return serde_json::to_value(result)
                .map(Some)
                .map_err(|e| OpError::Unusable(e.to_string()));
        }
        if let Operation::Groups(groups::Action::Read, _) = change {
            let result = self
                .handle
                .read_groups(
                    target.ok_or_else(|| OpError::Rejected("output target required".into()))?,
                    context.clone(),
                    deadline,
                )
                .await
                .map_err(op_error)?;
            return serde_json::to_value(result)
                .map(Some)
                .map_err(|e| OpError::Unusable(e.to_string()));
        }
        let completion = self
            .reporter
            .as_ref()
            .and_then(Reporter::command_completion)
            .ok_or_else(|| OpError::Rejected("active command completion is required".into()))?;
        let change = match change {
            Operation::Memory(..) => {
                unreachable!("memory reads dispatched before metadata completion")
            }
            Operation::GroupOutput(action) => {
                let result = self
                    .handle
                    .group_output(
                        target.ok_or_else(|| OpError::Rejected("group target required".into()))?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(15),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::InputFeedback(action) => {
                let result = self
                    .handle
                    .input_feedback(
                        target.ok_or_else(|| OpError::Rejected("input target required".into()))?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::OutputSettings(action) => {
                let result = self
                    .handle
                    .output_settings(
                        target.ok_or_else(|| {
                            OpError::Rejected("individual output target required".into())
                        })?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::InputSettings(action) => {
                let result = self
                    .handle
                    .input_settings(
                        target.ok_or_else(|| {
                            OpError::Rejected("input instance target required".into())
                        })?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(8),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::SceneSettings(action) => {
                let result = self
                    .handle
                    .scene_setting(
                        target.ok_or_else(|| {
                            OpError::Rejected("scene operation target required".into())
                        })?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(18),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::Scenes(_) => {
                unreachable!("scene operations dispatched before metadata completion")
            }
            Operation::SavedScene => {
                unreachable!("saved scene dispatched before metadata completion")
            }
            Operation::Identify(_) => {
                unreachable!("identification dispatched before metadata completion")
            }
            Operation::Groups(action, expected) => {
                let result = self
                    .handle
                    .groups(
                        target.ok_or_else(|| OpError::Rejected("output target required".into()))?,
                        action,
                        expected,
                        context.clone(),
                        Instant::now() + Duration::from_secs(18),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::Restore(action) => {
                let result = self
                    .handle
                    .restore(
                        target.ok_or_else(|| {
                            OpError::Rejected("saved fixture target required".into())
                        })?,
                        action,
                        context.clone(),
                        Instant::now() + Duration::from_secs(18),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::Reassignment(p, space) => {
                let result = self
                    .handle
                    .reset_space(
                        p.bus,
                        space,
                        p.expected_revision,
                        context.clone(),
                        Instant::now() + Duration::from_secs(18),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::Assignment(p, space) => {
                let result = self
                    .handle
                    .assign_space(
                        p.bus,
                        space,
                        p.expected_revision,
                        context.clone(),
                        Instant::now() + Duration::from_secs(18),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
            Operation::Metadata(change) => change,
            Operation::Discovery(p) => {
                let result = self
                    .handle
                    .discover(
                        p.bus,
                        p.space,
                        p.address,
                        p.expected_revision,
                        context.clone(),
                        Instant::now() + Duration::from_secs(10),
                        Some(completion),
                    )
                    .await
                    .map_err(op_error)?;
                return serde_json::to_value(result)
                    .map(Some)
                    .map_err(|e| OpError::Unusable(e.to_string()));
            }
        };
        let result = self
            .handle
            .submit(change, context.clone(), deadline, Some(completion))
            .await
            .map_err(op_error)?;
        Ok(Some(
            serde_json::json!({"revision":result.model.inventory().revision}),
        ))
    }
    async fn configure(&mut self, _target: &str, _config: Option<&Value>) -> Result<(), OpError> {
        Err(OpError::Unsupported("target configuration pending".into()))
    }
}

impl DaliProxy {
    fn output_budget(snapshot: &Snapshot, target: &str, colour_write: bool) -> Duration {
        let known_colour = snapshot.outputs.get(target).is_some_and(|sample| {
            sample.colour.is_some()
                && routing::generations(&snapshot.model)
                    .targets
                    .get(target)
                    .map(String::as_str)
                    == sample.context.generation()
        });
        Duration::from_secs(if colour_write {
            8
        } else if known_colour || colour::configured(snapshot, target).is_some() {
            6
        } else {
            2
        })
    }
    fn output_route(
        snapshot: &Snapshot,
        target: &str,
        cause: &RequestContext,
        writing: bool,
    ) -> Result<output::Route, OpError> {
        if snapshot.model.projections().iter().any(|value| {
            value.target == target
                && !matches!(value.location, crate::inventory::Location::Output { .. })
        }) {
            return Err(OpError::Unsupported(
                "individual output required for this operation".into(),
            ));
        }
        output::resolve(snapshot, target, cause, writing).map_err(op_error)
    }
    fn snapshot(&self) -> Result<Snapshot, OpError> {
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
        if matches!(snapshot.phase, Phase::Stopping | Phase::Closed) {
            return Err(OpError::Link(LinkError::new("service stopped")));
        }
        Ok(snapshot)
    }
    fn inventory(snapshot: &Snapshot, cause: &RequestContext) -> Result<ContextualState, OpError> {
        let token = routing::generations(&snapshot.model).instance;
        if cause.generation().is_some_and(|value| value != token) {
            return Err(OpError::Rejected("retired controller generation".into()));
        }
        let context = RequestContext::new(token)
            .map_err(OpError::Unusable)?
            .with_traceparent(cause.traceparent());
        let values =
            serde_json::from_slice(&snapshot.model.state_bytes().map_err(OpError::Unusable)?)
                .map_err(|error| OpError::Unusable(error.to_string()))?;
        Ok(ContextualState::new(
            TargetState::new("__inventory", values),
            context,
        ))
    }
    async fn read_scope(
        &mut self,
        scope: Scope,
        cause: &RequestContext,
    ) -> Result<Vec<ContextualState>, OpError> {
        match scope {
            Scope::Any => Ok(vec![Self::inventory(&self.snapshot()?, cause)?]),
            Scope::Target(target) if target == "__inventory" => {
                Ok(vec![Self::inventory(&self.snapshot()?, cause)?])
            }
            Scope::Target(target) => {
                let snapshot = self.snapshot()?;
                if group::is_group(&snapshot, &target) {
                    let deadline = Instant::now() + Duration::from_secs(15);
                    let observed = group::read(self, &target, cause.clone(), deadline).await?;
                    return Ok(vec![group::accepted(
                        &self.snapshot()?,
                        &target,
                        &observed,
                        deadline,
                    )?]);
                }
                if feedback::is_input(&snapshot, &target) {
                    return Ok(vec![
                        feedback::read_target(self, &snapshot, &target, cause).await?,
                    ]);
                }
                let deadline = Instant::now() + Self::output_budget(&snapshot, &target, false);
                let observed = self
                    .read_observation(&target, cause.clone(), deadline)
                    .await?;
                if Instant::now() >= deadline {
                    return Err(OpError::Rejected(
                        "read completion exceeded its original deadline".into(),
                    ));
                }
                Ok(vec![state::accepted(
                    &self.snapshot()?,
                    &target,
                    &observed,
                )?])
            }
            Scope::All => self.read_all(cause).await,
        }
    }
    async fn read_input_observation(
        &self,
        target: &str,
        cause: RequestContext,
        deadline: Instant,
    ) -> Result<ContextualState, OpError> {
        let observed = self
            .handle
            .read_input(target, cause, deadline)
            .await
            .map_err(op_error)?;
        #[cfg(test)]
        if let Some(pause) = &self.read_pause {
            pause.point().await;
        }
        accepted_input(&self.snapshot()?, target, &observed, deadline)
    }
    async fn read_observation(
        &self,
        target: &str,
        cause: RequestContext,
        deadline: Instant,
    ) -> Result<output::Observation, OpError> {
        Self::output_route(&self.snapshot()?, target, &cause, false)?;
        let observed = self
            .handle
            .read_output(target, cause, deadline)
            .await
            .map_err(op_error)?;
        #[cfg(test)]
        if let Some(pause) = &self.read_pause {
            pause.point().await;
        }
        Ok(observed)
    }
    async fn read_all(&mut self, cause: &RequestContext) -> Result<Vec<ContextualState>, OpError> {
        let snapshot = self.snapshot()?;
        let inventory = Self::inventory(&snapshot, cause)?;
        let table = routing::generations(&snapshot.model);
        let mut targets: Vec<(String,ReadKind)> = snapshot.model.projections().into_iter()
            .filter(|value| matches!(value.location, crate::inventory::Location::Output { bus, .. } | crate::inventory::Location::InputInstance { bus, .. } | crate::inventory::Location::Group { bus, .. }
                if snapshot.model.inventory().buses.iter().any(|value| value.bus == bus && value.enabled)))
            .map(|value| { let kind = match value.location { crate::inventory::Location::InputInstance { .. } => ReadKind::Input, crate::inventory::Location::Group { .. } => ReadKind::Group, _ => ReadKind::Output }; (value.target,kind) }).collect();
        // Preserve the existing individual/input order; append new group work.
        // The same persistent cursor rotates across the complete endpoint list.
        targets.sort_by_key(|(_, kind)| matches!(kind, ReadKind::Group));
        targets.extend(
            snapshot
                .feedbacks
                .keys()
                .filter(|target| table.targets.contains_key(*target))
                .cloned()
                .map(|target| (target, ReadKind::Feedback)),
        );
        if targets.is_empty() {
            let mut states = vec![inventory];
            states.extend(state::projections(&snapshot));
            return Ok(states);
        }
        if !matches!(snapshot.hardware, Hardware::Ready { .. }) {
            return Err(OpError::Rejected(
                "UART is not ready; inventory remains available".into(),
            ));
        }
        let start = self.read_cursor % targets.len();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut observed = Vec::new();
        let mut observed_inputs = Vec::new();
        let mut observed_groups = Vec::new();
        let mut observed_feedbacks = Vec::new();
        let mut failed = 0;
        for offset in 0..targets.len() {
            if Instant::now() >= deadline {
                failed += targets.len() - offset;
                break;
            }
            let index = (start + offset) % targets.len();
            // Persist before the first await: runtime cancellation/reconnect
            // must not keep restarting at the same fixture.
            self.read_cursor = (index + 1) % targets.len();
            let (target, kind) = &targets[index];
            let context = RequestContext::new(table.targets[target].clone())
                .map_err(OpError::Unusable)?
                .with_traceparent(cause.traceparent());
            let budget = match kind {
                ReadKind::Input => Duration::from_secs(4),
                ReadKind::Feedback => Duration::from_secs(8),
                ReadKind::Group => Duration::from_secs(15),
                ReadKind::Output => Self::output_budget(&self.snapshot()?, target, false),
            };
            let until = deadline.min(Instant::now() + budget);
            if matches!(kind, ReadKind::Feedback) {
                match feedback::read(self, target, context, until).await {
                    Ok(value) => observed_feedbacks.push((target.clone(), value)),
                    Err(_) => failed += 1,
                }
            } else if matches!(kind, ReadKind::Group) {
                match group::read(self, target, context, until).await {
                    Ok(value) => observed_groups.push((target.clone(), value)),
                    Err(_) => failed += 1,
                }
            } else if matches!(kind, ReadKind::Input) {
                match self.read_input_observation(target, context, until).await {
                    Ok(state) => observed_inputs.push((target.clone(), state.context)),
                    Err(_) => failed += 1,
                }
            } else {
                match self.read_observation(target, context, until).await {
                    Ok(value) => observed.push((target.clone(), value)),
                    Err(_) => failed += 1,
                }
            }
        }
        let latest = self.snapshot()?;
        if latest.model.inventory().revision != snapshot.model.inventory().revision
            || latest.model.inventory().inventory_epoch
                != snapshot.model.inventory().inventory_epoch
            || Instant::now() >= deadline
        {
            return Err(OpError::Rejected(
                "complete readback changed or exceeded its original deadline".into(),
            ));
        }
        let mut states = vec![inventory];
        for (target, context) in observed_inputs {
            let valid=latest.inputs.get(&target).is_some_and(|value|!value.stale && value.context.generation()==context.generation()
                && matches!(&latest.hardware,Hardware::Ready{buses,..} if buses.iter().any(|b|b.bus==value.bus && b.status==2)));
            if !valid {
                failed += 1;
            }
        }
        let mut projected: std::collections::BTreeMap<_, _> = state::projections(&latest)
            .into_iter()
            .map(|value| (value.state.target.clone(), value))
            .collect();
        for (target, value) in observed {
            match state::accept_projected(&latest, &target, &value, projected.remove(&target)) {
                Ok(state) => states.push(state),
                Err(_) => failed += 1,
            }
        }
        for (target, value) in observed_groups {
            projected.remove(&target);
            match group::accepted(&latest, &target, &value, deadline) {
                Ok(state) => states.push(state),
                Err(_) => failed += 1,
            }
        }
        for (target, value) in observed_feedbacks {
            match feedback::accepted(&latest, &target, &value, deadline) {
                Ok(state) => {
                    projected.insert(target, state);
                }
                Err(_) => failed += 1,
            }
        }
        if failed != 0 {
            return Err(OpError::Rejected(format!(
                "readback incomplete for {failed} endpoint(s); inspect current state"
            )));
        }
        states.extend(projected.into_values());
        Ok(states)
    }
}

fn op_error(error: ServiceError) -> OpError {
    match error {
        ServiceError::Closed => OpError::Link(LinkError::new("service stopped")),
        error => OpError::Rejected(error.to_string()),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    expected_revision: u64,
    name: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Removal {
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameBus {
    expected_revision: u64,
    bus: u8,
    name: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigureBus {
    expected_revision: u64,
    bus: u8,
    name: String,
    enabled: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Describe {
    expected_revision: u64,
    description: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorSelection {
    expected_revision: u64,
    color_representation: Option<crate::config::ColorRepresentation>,
}
fn decode<T: serde::de::DeserializeOwned>(call: &CommandCall) -> Result<T, OpError> {
    serde_json::from_value(Value::Object(call.params.clone()))
        .map_err(|e| OpError::Rejected(e.to_string()))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    bus: u8,
    expected_revision: u64,
}
enum Operation {
    Memory(memory::Action, u64),
    GroupOutput(group_output::Action),
    InputFeedback(input::feedback::Action),
    InputSettings(input::settings::Action),
    OutputSettings(output::settings::Action),
    Restore(restoration::Action),
    Reassignment(Assignment, discovery::Space),
    SavedScene,
    SceneSettings(scene_settings::Action),
    Scenes(scenes::Action),
    Identify(bool),
    Groups(groups::Action, Option<u64>),
    Assignment(Assignment, discovery::Space),
    Metadata(Change),
    Discovery(Discovery),
}
impl Operation {
    fn admit(
        &self,
        snapshot: &Snapshot,
        target: Option<&str>,
        context: &RequestContext,
    ) -> Result<(), ServiceError> {
        match self {
            Self::GroupOutput(action) => group_output::resolve(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("group target required".into()))?,
                action,
                context,
            )
            .map(|_| ()),
            Self::InputFeedback(action) => input::feedback::resolve(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("input target required".into()))?,
                action,
                context,
            )
            .map(|_| ()),
            Self::Memory(action, _) => memory::resolve(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("memory target required".into()))?,
                *action,
                context,
            )
            .map(|_| ()),
            Self::OutputSettings(action) => output::settings::resolve(
                snapshot,
                target.ok_or_else(|| {
                    ServiceError::Rejected("individual output target required".into())
                })?,
                action,
                context,
            )
            .map(|_| ()),
            Self::InputSettings(action) => input::settings::resolve(
                snapshot,
                target.ok_or_else(|| {
                    ServiceError::Rejected("input instance target required".into())
                })?,
                action,
                context,
            )
            .map(|_| ()),
            Self::SavedScene => scene_settings::view::read(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("named scene required".into()))?,
                context,
            )
            .map(|_| ()),
            Self::SceneSettings(action) => scene_settings::resolve(
                snapshot,
                target.ok_or_else(|| {
                    ServiceError::Rejected("scene operation target required".into())
                })?,
                action,
                context,
                false,
            )
            .map(|_| ()),
            Self::Scenes(action) => scenes::resolve(
                snapshot,
                target.ok_or_else(|| {
                    ServiceError::Rejected("scene operation target required".into())
                })?,
                *action,
                context,
            )
            .map(|_| ()),
            Self::Identify(_) => identification::admit(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("output target required".into()))?,
                context,
            ),
            Self::Groups(action, expected) => groups::resolve(
                snapshot,
                target.ok_or_else(|| ServiceError::Rejected("output target required".into()))?,
                *action,
                *expected,
                context,
            )
            .map(|_| ()),
            Self::Restore(action) => restoration::resolve(
                snapshot,
                target.ok_or_else(|| {
                    ServiceError::Rejected("saved fixture target required".into())
                })?,
                action,
                context,
            )
            .map(|_| ()),
            Self::Reassignment(p, space) => {
                commissioning::resolve_space(snapshot, p.bus, *space, p.expected_revision, context)
                    .map(|_| ())
            }
            Self::Assignment(p, space) => match space {
                discovery::Space::Outputs => {
                    commissioning::resolve(snapshot, p.bus, p.expected_revision, context)
                }
                discovery::Space::Inputs => commissioning::resolve_space(
                    snapshot,
                    p.bus,
                    *space,
                    p.expected_revision,
                    context,
                ),
            }
            .map(|_| ()),
            Self::Metadata(change) => routing::admit(snapshot, change, context),
            Self::Discovery(p) => discovery::resolve(
                snapshot,
                p.bus,
                p.space,
                p.address,
                p.expected_revision,
                context,
            )
            .map(|_| ()),
        }
    }
    fn budget(&self) -> Duration {
        Duration::from_secs(match self {
            Self::Memory(..) => 15,
            Self::GroupOutput(_) => 15,
            Self::InputFeedback(_) => 8,
            Self::InputSettings(_) => 8,
            Self::OutputSettings(_) => 8,
            Self::SceneSettings(action) => {
                if action.writing() {
                    18
                } else {
                    8
                }
            }
            Self::Scenes(action) => {
                if action.writing() {
                    3
                } else {
                    8
                }
            }
            Self::Identify(_) => 8,
            Self::SavedScene | Self::Metadata(_) | Self::Groups(groups::Action::Read, _) => 2,
            Self::Discovery(_) => 10,
            Self::Restore(_)
            | Self::Reassignment(_, _)
            | Self::Groups(_, _)
            | Self::Assignment(_, _) => 18,
        })
    }
}

/// HTTP composition uses typed service calls, never an SDK worker completion.
pub(crate) struct PreparedCommand {
    operation: PreparedAction,
    target: Option<String>,
    pub(crate) context: RequestContext,
    deadline: Instant,
}
pub(crate) enum PreparationError {
    Invalid,
    Conflict,
}
enum PreparedAction {
    Command(Operation),
    Output(Option<output::WriteIntent>),
    Input,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputValues {
    values: PropertyMap,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupColorValues {
    expected_revision: u64,
    values: PropertyMap,
}
impl PreparedAction {
    fn prepare(
        snapshot: &Snapshot,
        target: Option<&str>,
        call: &CommandCall,
        context: &RequestContext,
    ) -> Result<(Self, Duration), PreparationError> {
        let action = match call.command.as_str() {
            "apply_output" | "read_output" | "read_input" => {
                let target = target.ok_or(PreparationError::Invalid)?;
                let location = snapshot
                    .model
                    .projections()
                    .into_iter()
                    .find(|p| p.target == target)
                    .ok_or(PreparationError::Conflict)?
                    .location;
                if call.command == "read_input" {
                    if !call.params.is_empty()
                        || !matches!(location, crate::inventory::Location::InputInstance { .. })
                    {
                        return Err(PreparationError::Invalid);
                    }
                    input::readback::resolve(snapshot, target, context)
                        .map_err(|_| PreparationError::Conflict)?;
                    return Ok((Self::Input, Duration::from_secs(4)));
                }
                if !matches!(location, crate::inventory::Location::Output { .. }) {
                    return Err(PreparationError::Invalid);
                }
                let intent = if call.command == "apply_output" {
                    let p: OutputValues = decode(call).map_err(|_| PreparationError::Invalid)?;
                    Some(
                        state::intent(&p.values, colour::configured(snapshot, target))
                            .map_err(|_| PreparationError::Invalid)?,
                    )
                } else {
                    if !call.params.is_empty() {
                        return Err(PreparationError::Invalid);
                    }
                    None
                };
                DaliProxy::output_route(snapshot, target, context, intent.is_some())
                    .map_err(|_| PreparationError::Conflict)?;
                let colour_write = matches!(
                    intent,
                    Some(output::WriteIntent::Colour(_) | output::WriteIntent::Combined(_, _))
                );
                return Ok((
                    Self::Output(intent),
                    DaliProxy::output_budget(snapshot, target, colour_write),
                ));
            }
            _ => {
                Self::Command(parse(snapshot, target, call).map_err(|_| PreparationError::Invalid)?)
            }
        };
        let Self::Command(operation) = &action else {
            unreachable!()
        };
        operation
            .admit(snapshot, target, context)
            .map_err(|_| PreparationError::Conflict)?;
        let budget = operation.budget();
        Ok((action, budget))
    }
}
fn accepted_input(
    snapshot: &Snapshot,
    target: &str,
    observed: &input::Observation,
    deadline: Instant,
) -> Result<ContextualState, OpError> {
    input::readback::resolve(snapshot, target, &observed.context).map_err(op_error)?;
    if Instant::now() >= deadline
        || !snapshot.inputs.get(target).is_some_and(|value| {
            !value.stale && value.context.generation() == observed.context.generation()
        })
        || !matches!(&snapshot.hardware,Hardware::Ready{buses,..} if buses.iter().any(|b|b.bus==observed.bus && b.status==2))
    {
        return Err(OpError::Rejected(
            "input evidence changed before MQTT completion".into(),
        ));
    }
    input::projections(snapshot)
        .into_iter()
        .find(|s| {
            s.state.target == target && s.context.generation() == observed.context.generation()
        })
        .ok_or_else(|| OpError::Rejected("input mapping changed before MQTT completion".into()))
}
fn projected_state(state: ContextualState) -> Value {
    serde_json::json!({"target":state.state.target,"generation":state.context.generation(),"values":state.state.values})
}
pub(crate) fn projected_states(snapshot: &Snapshot) -> Vec<Value> {
    state::projections(snapshot)
        .into_iter()
        .map(projected_state)
        .collect()
}
pub(super) fn accept_output(
    snapshot: &Snapshot,
    target: &str,
    observed: &output::Observation,
) -> Result<(), ServiceError> {
    state::accepted(snapshot, target, observed)
        .map(|_| ())
        .map_err(|e| ServiceError::Rejected(e.to_string()))
}
impl PreparedCommand {
    pub(crate) fn new(
        snapshot: &Snapshot,
        target: Option<String>,
        call: &CommandCall,
        context: RequestContext,
    ) -> Result<Self, PreparationError> {
        let table = routing::generations(&snapshot.model);
        let token = match target.as_deref() {
            Some("__inventory") => return Err(PreparationError::Invalid),
            Some(target) => table
                .targets
                .get(target)
                .ok_or(PreparationError::Conflict)?,
            None => &table.instance,
        };
        if context.generation() != Some(token.as_str()) {
            return Err(PreparationError::Conflict);
        }
        let (operation, budget) =
            PreparedAction::prepare(snapshot, target.as_deref(), call, &context)?;
        let deadline = Instant::now() + budget;
        Ok(Self {
            operation,
            target,
            context,
            deadline,
        })
    }
    pub(crate) async fn run(self, handle: ServiceHandle) -> Result<Value, ServiceError> {
        let Self {
            operation,
            target,
            context,
            deadline,
        } = self;
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        fn value<T: serde::Serialize>(value: T) -> Result<Value, ServiceError> {
            serde_json::to_value(value)
                .map_err(|_| ServiceError::Uncertain("result encoding failed".into()))
        }
        let operation = match operation {
            PreparedAction::Command(operation) => operation,
            PreparedAction::Output(intent) => {
                let target = target
                    .ok_or_else(|| ServiceError::Rejected("output target required".into()))?;
                let observed = match intent {
                    None => handle.read_output(&target, context, deadline).await,
                    Some(output::WriteIntent::Base(base)) => {
                        handle.apply_output(&target, base, context, deadline).await
                    }
                    Some(output::WriteIntent::Colour(colour)) => {
                        handle
                            .apply_colour(&target, colour, context, deadline)
                            .await
                    }
                    Some(output::WriteIntent::Combined(base, colour)) => {
                        handle
                            .apply_colour_with_output(&target, colour, base, context, deadline)
                            .await
                    }
                }?;
                if Instant::now() >= deadline {
                    return Err(ServiceError::Uncertain(
                        "output completion exceeded its original deadline".into(),
                    ));
                }
                let latest = handle.snapshot().ok_or(ServiceError::Closed)?;
                return state::accepted(&latest, &target, &observed)
                    .map(projected_state)
                    .map_err(|e| ServiceError::Rejected(e.to_string()));
            }
            PreparedAction::Input => {
                let target = target.ok_or_else(|| {
                    ServiceError::Rejected("input instance target required".into())
                })?;
                let observed = handle.read_input(&target, context, deadline).await?;
                let latest = handle.snapshot().ok_or(ServiceError::Closed)?;
                return accepted_input(&latest, &target, &observed, deadline)
                    .map(projected_state)
                    .map_err(|e| ServiceError::Rejected(e.to_string()));
            }
        };
        match operation {
            Operation::Memory(action, revision) => value(
                handle
                    .memory(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("memory target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        Some(revision),
                    )
                    .await?,
            ),
            Operation::GroupOutput(action) => value(
                handle
                    .group_output(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("group target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::InputFeedback(action) => value(
                handle
                    .input_feedback(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("input target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::OutputSettings(action) => value(
                handle
                    .output_settings(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("individual output target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::InputSettings(action) => value(
                handle
                    .input_settings(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("input instance target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::Restore(action) => value(
                handle
                    .restore(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("saved fixture target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::Reassignment(p, space) => value(
                handle
                    .reset_space(p.bus, space, p.expected_revision, context, deadline, None)
                    .await?,
            ),
            Operation::SavedScene => value(scene_settings::view::read(
                &handle.snapshot().ok_or(ServiceError::Closed)?,
                target
                    .as_deref()
                    .ok_or_else(|| ServiceError::Rejected("named scene required".into()))?,
                &context,
            )?),
            Operation::SceneSettings(action) => value(
                handle
                    .scene_setting(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("scene operation target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::Scenes(action) => value(
                handle
                    .scene(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("scene operation target required".into())
                        })?,
                        action,
                        context,
                        deadline,
                    )
                    .await?,
            ),
            Operation::Identify(stop) => value(
                handle
                    .identify_output(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("output target required".into())
                        })?,
                        stop,
                        context,
                        deadline,
                    )
                    .await?,
            ),
            Operation::Metadata(change) => {
                let snapshot = handle.submit(change, context, deadline, None).await?;
                Ok(serde_json::json!({"revision":snapshot.model.inventory().revision}))
            }
            Operation::Assignment(p, space) => value(
                handle
                    .assign_space(p.bus, space, p.expected_revision, context, deadline, None)
                    .await?,
            ),
            Operation::Discovery(p) => value(
                handle
                    .discover(
                        p.bus,
                        p.space,
                        p.address,
                        p.expected_revision,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
            Operation::Groups(action, expected) => value(
                handle
                    .groups(
                        target.as_deref().ok_or_else(|| {
                            ServiceError::Rejected("output target required".into())
                        })?,
                        action,
                        expected,
                        context,
                        deadline,
                        None,
                    )
                    .await?,
            ),
        }
    }
}
struct Discovery {
    bus: u8,
    space: discovery::Space,
    address: u8,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoveryParams {
    bus: u8,
    space: String,
    address: u8,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupAssignment {
    group: u8,
    member: bool,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupReview {
    reviewed_mask: u16,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupCreation {
    bus: u8,
    group: u8,
    name: String,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneCreation {
    bus: u8,
    slot: u8,
    name: String,
    description: Option<String>,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneRead {
    slot: u8,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneRowSet {
    fixture: String,
    setting: crate::config::scene_settings::Setting,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneRowReview {
    fixture: String,
    reviewed: scenes::Stored,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneRecall {
    group: Option<u8>,
    expected_revision: u64,
}
fn parse(
    snapshot: &Snapshot,
    target: Option<&str>,
    call: &CommandCall,
) -> Result<Operation, OpError> {
    if matches!(
        call.command.as_str(),
        "read_identity"
            | "read_memory"
            | "read_luminaire_data"
            | "read_energy_data"
            | "read_diagnostics"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("memory target required".into()));
        }
        if call.command == "read_energy_data" || call.command == "read_diagnostics" {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Parameters {
                bank: u8,
            }
            let params: Parameters = decode(call)?;
            let action = if call.command == "read_energy_data" {
                memory::Action::Energy(
                    crate::dali::energy::Bank::new(params.bank)
                        .map_err(|reason| OpError::Rejected(reason.into()))?,
                )
            } else {
                memory::Action::Diagnostics(
                    crate::dali::diagnostics::Bank::new(params.bank)
                        .map_err(|reason| OpError::Rejected(reason.into()))?,
                )
            };
            return Ok(Operation::Memory(
                action,
                snapshot.model.inventory().revision,
            ));
        }
        if call.command != "read_memory" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "identity/luminaire reads accept no parameters".into(),
                ));
            }
            return Ok(Operation::Memory(
                if call.command == "read_identity" {
                    memory::Action::Identity
                } else {
                    memory::Action::Luminaire
                },
                snapshot.model.inventory().revision,
            ));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Parameters {
            bank: u8,
            start: u8,
            count: u8,
        }
        let params: Parameters = decode(call)?;
        let window = crate::dali::memory::Window::new(params.start, params.count)
            .map_err(|reason| OpError::Rejected(reason.into()))?;
        return Ok(Operation::Memory(
            memory::Action::Read {
                bank: params.bank,
                window,
            },
            snapshot.model.inventory().revision,
        ));
    }
    if matches!(
        call.command.as_str(),
        "read_group_color" | "set_group_color"
    ) {
        let target = target.ok_or_else(|| OpError::Rejected("group target required".into()))?;
        if call.command == "read_group_color" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "read_group_color accepts no parameters".into(),
                ));
            }
            return Ok(Operation::GroupOutput(group_output::Action::ReadColor));
        }
        let params: GroupColorValues = decode(call)?;
        let intent = group::intent(&params.values, colour::configured(snapshot, target))?;
        if intent.colour().is_none() {
            return Err(OpError::Rejected(
                "set_group_color requires a color value".into(),
            ));
        }
        return Ok(Operation::GroupOutput(group_output::Action::Color {
            expected: params.expected_revision,
            intent,
        }));
    }
    if matches!(
        call.command.as_str(),
        "read_group_output" | "set_group_output"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("group target required".into()));
        }
        if call.command == "read_group_output" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "read_group_output accepts no parameters".into(),
                ));
            }
            return Ok(Operation::GroupOutput(group_output::Action::Read));
        }
        if call
            .params
            .keys()
            .any(|k| !matches!(k.as_str(), "expected_revision" | "power" | "brightness"))
        {
            return Err(OpError::Rejected("unknown group power parameter".into()));
        }
        let expected = call
            .params
            .get("expected_revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                OpError::Rejected("expected_revision must be an unsigned integer".into())
            })?;
        let power = call
            .params
            .get("power")
            .map(|v| {
                v.as_bool()
                    .ok_or_else(|| OpError::Rejected("power must be a JSON boolean".into()))
            })
            .transpose()?;
        let brightness = call
            .params
            .get("brightness")
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| OpError::Rejected("brightness must be a JSON number".into()))
            })
            .transpose()?;
        let intent = crate::dali::gear::Intent::new(power, brightness)
            .map_err(|e| OpError::Rejected(e.to_string()))?;
        return Ok(Operation::GroupOutput(group_output::Action::Set {
            expected,
            intent,
        }));
    }
    if matches!(
        call.command.as_str(),
        "read_input_feedback" | "set_input_feedback"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("input target required".into()));
        }
        if call.command == "read_input_feedback" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "read_input_feedback accepts no parameters".into(),
                ));
            }
            return Ok(Operation::InputFeedback(input::feedback::Action::Read));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Set {
            expected_revision: u64,
            active: bool,
        }
        let p: Set = decode(call)?;
        return Ok(Operation::InputFeedback(input::feedback::Action::Set {
            expected: p.expected_revision,
            active: p.active,
        }));
    }
    if matches!(
        call.command.as_str(),
        "read_output_settings" | "configure_output"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected(
                "individual output target required".into(),
            ));
        }
        if call.command == "read_output_settings" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "read_output_settings accepts no parameters".into(),
                ));
            }
            return Ok(Operation::OutputSettings(output::settings::Action::Read));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Configure {
            expected_revision: u64,
            settings: output::settings::Patch,
        }
        let p: Configure = decode(call)?;
        return Ok(Operation::OutputSettings(
            output::settings::Action::Configure {
                expected: p.expected_revision,
                settings: p.settings,
            },
        ));
    }
    if matches!(
        call.command.as_str(),
        "read_input_settings" | "configure_input"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("input instance target required".into()));
        }
        if call.command == "read_input_settings" {
            if !call.params.is_empty() {
                return Err(OpError::Rejected(
                    "read_input_settings accepts no parameters".into(),
                ));
            }
            return Ok(Operation::InputSettings(input::settings::Action::Read));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Configure {
            expected_revision: u64,
            settings: input::settings::Patch,
        }
        let p: Configure = decode(call)?;
        return Ok(Operation::InputSettings(
            input::settings::Action::Configure {
                expected: p.expected_revision,
                settings: p.settings,
            },
        ));
    }
    if call.command == "read_scene_settings" {
        if target.is_none() || !call.params.is_empty() {
            return Err(OpError::Rejected(
                "saved scene read requires a scene target and no parameters".into(),
            ));
        }
        return Ok(Operation::SavedScene);
    }
    if matches!(
        call.command.as_str(),
        "set_scene_row" | "adopt_scene_row" | "repair_scene_row" | "capture_scene_row"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("scene operation target required".into()));
        }
        let action = match call.command.as_str() {
            "set_scene_row" => {
                let p: SceneRowSet = decode(call)?;
                scene_settings::Action::Set {
                    fixture: p.fixture,
                    setting: p.setting,
                    expected_revision: p.expected_revision,
                }
            }
            "capture_scene_row" => {
                let p: SceneRead = decode(call)?;
                scene_settings::Action::Capture {
                    slot: crate::dali::scenes::Slot::new(p.slot)
                        .map_err(|e| OpError::Rejected(e.to_string()))?,
                }
            }
            name => {
                let p: SceneRowReview = decode(call)?;
                if name == "adopt_scene_row" {
                    scene_settings::Action::Adopt {
                        fixture: p.fixture,
                        reviewed: p.reviewed,
                        expected_revision: p.expected_revision,
                    }
                } else {
                    scene_settings::Action::Repair {
                        fixture: p.fixture,
                        reviewed: p.reviewed,
                        expected_revision: p.expected_revision,
                    }
                }
            }
        };
        return Ok(Operation::SceneSettings(action));
    }
    if matches!(
        call.command.as_str(),
        "restore_unaddressed_address" | "restore_identified_address" | "clear_short_address"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("saved fixture target required".into()));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Unique {
            expected_revision: u64,
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Identified {
            expected_revision: u64,
            candidate: String,
            candidate_generation: String,
        }
        let action = if call.command == "clear_short_address" {
            let p: Unique = decode(call)?;
            restoration::Action::ClearShortAddress {
                expected_revision: p.expected_revision,
            }
        } else if call.command == "restore_unaddressed_address" {
            let p: Unique = decode(call)?;
            restoration::Action::OnlyUnaddressed {
                expected_revision: p.expected_revision,
            }
        } else {
            let p: Identified = decode(call)?;
            restoration::Action::Identified {
                candidate: p.candidate,
                candidate_generation: p.candidate_generation,
                expected_revision: p.expected_revision,
            }
        };
        return Ok(Operation::Restore(action));
    }
    if call.command == "reset_address_space" {
        if target.is_some() {
            return Err(OpError::Rejected("controller-only command".into()));
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reset {
            bus: u8,
            space: String,
            expected_revision: u64,
        }
        let p: Reset = decode(call)?;
        let space = match p.space.as_str() {
            "outputs" => discovery::Space::Outputs,
            "inputs" => discovery::Space::Inputs,
            _ => return Err(OpError::Rejected("space must be outputs or inputs".into())),
        };
        return Ok(Operation::Reassignment(
            Assignment {
                bus: p.bus,
                expected_revision: p.expected_revision,
            },
            space,
        ));
    }
    if call.command == "read_scene" || call.command == "recall_scene" {
        if target.is_none() {
            return Err(OpError::Rejected("scene operation target required".into()));
        }
        let action = if call.command == "read_scene" {
            let p: SceneRead = decode(call)?;
            scenes::Action::Read {
                slot: crate::dali::scenes::Slot::new(p.slot)
                    .map_err(|e| OpError::Rejected(e.to_string()))?,
            }
        } else {
            let p: SceneRecall = decode(call)?;
            if call.params.contains_key("group") && p.group.is_none() {
                return Err(OpError::Rejected(
                    "group must be an integer in 0..15".into(),
                ));
            }
            let group = p
                .group
                .map(crate::dali::gear::GroupAddress::new)
                .transpose()
                .map_err(|e| OpError::Rejected(e.to_string()))?;
            scenes::Action::Recall {
                group,
                expected_revision: p.expected_revision,
            }
        };
        return Ok(Operation::Scenes(action));
    }
    if matches!(call.command.as_str(), "blink" | "stop_blink") {
        if target.is_none() || !call.params.is_empty() {
            return Err(OpError::Rejected(
                "identification needs an output target and no parameters".into(),
            ));
        }
        return Ok(Operation::Identify(call.command == "stop_blink"));
    }
    if matches!(
        call.command.as_str(),
        "read_groups" | "assign_group" | "adopt_groups" | "repair_groups"
    ) {
        if target.is_none() {
            return Err(OpError::Rejected("output target required".into()));
        }
        return match call.command.as_str() {
            "read_groups" if call.params.is_empty() => {
                Ok(Operation::Groups(groups::Action::Read, None))
            }
            "assign_group" => {
                let p: GroupAssignment = decode(call)?;
                let group = crate::dali::gear::GroupAddress::new(p.group)
                    .map_err(|e| OpError::Rejected(e.to_string()))?;
                Ok(Operation::Groups(
                    groups::Action::Assign {
                        group,
                        member: p.member,
                    },
                    Some(p.expected_revision),
                ))
            }
            "adopt_groups" | "repair_groups" => {
                let p: GroupReview = decode(call)?;
                Ok(Operation::Groups(
                    if call.command == "adopt_groups" {
                        groups::Action::Adopt {
                            reviewed_mask: p.reviewed_mask,
                        }
                    } else {
                        groups::Action::Repair {
                            reviewed_mask: p.reviewed_mask,
                        }
                    },
                    Some(p.expected_revision),
                ))
            }
            _ => Err(OpError::Rejected(
                "read_groups accepts no parameters".into(),
            )),
        };
    }
    if matches!(
        call.command.as_str(),
        "assign_next_output" | "assign_next_input"
    ) {
        if target.is_some() {
            return Err(OpError::Rejected(
                "assignment requires controller target".into(),
            ));
        }
        return decode(call).map(|p| {
            Operation::Assignment(
                p,
                if call.command == "assign_next_output" {
                    discovery::Space::Outputs
                } else {
                    discovery::Space::Inputs
                },
            )
        });
    }
    if call.command == "discover_address" {
        if target.is_some() {
            return Err(OpError::Rejected(
                "discovery requires controller target".into(),
            ));
        }
        let p: DiscoveryParams = decode(call)?;
        let space = match p.space.as_str() {
            "outputs" => discovery::Space::Outputs,
            "inputs" => discovery::Space::Inputs,
            _ => return Err(OpError::Rejected("space must be outputs or inputs".into())),
        };
        return Ok(Operation::Discovery(Discovery {
            bus: p.bus,
            space,
            address: p.address,
            expected_revision: p.expected_revision,
        }));
    }
    parse_metadata(snapshot, target, call).map(Operation::Metadata)
}
fn parse_metadata(
    snapshot: &Snapshot,
    target: Option<&str>,
    call: &CommandCall,
) -> Result<Change, OpError> {
    let edit = match (call.command.as_str(), target) {
        ("create_scene", None) => {
            let p: SceneCreation = decode(call)?;
            (
                p.expected_revision,
                MetadataEdit::CreateScene {
                    bus: p.bus,
                    slot: p.slot,
                    name: p.name,
                    description: p.description,
                },
            )
        }
        ("create_group", None) => {
            let p: GroupCreation = decode(call)?;
            (
                p.expected_revision,
                MetadataEdit::CreateGroup {
                    bus: p.bus,
                    group: p.group,
                    name: p.name,
                },
            )
        }
        ("remove_from_list", Some(target)) => {
            let params: Removal = decode(call)?;
            let projection = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
                .ok_or_else(|| OpError::Rejected("unknown item".into()))?;
            let edit = match projection.location {
                crate::inventory::Location::Output { bus, short_address } => {
                    MetadataEdit::RemoveOutput {
                        bus,
                        address: short_address,
                    }
                }
                crate::inventory::Location::Input { bus, short_address } => {
                    MetadataEdit::RemoveInput {
                        bus,
                        address: short_address,
                    }
                }
                crate::inventory::Location::Scene { bus, slot } => {
                    MetadataEdit::RemoveScene { bus, slot }
                }
                _ => {
                    return Err(OpError::Unsupported(
                        "output, input module or scene required".into(),
                    ))
                }
            };
            (params.expected_revision, edit)
        }
        ("set_color_representation", Some(target)) => {
            let params: ColorSelection = decode(call)?;
            if !call.params.contains_key("color_representation") {
                return Err(OpError::Rejected(
                    "color_representation is required; use null to clear".into(),
                ));
            }
            let projection = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
                .ok_or_else(|| OpError::Rejected("unknown output".into()))?;
            let edit = match projection.location {
                crate::inventory::Location::Output { bus, short_address } => {
                    MetadataEdit::OutputColorRepresentation {
                        bus,
                        address: short_address,
                        representation: params.color_representation,
                    }
                }
                crate::inventory::Location::Group { bus, group_address } => {
                    MetadataEdit::GroupColorRepresentation {
                        bus,
                        address: group_address,
                        representation: params.color_representation,
                    }
                }
                _ => return Err(OpError::Unsupported("output or group required".into())),
            };
            (params.expected_revision, edit)
        }
        ("rename", Some(target)) if target != "__inventory" => {
            let params: Rename = decode(call)?;
            let projection = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
                .ok_or_else(|| OpError::Rejected("unknown item".into()))?;
            let edit = match projection.location {
                crate::inventory::Location::Input { bus, short_address } => {
                    MetadataEdit::RenameInput {
                        bus,
                        address: short_address,
                        name: params.name,
                    }
                }
                crate::inventory::Location::InputInstance {
                    bus,
                    short_address,
                    instance,
                } => MetadataEdit::RenameInputInstance {
                    bus,
                    address: short_address,
                    instance,
                    name: params.name,
                },
                crate::inventory::Location::Output { bus, short_address } => {
                    MetadataEdit::RenameOutput {
                        bus,
                        address: short_address,
                        name: params.name,
                    }
                }
                crate::inventory::Location::Group { bus, group_address } => {
                    MetadataEdit::RenameGroup {
                        bus,
                        address: group_address,
                        name: params.name,
                    }
                }
                crate::inventory::Location::Scene { bus, slot } => MetadataEdit::RenameScene {
                    bus,
                    slot,
                    name: params.name,
                },
            };
            (params.expected_revision, edit)
        }
        ("configure_bus", None) => {
            let params: ConfigureBus = decode(call)?;
            (
                params.expected_revision,
                MetadataEdit::ConfigureBus {
                    bus: params.bus,
                    name: params.name,
                    enabled: params.enabled,
                },
            )
        }
        ("rename_bus", None) => {
            let params: RenameBus = decode(call)?;
            (
                params.expected_revision,
                MetadataEdit::RenameBus {
                    bus: params.bus,
                    name: params.name,
                },
            )
        }
        ("describe", Some(target)) if target != "__inventory" => {
            let projection = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
                .ok_or_else(|| OpError::Rejected("unknown scene".into()))?;
            let crate::inventory::Location::Scene { bus, slot } = projection.location else {
                return Err(OpError::Unsupported("scene target required".into()));
            };
            let params: Describe = decode(call)?;
            if !call.params.contains_key("description") {
                return Err(OpError::Rejected(
                    "description is required; use null to clear".into(),
                ));
            }
            (
                params.expected_revision,
                MetadataEdit::DescribeScene {
                    bus,
                    slot,
                    description: params.description,
                },
            )
        }
        ("describe", None) => {
            let params: Describe = decode(call)?;
            if !call.params.contains_key("description") {
                return Err(OpError::Rejected(
                    "description is required; use null to clear".into(),
                ));
            }
            (
                params.expected_revision,
                MetadataEdit::Description(params.description),
            )
        }
        ("recover", None) if call.params.is_empty() => return Ok(Change::Recover),
        _ => return Err(OpError::Unsupported("unsupported command or target".into())),
    };
    Ok(Change::Edit {
        expected: edit.0,
        edit: edit.1,
    })
}

#[cfg(test)]
mod tests;
