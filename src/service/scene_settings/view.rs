//! Saved intent from one immutable snapshot, independent of hardware health.
use super::*;

#[derive(Serialize)]
pub(in crate::service) struct View {
    target: String,
    target_generation: String,
    revision: u64,
    bus: u8,
    slot: u8,
    fixtures: Vec<Fixture>,
}
#[derive(Serialize)]
struct Fixture {
    fixture: String,
    fixture_generation: String,
    saved: Option<Setting>,
}
pub(in crate::service) fn read(
    snapshot: &Snapshot,
    target: &str,
    context: &RequestContext,
) -> Result<View, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    let projections = snapshot.model.projections();
    let scene = projections
        .iter()
        .find(|p| p.target == target)
        .ok_or_else(|| ServiceError::Rejected("unknown scene".into()))?;
    let Location::Scene { bus, slot } = scene.location else {
        return Err(ServiceError::Rejected("named scene required".into()));
    };
    let table = routing::generations(&snapshot.model);
    let generation = &table.targets[target];
    if context
        .generation()
        .is_some_and(|supplied| supplied != generation)
    {
        return Err(ServiceError::Rejected("retired target generation".into()));
    }
    let fixtures = projections
        .iter()
        .filter_map(|p| {
            let Location::Output {
                bus: output_bus,
                short_address,
            } = p.location
            else {
                return None;
            };
            (output_bus == bus).then(|| Fixture {
                fixture: p.target.clone(),
                fixture_generation: table.targets[&p.target].clone(),
                saved: settings_value::saved(&snapshot.model, bus, slot, short_address),
            })
        })
        .collect();
    Ok(View {
        target: target.into(),
        target_generation: generation.clone(),
        revision: snapshot.model.inventory().revision,
        bus,
        slot,
        fixtures,
    })
}
