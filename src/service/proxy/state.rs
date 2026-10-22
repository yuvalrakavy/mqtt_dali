//! Physical projections are published only on the BridgeKit worker thread.
use super::*;

pub(super) fn intent(
    values: &PropertyMap,
    representation: Option<crate::config::ColorRepresentation>,
) -> Result<output::WriteIntent, OpError> {
    if let Some(name) = values.keys().find(|name| {
        !matches!(name.as_str(), "PowerState" | "Brightness") && !super::colour::is_property(name)
    }) {
        return Err(OpError::Unsupported(format!(
            "unknown output property: {name}"
        )));
    }
    let power = values
        .get("PowerState")
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| OpError::Rejected("PowerState must be a JSON bool".into()))
        })
        .transpose()?;
    let brightness = values
        .get("Brightness")
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| OpError::Rejected("Brightness must be a JSON number".into()))
        })
        .transpose()?;
    let base = if power.is_some() || brightness.is_some() {
        Some(
            crate::dali::gear::Intent::new(power, brightness)
                .map_err(|error| OpError::Rejected(error.to_string()))?,
        )
    } else {
        None
    };
    match (base, super::colour::intent(values, representation)?) {
        (Some(base), Some(colour)) => Ok(output::WriteIntent::Combined(base, colour)),
        (Some(base), None) => Ok(output::WriteIntent::Base(base)),
        (None, Some(colour)) => Ok(output::WriteIntent::Colour(colour)),
        (None, None) => Err(OpError::Rejected(
            "at least one output property is required".into(),
        )),
    }
}

fn lifecycle(snapshot: &Snapshot, bus: u8) -> bool {
    matches!(snapshot.phase, Phase::Ready | Phase::Writing)
        && snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|value| value.bus == bus && value.enabled)
        && matches!(&snapshot.hardware, Hardware::Ready { buses, .. }
            if buses.iter().any(|value| value.bus == bus && value.status == 2))
}

pub(super) fn projections(snapshot: &Snapshot) -> Vec<ContextualState> {
    let table = routing::generations(&snapshot.model);
    snapshot
        .model
        .projections()
        .into_iter()
        .filter_map(|projection| {
            let crate::inventory::Location::Output { bus, .. } = projection.location else {
                return None;
            };
            let token = &table.targets[&projection.target];
            let observation = snapshot
                .outputs
                .get(&projection.target)
                .filter(|value| value.context.generation() == Some(token.as_str()));
            let quality = match observation {
                None => "unobserved",
                Some(_) if !lifecycle(snapshot, bus) => "stale",
                Some(value) => match value.quality {
                    output::Quality::Current => "current",
                    output::Quality::Partial => "partial",
                    output::Quality::Unavailable => "unavailable",
                    output::Quality::Stale => "stale",
                },
            };
            // Library available() counts answered MASK evidence; wire availability
            // additionally requires usable current power/brightness values.
            let controls = observation
                .filter(|_| quality == "current")
                .and_then(|value| Some((value.power()?, value.brightness()?)));
            let mut values = PropertyMap::from_iter([
                ("Available".into(), Value::Bool(controls.is_some())),
                ("Quality".into(), Value::String(quality.into())),
            ]);
            if let Some((power, brightness)) = controls {
                values.insert("PowerState".into(), Value::Bool(power));
                values.insert("Brightness".into(), Value::from(brightness));
            }
            super::colour::project(
                &mut values,
                projection.color_representation,
                observation,
                lifecycle(snapshot, bus),
            );
            let context = match observation.filter(|_| quality != "stale") {
                Some(value) => value.context.clone(),
                None => RequestContext::new(token.clone())
                    .expect("validated target token")
                    .with_traceparent(None),
            };
            Some(ContextualState::new(
                TargetState::new(projection.target, values),
                context,
            ))
        })
        .chain(super::super::input::projections(snapshot))
        .collect()
}

pub(super) fn accepted(
    snapshot: &Snapshot,
    target: &str,
    observation: &output::Observation,
) -> Result<ContextualState, OpError> {
    let projection = projections(snapshot)
        .into_iter()
        .find(|value| value.state.target == target);
    accept_projected(snapshot, target, observation, projection)
}

pub(super) fn accept_projected(
    snapshot: &Snapshot,
    target: &str,
    observation: &output::Observation,
    projection: Option<ContextualState>,
) -> Result<ContextualState, OpError> {
    if !lifecycle(snapshot, observation.bus)
        || observation.quality == output::Quality::Stale
        || snapshot.outputs.get(target) != Some(observation)
    {
        return Err(OpError::Rejected(
            "output evidence changed before MQTT completion".into(),
        ));
    }
    let mut state = projection
        .filter(|value| value.context.generation() == observation.context.generation())
        .ok_or_else(|| OpError::Rejected("output mapping changed before MQTT completion".into()))?;
    state.context = observation.context.clone();
    Ok(state)
}

pub(super) struct Watcher {
    snapshots: watch::Receiver<Option<Snapshot>>,
    report: Reporter,
    #[cfg(test)]
    pause: Option<std::sync::Arc<super::super::tests::Pause>>,
}
impl Watcher {
    pub(super) fn new(snapshots: watch::Receiver<Option<Snapshot>>, report: Reporter) -> Self {
        Self {
            snapshots,
            report,
            #[cfg(test)]
            pause: None,
        }
    }
    #[cfg(test)]
    pub(super) fn with_pause(
        mut self,
        pause: Option<std::sync::Arc<super::super::tests::Pause>>,
    ) -> Self {
        self.pause = pause;
        self
    }
    pub(super) async fn run(mut self) {
        loop {
            let snapshot = self.snapshots.borrow_and_update().clone();
            if let Some(snapshot) = snapshot {
                for state in projections(&snapshot) {
                    // The driver also reports on this worker. Only the SDK owns
                    // the publication cache; a helper-local skip cache can miss
                    // a physical return after a newer driver report.
                    self.report.state_with_context(
                        &state.state.target,
                        state.state.values,
                        false,
                        &state.context,
                    );
                }
                if matches!(snapshot.phase, Phase::Stopping | Phase::Closed) {
                    return;
                }
            }
            #[cfg(test)]
            if let Some(pause) = &self.pause {
                pause.point().await;
            }
            // WAIT: v2-mqtt-observations
            if self.snapshots.changed().await.is_err() {
                return;
            }
        }
    }
}
