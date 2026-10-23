//! Base group scalar adaptation; native execution and evidence stay with its owner.
use super::*;
use crate::dali::gear::Intent;

pub(super) fn is_group(snapshot: &Snapshot, target: &str) -> bool {
    snapshot.model.projections().iter().any(|p| {
        p.target == target && matches!(p.location, crate::inventory::Location::Group { .. })
    })
}
pub(super) fn intent(values: &PropertyMap) -> Result<Intent, OpError> {
    match state::intent(values, None)? {
        output::WriteIntent::Base(intent) => Ok(intent),
        _ => Err(OpError::Unsupported(
            "group colour properties are not supported".into(),
        )),
    }
}
pub(super) fn route(
    snapshot: &Snapshot,
    target: &str,
    context: &RequestContext,
    writing: bool,
) -> Result<(), OpError> {
    if writing && context.generation().is_none() {
        return Err(OpError::Rejected(
            "original group generation required".into(),
        ));
    }
    group_output::resolve(snapshot, target, &group_output::Action::Read, context)
        .map(|_| ())
        .map_err(op_error)
}
fn observation(reply: group_output::Reply) -> Result<group_output::Observation, OpError> {
    match reply {
        group_output::Reply::Observed(o) => Ok(o),
        group_output::Reply::Unconfirmed(_) => Err(OpError::Rejected(
            "group write is unconfirmed; inspect actual state before retrying".into(),
        )),
    }
}
pub(super) async fn read(
    proxy: &DaliProxy,
    target: &str,
    context: RequestContext,
    deadline: Instant,
) -> Result<group_output::Observation, OpError> {
    route(&proxy.snapshot()?, target, &context, false)?;
    let reply = proxy
        .handle
        .group_evidence(target, group_output::Action::Read, context, deadline, None)
        .await
        .map_err(op_error)?;
    #[cfg(test)]
    if let Some(pause) = &proxy.read_pause {
        pause.point().await;
    }
    if Instant::now() >= deadline {
        return Err(OpError::Rejected(
            "group read completion exceeded its original deadline".into(),
        ));
    }
    observation(reply)
}
pub(super) async fn apply(
    proxy: &DaliProxy,
    snapshot: &Snapshot,
    target: &str,
    values: &PropertyMap,
    context: &RequestContext,
) -> Result<(), OpError> {
    route(snapshot, target, context, true)?;
    let intent = intent(values)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let observed = observation(
        proxy
            .handle
            .group_evidence(
                target,
                group_output::Action::Set {
                    expected: snapshot.model.inventory().revision,
                    intent,
                },
                context.clone(),
                deadline,
                None,
            )
            .await
            .map_err(op_error)?,
    )?;
    let state = accepted(&proxy.snapshot()?, target, &observed, deadline)?;
    let current = state.state.values.get("Available") == Some(&Value::Bool(true));
    proxy
        .reporter
        .as_ref()
        .ok_or_else(|| OpError::Link(LinkError::new("proxy is disconnected")))?
        .state_with_context(target, state.state.values, false, &state.context);
    if !current {
        return Err(OpError::Rejected(
            "post-write group evidence is incomplete or empty; inspect state before retrying"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn projections(snapshot: &Snapshot) -> Vec<ContextualState> {
    let table = routing::generations(&snapshot.model);
    snapshot
        .model
        .projections()
        .into_iter()
        .filter_map(|p| {
            let crate::inventory::Location::Group { bus, .. } = p.location else {
                return None;
            };
            let token = &table.targets[&p.target];
            let observation = snapshot
                .group_outputs
                .get(&p.target)
                .filter(|o| o.context.generation() == Some(token.as_str()));
            let stale = observation.is_some_and(|o| {
                o.stale
                    || o.report().revision != snapshot.model.inventory().revision
                    || !state::lifecycle(snapshot, bus)
            });
            let fresh = observation.filter(|_| !stale);
            let sample = fresh.and_then(|o| o.report().values.as_ref());
            let quality = if stale {
                "stale"
            } else {
                sample.map_or("unobserved", |v| match v.quality {
                    "uniform" | "mixed" => "current",
                    q => q,
                })
            };
            let available = quality == "current";
            let control = sample.filter(|_| available);
            let values = PropertyMap::from_iter([
                ("Available".into(), Value::Bool(available)),
                ("Quality".into(), Value::from(quality)),
                ("Coverage".into(), Value::from("saved_inventory")),
                (
                    "PowerState".into(),
                    control
                        .and_then(|v| v.power_state)
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                ),
                (
                    "Brightness".into(),
                    control
                        .and_then(|v| v.brightness)
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                ),
                (
                    "PowerMixed".into(),
                    control
                        .and_then(|v| v.power_mixed)
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                ),
                (
                    "BrightnessMixed".into(),
                    control
                        .and_then(|v| v.brightness_mixed)
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                ),
                (
                    "KnownMembers".into(),
                    sample
                        .map(|v| Value::from(v.known_members))
                        .unwrap_or(Value::Null),
                ),
                (
                    "MembershipMatches".into(),
                    fresh
                        .and_then(|o| o.report().membership_matches)
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                ),
            ]);
            let context = fresh.map(|o| o.context.clone()).unwrap_or_else(|| {
                RequestContext::new(token.clone())
                    .expect("validated group generation")
                    .with_traceparent(None)
            });
            Some(ContextualState::new(
                TargetState::new(p.target, values),
                context,
            ))
        })
        .collect()
}
pub(super) fn accepted(
    snapshot: &Snapshot,
    target: &str,
    observed: &group_output::Observation,
    deadline: Instant,
) -> Result<ContextualState, OpError> {
    if Instant::now() >= deadline
        || observed.stale
        || !state::lifecycle(snapshot, observed.report().bus)
        || observed.report().revision != snapshot.model.inventory().revision
        || snapshot.group_outputs.get(target) != Some(observed)
    {
        return Err(OpError::Rejected(
            "group evidence changed before MQTT completion".into(),
        ));
    }
    let mut state = projections(snapshot)
        .into_iter()
        .find(|s| {
            s.state.target == target && s.context.generation() == observed.context.generation()
        })
        .ok_or_else(|| OpError::Rejected("group mapping changed before MQTT completion".into()))?;
    state.context = observed.context.clone();
    Ok(state)
}
