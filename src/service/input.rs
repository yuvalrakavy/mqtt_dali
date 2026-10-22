//! Observed native input state and events, separate from retained intent.
use super::{output::Stamp, routing, Hardware, Model, Phase, RequestContext, Snapshot};
use crate::dali::input::{self as protocol, ButtonAction, InstanceType, Payload, Source};
use crate::inventory::{Location, Projection};
use mqtt_bridge_kit::{ContextualState, PropertyMap, TargetState};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub const EVENT_CAPACITY: usize = 64;
pub mod readback;

#[derive(Debug, Clone, PartialEq)]
pub enum Evidence {
    Event {
        frame: protocol::Event,
        payload: Payload,
    },
    Query(readback::ReadValue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub context: RequestContext,
    pub evidence: Evidence,
    pub(super) bus: u8,
    pub(super) stamp: Stamp,
    pub(super) stale: bool,
}
#[derive(Debug, Clone)]
pub struct Notice {
    pub target: String,
    pub context: RequestContext,
    pub instance_type: u8,
    pub frame: protocol::Event,
    pub payload: Payload,
    // Private receive lifetime; unchanged item identity does not authorize replay
    // across a bus disable/re-enable or transport receive boundary.
    stamp: Stamp,
    // A continuing saved identity can undergo a native address repair without
    // changing its token/stamp. That repair permanently revokes older actions.
    valid: Arc<AtomicBool>,
}
impl Notice {
    pub(crate) fn current(&self, snapshot: &Snapshot) -> bool {
        self.valid.load(Ordering::SeqCst)
            && matches!(snapshot.phase, Phase::Ready | Phase::Writing)
            && snapshot.input_listeners.contains(&self.target)
            && routing::generations(&snapshot.model)
                .targets
                .get(&self.target)
                .map(String::as_str)
                == self.context.generation()
            && snapshot
                .inputs
                .get(&self.target)
                .is_some_and(|observation| {
                    !observation.stale
                        && observation.stamp == self.stamp
                        && snapshot
                            .model
                            .inventory()
                            .buses
                            .iter()
                            .any(|bus| bus.bus == observation.bus && bus.enabled)
                })
    }
    pub fn value(&self) -> Value {
        let mut value = json!({"kind":"dali_input","target":self.target,
            "instance_type":self.instance_type,"frame":self.frame.raw(),
            "information":self.frame.information()});
        if let Payload::Button(action) = self.payload {
            value["action"] = Value::from(match action {
                ButtonAction::Released => "released",
                ButtonAction::Pressed => "pressed",
                ButtonAction::ShortPress => "short_press",
                ButtonAction::DoublePress => "double_press",
                ButtonAction::LongPressStart => "long_press_start",
                ButtonAction::LongPressRepeat => "long_press_repeat",
                ButtonAction::LongPressStop => "long_press_stop",
                ButtonAction::Free => "free",
                ButtonAction::Stuck => "stuck",
            });
        }
        match self.payload {
            Payload::Absolute { raw_10 } => value["position_raw"] = Value::from(raw_10),
            Payload::Light { raw_10 } => value["light_level_raw"] = Value::from(raw_10),
            Payload::Occupancy {
                occupied,
                movement,
                repeated,
                sensor,
            } => {
                value["occupied"] = Value::Bool(occupied);
                value["movement"] = Value::Bool(movement);
                value["repeated"] = Value::Bool(repeated);
                value["sensor"] = Value::from(match sensor {
                    protocol::SensorKind::Presence => "presence",
                    protocol::SensorKind::Movement => "movement",
                });
            }
            _ => {}
        }
        value
    }
}
#[derive(Clone)]
struct Route {
    projection: Projection,
    context: RequestContext,
    floor: Option<(Stamp, u64)>,
    native_sequence: u64,
    notice_valid: Arc<AtomicBool>,
}
#[derive(Clone)]
pub(super) struct Cut {
    pub target: String,
    pub context: RequestContext,
    pub stamp: Stamp,
}
#[derive(Default)]
pub(super) struct Router {
    model: Option<(String, u64)>,
    routes: BTreeMap<String, Route>,
    pub observations: BTreeMap<String, Observation>,
    notices: VecDeque<Notice>,
    dropped: u64,
}
impl Router {
    pub fn sync(&mut self, model: &Model) -> bool {
        let key = (
            model.inventory().inventory_epoch.clone(),
            model.inventory().revision,
        );
        if self.model.as_ref() == Some(&key) {
            return false;
        }
        let table = routing::generations(model);
        let mut previous = std::mem::take(&mut self.routes);
        for projection in model.projections() {
            let Location::InputInstance { bus, .. } = projection.location else {
                continue;
            };
            if !model
                .inventory()
                .buses
                .iter()
                .any(|b| b.bus == bus && b.enabled)
            {
                continue;
            }
            let context = RequestContext::new(table.targets[&projection.target].clone())
                .expect("validated input generation")
                .with_traceparent(None);
            let route = previous
                .remove(&projection.target)
                .filter(|r| r.projection == projection)
                .unwrap_or_else(|| Route {
                    projection: projection.clone(),
                    context,
                    floor: None,
                    native_sequence: 0,
                    notice_valid: Arc::new(AtomicBool::new(true)),
                });
            self.routes.insert(projection.target, route);
        }
        self.observations.retain(|target, observation| {
            self.routes
                .get(target)
                .is_some_and(|route| route.context.generation() == observation.context.generation())
        });
        self.model = Some(key);
        true
    }
    pub fn pending(&self, ready: impl Fn(u8) -> Option<Stamp>) -> Vec<Cut> {
        self.routes
            .values()
            .filter_map(|route| {
                let Location::InputInstance { bus, .. } = route.projection.location else {
                    unreachable!()
                };
                let stamp = ready(bus)?;
                route
                    .floor
                    .is_none_or(|(old, _)| old != stamp)
                    .then(|| Cut {
                        target: route.projection.target.clone(),
                        context: route.context.clone(),
                        stamp,
                    })
            })
            .collect()
    }
    pub fn listeners(
        &self,
        ready: impl Fn(u8) -> Option<Stamp>,
    ) -> std::collections::BTreeSet<String> {
        self.routes
            .values()
            .filter_map(|r| {
                let Location::InputInstance { bus, .. } = r.projection.location else {
                    unreachable!()
                };
                r.floor
                    .is_some_and(|(stamp, _)| ready(bus) == Some(stamp))
                    .then(|| r.projection.target.clone())
            })
            .collect()
    }
    pub fn install(&mut self, cuts: &[Cut], minimum: u64, ready: impl Fn(u8) -> Option<Stamp>) {
        for cut in cuts {
            if let Some(route) = self.routes.get_mut(&cut.target) {
                let Location::InputInstance { bus, .. } = route.projection.location else {
                    unreachable!()
                };
                if route.context.generation() == cut.context.generation()
                    && ready(bus) == Some(cut.stamp)
                {
                    if route.floor.is_none_or(|(stamp, _)| stamp != cut.stamp) {
                        route.native_sequence = 0;
                    }
                    // A redundant old completion must not move an installed floor backwards.
                    route.floor = Some((
                        cut.stamp,
                        route
                            .floor
                            .filter(|(stamp, _)| *stamp == cut.stamp)
                            .map_or(minimum, |(_, old)| old.max(minimum)),
                    ));
                }
            }
        }
    }
    pub fn observe(&mut self, bus: u8, sequence: u64, raw: u32, stamp: Option<Stamp>) {
        let Some(stamp) = stamp else {
            return;
        };
        let Ok(Some(event)) = protocol::decode(raw) else {
            return;
        };
        let mut candidates = self.routes.values().filter(|route| {
            let Location::InputInstance {
                bus: route_bus,
                short_address,
                instance,
            } = route.projection.location
            else {
                return false;
            };
            route_bus == bus
                && match event.source() {
                    Source::DeviceInstance {
                        address,
                        instance: source,
                    } => address.raw() == short_address && source.raw() == instance,
                    Source::Device { address, kind } => {
                        address.raw() == short_address
                            && Some(kind.raw()) == route.projection.instance_type
                    }
                    _ => false,
                }
        });
        let Some(route) = candidates.next() else {
            return;
        };
        if candidates.next().is_some()
            || !route
                .floor
                .is_some_and(|(floor, min)| floor == stamp && sequence >= min)
        {
            return;
        }
        let kind = route
            .projection
            .instance_type
            .expect("validated input instance type");
        let Ok(payload) = event.payload(Some(InstanceType::from_raw(kind))) else {
            return;
        };
        let target = route.projection.target.clone();
        let context = route.context.clone();
        let valid = route.notice_valid.clone();
        self.observations.insert(
            target.clone(),
            Observation {
                context: context.clone(),
                evidence: Evidence::Event {
                    frame: event,
                    payload,
                },
                bus,
                stamp,
                stale: false,
            },
        );
        self.routes.get_mut(&target).unwrap().native_sequence = sequence;
        if self.notices.len() >= crate::atx::monitor::EVENT_CAPACITY {
            self.notices.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.notices.push_back(Notice {
            target,
            context,
            instance_type: kind,
            frame: event,
            payload,
            stamp,
            valid,
        });
    }
    /// Reuse normal receive-boundary installation after an address repair.
    /// Queued old Notice clones stay revoked when fresh observations resume.
    pub fn cut_for_restore(&mut self, bus: u8, address: u8) {
        for (target, route) in &mut self.routes {
            if matches!(route.projection.location,Location::InputInstance{bus:route_bus,short_address,..} if route_bus==bus&&short_address==address)
            {
                route.notice_valid.store(false, Ordering::SeqCst);
                route.notice_valid = Arc::new(AtomicBool::new(true));
                route.floor = None;
                route.native_sequence = 0;
                if let Some(observation) = self.observations.get_mut(target) {
                    observation.stale = true;
                }
            }
        }
    }
    pub fn invalidate(&mut self, ready: impl Fn(u8) -> Option<Stamp>) {
        for observation in self.observations.values_mut() {
            if ready(observation.bus) != Some(observation.stamp) {
                observation.stale = true;
            }
        }
    }
    pub fn native_sequence(&self, target: &str) -> u64 {
        self.routes
            .get(target)
            .map_or(0, |route| route.native_sequence)
    }
    pub fn accept_query(&mut self, target: &str, sequence: u64, observation: &mut Observation) {
        let Some(route) = self
            .routes
            .get(target)
            .filter(|r| r.context.generation() == observation.context.generation())
        else {
            return;
        };
        if route.native_sequence > sequence {
            if let Some(native) = self.observations.get(target) {
                *observation = native.clone();
            }
        } else {
            self.observations.insert(target.into(), observation.clone());
        }
    }
    pub fn take_notices(&mut self) -> (Vec<Notice>, u64) {
        (
            self.notices.drain(..).collect(),
            std::mem::take(&mut self.dropped),
        )
    }
}

pub(super) fn retained(
    model: &Model,
    mut observations: BTreeMap<String, Observation>,
) -> BTreeMap<String, Observation> {
    let table = routing::generations(model);
    observations.retain(|target, value| {
        table.targets.get(target).map(String::as_str) == value.context.generation()
            && model
                .inventory()
                .buses
                .iter()
                .any(|bus| bus.bus == value.bus && bus.enabled)
    });
    observations
}

pub(super) fn retained_listeners(
    model: &Model,
    old: &Snapshot,
) -> std::collections::BTreeSet<String> {
    let before = routing::generations(&old.model);
    let after = routing::generations(model);
    old.input_listeners
        .iter()
        .filter(|target| {
            after.targets.contains_key(*target)
                && before.targets.get(*target) == after.targets.get(*target)
                && model.projections().iter().any(|p| {
                    p.target == target.as_str()
                        && matches!(p.location, Location::InputInstance {bus,..}
                        if model.inventory().buses.iter().any(|b| b.bus == bus && b.enabled))
                })
        })
        .cloned()
        .collect()
}

pub(super) fn projections(snapshot: &Snapshot) -> Vec<ContextualState> {
    let table = routing::generations(&snapshot.model);
    let mut states = Vec::new();
    for projection in snapshot.model.projections() {
        let Location::InputInstance { bus, .. } = projection.location else {
            continue;
        };
        let token = &table.targets[&projection.target];
        let observation = snapshot
            .inputs
            .get(&projection.target)
            .filter(|o| o.context.generation() == Some(token.as_str()));
        let live = matches!(snapshot.phase, Phase::Ready | Phase::Writing)
            && snapshot
                .model
                .inventory()
                .buses
                .iter()
                .any(|b| b.bus == bus && b.enabled)
            && matches!(&snapshot.hardware,Hardware::Ready {buses,..} if buses.iter().any(|b|b.bus==bus && b.status==2));
        let quality = match observation {
            None => "unobserved",
            Some(o) if o.stale || !live => "stale",
            Some(o) => match &o.evidence {
                Evidence::Query(value) => value.quality.label(),
                Evidence::Event { payload, .. } => match payload {
                    Payload::Unknown { .. } | Payload::Unresolved => "unsupported",
                    Payload::Button(ButtonAction::Pressed | ButtonAction::Released) => "current",
                    Payload::Button(_) => "partial",
                    _ => "current",
                },
            },
        };
        let current = observation.filter(|_| !matches!(quality, "stale" | "unobserved"));
        let mut values = PropertyMap::from_iter([
            (
                "Available".into(),
                Value::Bool(
                    quality == "current"
                        || quality == "partial"
                            && matches!(
                                observation.map(|o| &o.evidence),
                                Some(Evidence::Event { .. })
                            ),
                ),
            ),
            ("Quality".into(), Value::from(quality)),
            (
                "EventInformation".into(),
                current.map_or(Value::Null, |o| match o.evidence {
                    Evidence::Event { frame, .. } => Value::from(frame.information()),
                    _ => Value::Null,
                }),
            ),
        ]);
        let payload = current.and_then(|o| match o.evidence {
            Evidence::Event { payload, .. } => Some(payload),
            _ => None,
        });
        match projection.instance_type.unwrap() {
            1 => {
                values.insert(
                    "Status".into(),
                    match payload {
                        Some(Payload::Button(ButtonAction::Pressed)) => Value::Bool(true),
                        Some(Payload::Button(ButtonAction::Released)) => Value::Bool(false),
                        _ => Value::Null,
                    },
                );
            }
            2 => {
                values.insert(
                    "PositionRaw".into(),
                    match payload {
                        Some(Payload::Absolute { raw_10 }) => Value::from(raw_10),
                        _ => Value::Null,
                    },
                );
            }
            3 => {
                let (occupied, movement) = match payload {
                    Some(Payload::Occupancy {
                        occupied, movement, ..
                    }) => (Value::Bool(occupied), Value::Bool(movement)),
                    _ => (Value::Null, Value::Null),
                };
                values.insert("Occupied".into(), occupied);
                values.insert("Movement".into(), movement);
            }
            4 => {
                values.insert(
                    "LightLevelRaw".into(),
                    match payload {
                        Some(Payload::Light { raw_10 }) => Value::from(raw_10),
                        _ => Value::Null,
                    },
                );
            }
            _ => {}
        }
        project_query(&mut values, projection.instance_type.unwrap(), current);
        let context = current.map_or_else(
            || {
                RequestContext::new(token.clone())
                    .expect("validated input generation")
                    .with_traceparent(None)
            },
            |o| o.context.clone(),
        );
        states.push(ContextualState::new(
            TargetState::new(projection.target, values),
            context,
        ));
    }
    states
}

fn project_query(values: &mut PropertyMap, kind: u8, observation: Option<&Observation>) {
    let query = observation.and_then(|o| match &o.evidence {
        Evidence::Query(q) => Some(q),
        _ => None,
    });
    let value = query.and_then(|q| q.value.as_ref());
    values.insert(
        "ValueSource".into(),
        observation.map_or(Value::Null, |o| {
            Value::from(match o.evidence {
                Evidence::Event { .. } => "event",
                Evidence::Query(_) => "query",
            })
        }),
    );
    values.insert(
        "ValueResolution".into(),
        query
            .and_then(|q| q.resolution)
            .map_or(Value::Null, |r| Value::from(r.bits())),
    );
    values.insert(
        "InputValueRaw".into(),
        value.map_or(Value::Null, |v| Value::from(v.hex())),
    );
    if kind == 2 {
        values.insert("Position".into(), Value::Null);
        values.insert("InputState".into(), Value::Null);
    }
    if !query.is_some_and(|q| q.quality == readback::Quality::Current) {
        return;
    }
    let Some(value) = value else {
        return;
    };
    match kind {
        1 => {
            values.insert("Status".into(), Value::Bool(value.as_u64() == Some(1)));
        }
        2 => {
            values.insert("Position".into(), Value::from(value.normalized()));
            if value.resolution().bits() == 1 {
                values.insert("InputState".into(), Value::Bool(value.as_u64() == Some(1)));
            }
        }
        3 => {
            let flags = value.as_u64().expect("current two-bit occupancy value");
            values.insert("Occupied".into(), Value::Bool(flags & 2 != 0));
            values.insert("Movement".into(), Value::Bool(flags & 1 != 0));
        }
        _ => {}
    }
}

pub(super) async fn events(
    handle: super::ServiceHandle,
    mut receive: tokio::sync::broadcast::Receiver<Notice>,
    report: mqtt_bridge_kit::Reporter,
) {
    loop {
        // WAIT: v2-input-events
        match receive.recv().await {
            Ok(notice) => {
                #[cfg(test)]
                // WAIT: v2-input-events
                handle.input_event_pause.point().await;
                if handle
                    .snapshot()
                    .is_some_and(|snapshot| notice.current(&snapshot))
                {
                    report.event_with_context(notice.value(), &notice.context);
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                tracing::warn!(
                    kind = "input_event_loss",
                    dropped,
                    "Native input events were lost; State cannot replay actions"
                );
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

#[cfg(test)]
mod tests;
