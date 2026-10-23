//! Scalar indicator bindings share the native feedback owner and final evidence.
use super::*;
use crate::inventory::Location;
use crate::service::input::feedback::{self as native, cache};
use std::sync::Arc;

pub(super) fn is_input(snapshot: &Snapshot, target: &str) -> bool {
    snapshot.model.projections().iter().any(|p| {
        p.target == target
            && matches!(
                p.location,
                Location::Input { .. } | Location::InputInstance { .. }
            )
    })
}
pub(super) fn intent(values: &PropertyMap) -> Result<bool, OpError> {
    if values.len() != 1 || !values.contains_key("FeedbackActive") {
        return Err(OpError::Rejected(
            "input writes require only FeedbackActive".into(),
        ));
    }
    values["FeedbackActive"]
        .as_bool()
        .ok_or_else(|| OpError::Rejected("FeedbackActive must be a JSON bool".into()))
}
pub(super) fn route(
    snapshot: &Snapshot,
    target: &str,
    cause: &RequestContext,
    writing: bool,
) -> Result<(), OpError> {
    let action = if writing {
        native::Action::Set {
            expected: snapshot.model.inventory().revision,
            active: false,
        }
    } else {
        native::Action::Read
    };
    native::resolve(snapshot, target, &action, cause)
        .map(|_| ())
        .map_err(op_error)
}
fn observation(reply: native::Reply) -> Result<Arc<native::Observation>, OpError> {
    match reply {
        native::Reply::Observed(o) => Ok(o),
        native::Reply::Unconfirmed(_) => Err(OpError::Rejected(
            "feedback write is unconfirmed; inspect actual state before retrying".into(),
        )),
    }
}
pub(super) async fn read(
    proxy: &DaliProxy,
    target: &str,
    context: RequestContext,
    deadline: Instant,
) -> Result<Arc<native::Observation>, OpError> {
    route(&proxy.snapshot()?, target, &context, false)?;
    let reply = proxy
        .handle
        .feedback_evidence(target, native::Action::Read, context, deadline, None)
        .await
        .map_err(op_error)?;
    #[cfg(test)]
    if let Some(pause) = &proxy.read_pause {
        pause.point().await;
    }
    if Instant::now() >= deadline {
        return Err(OpError::Rejected(
            "feedback read exceeded its original deadline".into(),
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
    let active = intent(values)?;
    let deadline = Instant::now() + Duration::from_secs(8);
    let observed = observation(
        proxy
            .handle
            .feedback_evidence(
                target,
                native::Action::Set {
                    expected: snapshot.model.inventory().revision,
                    active,
                },
                context.clone(),
                deadline,
                None,
            )
            .await
            .map_err(op_error)?,
    )?;
    let state = accepted(&proxy.snapshot()?, target, &observed, deadline)?;
    let verified = observed.report.outcome == "set"
        && state.state.values.get("FeedbackAvailable") == Some(&Value::Bool(true))
        && state.state.values.get("FeedbackActive") == Some(&Value::Bool(active));
    proxy
        .reporter
        .as_ref()
        .ok_or_else(|| OpError::Link(LinkError::new("proxy is disconnected")))?
        .state_with_context(target, state.state.values, false, &state.context);
    if !verified {
        return Err(OpError::Rejected("feedback actual state did not confirm the requested value; inspect state before retrying".into()));
    }
    Ok(())
}
pub(super) fn accepted(
    snapshot: &Snapshot,
    target: &str,
    observed: &Arc<native::Observation>,
    deadline: Instant,
) -> Result<ContextualState, OpError> {
    route(snapshot, target, &observed.context, false)?;
    if Instant::now() >= deadline
        || observed.stale
        || !cache::live(snapshot, observed.report.bus)
        || observed.report.revision != snapshot.model.inventory().revision
        || !snapshot
            .feedbacks
            .get(target)
            .is_some_and(|o| Arc::ptr_eq(o, observed))
    {
        return Err(OpError::Rejected(
            "feedback evidence changed before MQTT completion".into(),
        ));
    }
    let mut state = state::projections(snapshot)
        .into_iter()
        .find(|s| s.state.target == target)
        .ok_or_else(|| OpError::Rejected("input mapping changed before MQTT completion".into()))?;
    state.context = cache::aggregate_context(snapshot, target);
    Ok(state)
}

pub(super) async fn read_target(
    proxy: &DaliProxy,
    snapshot: &Snapshot,
    target: &str,
    context: &RequestContext,
) -> Result<ContextualState, OpError> {
    route(snapshot, target, context, false)?;
    let deadline = Instant::now() + Duration::from_secs(12);
    let instance = snapshot
        .model
        .projections()
        .iter()
        .any(|p| p.target == target && matches!(p.location, Location::InputInstance { .. }));
    let input = if instance {
        Some(
            proxy
                .handle
                .read_input(
                    target,
                    context.clone(),
                    deadline.min(Instant::now() + Duration::from_secs(4)),
                )
                .await
                .map_err(op_error)?,
        )
    } else {
        None
    };
    let feedback = read(
        proxy,
        target,
        context.clone(),
        deadline.min(Instant::now() + Duration::from_secs(8)),
    )
    .await?;
    let latest = proxy.snapshot()?;
    if let Some(input) = input {
        accepted_input(&latest, target, &input, deadline)?;
    }
    accepted(&latest, target, &feedback, deadline)
}
