//! Accepted indicator evidence, independent from input measurements and notices.
use super::*;
use crate::service::{Hardware, Model};
use mqtt_bridge_kit::{ContextualState, PropertyMap, TargetState};
use serde_json::Value;
use std::collections::BTreeMap;

/// The last owner-accepted family/cause for the aggregate input target State.
#[derive(Debug, Clone)]
pub struct StateContext {
    context: RequestContext,
    feedback: bool,
}

pub(in crate::service) fn retained(
    model: &Model,
    mut cache: BTreeMap<String, Arc<Observation>>,
) -> BTreeMap<String, Arc<Observation>> {
    let table = routing::generations(model);
    cache.retain(|target, o| {
        if o.report.revision != model.inventory().revision {
            Arc::make_mut(o).stale = true;
        }
        table.targets.get(target).map(String::as_str) == o.context.generation()
            && model
                .inventory()
                .buses
                .iter()
                .any(|b| b.bus == o.report.bus && b.enabled)
    });
    cache
}
pub(in crate::service) fn retained_contexts(
    model: &Model,
    mut contexts: BTreeMap<String, StateContext>,
) -> BTreeMap<String, StateContext> {
    let table = routing::generations(model);
    contexts.retain(|target, c| {
        table.targets.get(target).map(String::as_str) == c.context.generation()
    });
    contexts
}
pub(in crate::service) fn retire(snapshot: &mut Snapshot, target: &str) {
    if let Some(o) = snapshot.feedbacks.get_mut(target).filter(|o| !o.stale) {
        Arc::make_mut(o).stale = true;
    }
}
pub(in crate::service) fn invalidate(
    snapshot: &mut Snapshot,
    hat: &crate::service::hat::Hat,
) -> bool {
    let mut changed = false;
    for (target, o) in &mut snapshot.feedbacks {
        if !o.stale
            && (o.report.revision != snapshot.model.inventory().revision
                || !hat.cached_evidence_current(o.report.bus, o.stamp)
                || hat.input_feedback_changing(o.report.bus, o.report.address, target))
        {
            Arc::make_mut(o).stale = true;
            changed = true;
        }
    }
    changed
}
pub(in crate::service) fn live(snapshot: &Snapshot, bus: u8) -> bool {
    matches!(snapshot.phase, Phase::Ready | Phase::Writing)
        && snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|b| b.bus == bus && b.enabled)
        && matches!(&snapshot.hardware, Hardware::Ready { buses, .. } if buses.iter().any(|b| b.bus == bus && b.status == 2))
}
fn current<'a>(snapshot: &'a Snapshot, target: &str, token: &str) -> Option<&'a Arc<Observation>> {
    let o = snapshot.feedbacks.get(target)?;
    (!o.stale
        && live(snapshot, o.report.bus)
        && o.report.revision == snapshot.model.inventory().revision
        && Some(token) == o.context.generation())
    .then_some(o)
}
pub(in crate::service) fn note_input(
    snapshot: &mut Snapshot,
    target: &str,
    cause: &RequestContext,
) {
    if snapshot.inputs.get(target).is_some_and(|o| {
        !o.stale && live(snapshot, o.bus) && o.context.generation() == cause.generation()
    }) {
        snapshot.input_contexts.insert(
            target.into(),
            StateContext {
                context: cause.clone(),
                feedback: false,
            },
        );
    }
}
pub(in crate::service) fn note_feedback(
    snapshot: &mut Snapshot,
    target: &str,
    o: Arc<Observation>,
) {
    snapshot.input_contexts.insert(
        target.into(),
        StateContext {
            context: o.context.clone(),
            feedback: true,
        },
    );
    snapshot.feedbacks.insert(target.into(), o);
}
pub(in crate::service) fn input_changes(
    snapshot: &mut Snapshot,
    previous: &BTreeMap<String, crate::service::input::Observation>,
) {
    let changed: Vec<_> = snapshot
        .inputs
        .iter()
        .filter(|(t, o)| {
            previous
                .get(*t)
                .is_none_or(|old| old != *o || old.context.traceparent() != o.context.traceparent())
        })
        .map(|(t, o)| (t.clone(), o.context.clone()))
        .collect();
    for (target, cause) in changed {
        note_input(snapshot, &target, &cause);
    }
}
pub(in crate::service) fn aggregate_context(snapshot: &Snapshot, target: &str) -> RequestContext {
    let table = routing::generations(&snapshot.model);
    context_with(snapshot, target, &table.targets[target])
}
fn context_with(snapshot: &Snapshot, target: &str, token: &str) -> RequestContext {
    let feedback = current(snapshot, target, token).map(|o| &o.context);
    let input = snapshot
        .inputs
        .get(target)
        .filter(|o| !o.stale && live(snapshot, o.bus) && o.context.generation() == Some(token))
        .map(|o| &o.context);
    if let Some(c) = snapshot.input_contexts.get(target) {
        if c.context.generation() == Some(token)
            && ((c.feedback && feedback.is_some()) || (!c.feedback && input.is_some()))
        {
            return c.context.clone();
        }
    }
    feedback.or(input).cloned().unwrap_or_else(|| {
        RequestContext::new(token.to_owned())
            .expect("validated input token")
            .with_traceparent(None)
    })
}
pub(in crate::service) fn projections(snapshot: &Snapshot) -> Vec<ContextualState> {
    let table = routing::generations(&snapshot.model);
    snapshot
        .model
        .projections()
        .into_iter()
        .filter_map(|p| {
            if !matches!(
                p.location,
                Location::Input { .. } | Location::InputInstance { .. }
            ) {
                return None;
            }
            let observed = snapshot.feedbacks.get(&p.target);
            let fresh = current(snapshot, &p.target, &table.targets[&p.target]);
            let values = fresh.and_then(|o| o.report.feedback.as_ref());
            let quality = if observed.is_some() && fresh.is_none() {
                "stale"
            } else if let Some(o) = fresh {
                if o.report.outcome == "type_mismatch" {
                    "type_mismatch"
                } else if values
                    .is_some_and(|v| v.quality == "complete" && v.supported == Some(false))
                {
                    "unsupported"
                } else if values.is_some_and(Values::complete_supported) {
                    "current"
                } else if values.is_some() {
                    "partial"
                } else {
                    "unavailable"
                }
            } else {
                "unobserved"
            };
            let map = PropertyMap::from_iter([
                (
                    "FeedbackActive".into(),
                    values
                        .filter(|_| quality == "current")
                        .and_then(|v| v.active)
                        .map_or(Value::Null, Value::from),
                ),
                (
                    "FeedbackAvailable".into(),
                    Value::Bool(quality == "current"),
                ),
                (
                    "FeedbackSupported".into(),
                    values
                        .and_then(|v| v.supported)
                        .map_or(Value::Null, Value::from),
                ),
                (
                    "FeedbackCapabilityRaw".into(),
                    values
                        .and_then(|v| v.capability)
                        .map_or(Value::Null, Value::from),
                ),
                ("FeedbackQuality".into(), Value::from(quality)),
            ]);
            Some(ContextualState::new(
                TargetState::new(p.target.clone(), map),
                context_with(snapshot, &p.target, &table.targets[&p.target]),
            ))
        })
        .collect()
}
