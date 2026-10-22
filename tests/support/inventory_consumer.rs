//! Example consumer algorithm. Shares wire record types, never producer replay,
//! mutation or configuration validation. No Store or hardware dependency.
use mqtt_dali::inventory::{Ledger, Location, Origin, Projection};
use mqtt_dali::targets::{Namespace, Target};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const PENDING_ITEMS: usize = 64;
const PENDING_BYTES: usize = 64 * 1024;
const STATE_BYTES: usize = 8 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Acceptance {
    Applied,
    Unchanged,
    Ignored,
}

#[derive(Clone)]
pub struct Device {
    pub id: u64,
    pub record: Projection,
    pub intent: Option<Value>,
    pub observed: Option<Value>,
}
#[derive(Clone, Default)]
pub struct Consumer {
    pub devices: BTreeMap<Origin, Device>,
    pub retired: Vec<u64>,
    pub revision: u64,
    pub epoch: String,
    next_id: u64,
    document: Option<Value>,
    ledger: Option<Ledger>,
    pending: BTreeMap<(String, String), Value>,
    order: VecDeque<(String, String)>,
}
impl Consumer {
    /// State only: events must never pass through a retained observation cache.
    pub fn observe(
        &mut self,
        target: &str,
        generation: &str,
        payload: &[u8],
    ) -> Result<(), String> {
        let parsed = Target::parse(target).map_err(|e| e.to_string())?;
        if parsed.is_inventory() {
            return Err("inventory uses whole-document acceptance".into());
        }
        let (epoch, revision) = parse_generation(generation)?;
        let key = (target.to_owned(), generation.to_owned());
        if payload.is_empty() {
            self.forget_pending(&key);
            if self.accepts_state(target, generation) {
                self.devices
                    .values_mut()
                    .find(|d| d.record.target == target)
                    .unwrap()
                    .observed = None;
            }
            return Ok(());
        }
        if payload.len() > STATE_BYTES {
            return Err("State exceeds per-observation budget".into());
        }
        let value: Value = serde_json::from_slice(payload).map_err(|e| e.to_string())?;
        let object = value.as_object().ok_or("State must be an object")?;
        if object.keys().any(|key| {
            key.is_empty() || key.contains(['/', '+', '#']) || key.chars().any(char::is_control)
        }) || object.get("assumed").is_some_and(|v| !v.is_boolean())
        {
            return Err("invalid State properties".into());
        }
        if self.accepts_state(target, generation) {
            self.devices
                .values_mut()
                .find(|d| d.record.target == target)
                .unwrap()
                .observed = Some(value);
        } else if epoch != self.epoch || revision > self.revision {
            // Unknown names/epochs cannot create records. Keep at most the
            // latest value for each key, evicting in first-arrival order.
            if !self.pending.contains_key(&key) {
                self.order.push_back(key.clone());
            }
            self.pending.insert(key, value);
            while self.pending.len() > PENDING_ITEMS || self.pending_bytes() > PENDING_BYTES {
                let oldest = self
                    .order
                    .pop_front()
                    .expect("pending cache has an order entry");
                self.pending.remove(&oldest);
            }
        }
        Ok(())
    }
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
    fn pending_bytes(&self) -> usize {
        self.pending
            .iter()
            .map(|((target, generation), value)| {
                target.len() + generation.len() + value.to_string().len()
            })
            .sum()
    }
    fn forget_pending(&mut self, key: &(String, String)) {
        self.pending.remove(key);
        self.order.retain(|k| k != key);
    }
    fn reconcile_observations(&mut self) {
        let keys: Vec<_> = self.order.iter().cloned().collect();
        for key in keys {
            let (epoch, revision) = parse_generation(&key.1).expect("validated pending generation");
            if self.accepts_state(&key.0, &key.1) {
                let value = self.pending[&key].clone();
                self.devices
                    .values_mut()
                    .find(|d| d.record.target == key.0)
                    .unwrap()
                    .observed = Some(value);
                self.forget_pending(&key);
            } else if epoch == self.epoch && revision <= self.revision {
                self.forget_pending(&key);
            }
        }
    }
    pub fn by_target(&self, target: &str) -> Option<&Device> {
        self.devices.values().find(|d| d.record.target == target)
    }
    pub fn set_intent(&mut self, target: &str, value: Value) {
        self.devices
            .values_mut()
            .find(|d| d.record.target == target)
            .unwrap()
            .intent = Some(value);
    }
    pub fn accepts_state(&self, target: &str, generation: &str) -> bool {
        self.by_target(target).is_some_and(|device| {
            generation == format!("{}.{}", self.epoch, device.record.binding_revision)
        })
    }
    pub fn accept(&mut self, state: &[u8]) -> Result<Acceptance, String> {
        if state.len() > 512 * 1024 {
            return Err("snapshot exceeds consumer budget".into());
        }
        let envelope: Value = serde_json::from_slice(state).map_err(|e| e.to_string())?;
        let document = envelope.get("Inventory").ok_or("no complete inventory")?;
        if document["schema_version"].as_u64() != Some(1) || envelope.get("assumed").is_some() {
            return Err("not a supported observed inventory".into());
        }
        let epoch = document["inventory_epoch"].as_str().ok_or("no epoch")?;
        if epoch.len() != 32
            || !epoch
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid epoch".into());
        }
        let revision = document["revision"]
            .as_u64()
            .filter(|r| *r > 0)
            .ok_or("invalid revision")?;
        if !self.epoch.is_empty() && epoch != self.epoch {
            return Err("explicit reset adoption required".into());
        }
        if revision < self.revision {
            return Ok(Acceptance::Ignored);
        }
        if revision == self.revision {
            return if self.document.as_ref() == Some(document) {
                Ok(Acceptance::Unchanged)
            } else {
                Err("equal revision changed contents".into())
            };
        }
        let ledger: Ledger = serde_json::from_value(document["identity_ledger"].clone())
            .map_err(|e| e.to_string())?;
        if let Some(before) = &self.ledger {
            if before.baseline != ledger.baseline
                || !ledger.batches.starts_with(&before.batches)
                || ledger.batches[before.batches.len()..]
                    .iter()
                    .any(|b| b.revision <= self.revision)
            {
                return Err("accepted history was rewritten".into());
            }
        }
        // Build a lifetime table from the entire named history. This consumer
        // matches resulting lifetime origins, rather than invoking producer edits.
        let mut active: BTreeMap<String, (Origin, Projection)> = BTreeMap::new();
        for record in &ledger.baseline {
            if record.binding_revision != 1
                || active
                    .insert(
                        record.target.clone(),
                        (
                            Origin {
                                target: record.target.clone(),
                                revision: 1,
                            },
                            record.clone(),
                        ),
                    )
                    .is_some()
            {
                return Err("invalid epoch baseline".into());
            }
        }
        validate_active(&active)?;
        let mut last = 1;
        let mut safe_intent_migration: BTreeSet<_> = self.devices.keys().cloned().collect();
        for batch in &ledger.batches {
            let prior_parents = input_ancestors(&active)?;
            if batch.revision <= last || batch.revision > revision || batch.transitions.is_empty() {
                return Err("invalid history order".into());
            }
            last = batch.revision;
            let mut departures = BTreeMap::new();
            let mut arrivals = BTreeSet::new();
            for edge in &batch.transitions {
                if edge.from.is_none() && edge.to.is_none() {
                    return Err("empty transition".into());
                }
                if let Some(from) = &edge.from {
                    let (origin, current) = active
                        .get(&from.target)
                        .ok_or("missing transition predecessor")?;
                    if current != from
                        || departures
                            .insert(from.target.clone(), origin.clone())
                            .is_some()
                    {
                        return Err("inconsistent transition predecessor".into());
                    }
                }
                if let Some(to) = &edge.to {
                    if to.binding_revision != batch.revision || !arrivals.insert(to.target.clone())
                    {
                        return Err("invalid transition successor".into());
                    }
                    if let Some(from) = &edge.from {
                        if std::mem::discriminant(&from.location)
                            != std::mem::discriminant(&to.location)
                        {
                            return Err("address-space conversion is not continuity".into());
                        }
                        if batch.revision > self.revision
                            && (from.members != to.members
                                || from.instance_type != to.instance_type
                                || from.color_representation != to.color_representation
                                || (from.target == to.target && from.location == to.location))
                        {
                            safe_intent_migration.remove(&departures[&from.target]);
                        }
                    }
                }
            }
            for target in departures.keys() {
                active.remove(target);
            }
            for edge in &batch.transitions {
                if let Some(to) = &edge.to {
                    let origin = match &edge.from {
                        Some(from) => departures[&from.target].clone(),
                        None => Origin {
                            target: to.target.clone(),
                            revision: batch.revision,
                        },
                    };
                    if active
                        .insert(to.target.clone(), (origin, to.clone()))
                        .is_some()
                    {
                        return Err("successor collides with active name".into());
                    }
                }
            }
            validate_active(&active)?;
            for (child, parent) in input_ancestors(&active)? {
                if prior_parents.get(&child).is_some_and(|old| old != &parent) {
                    return Err("input child outlived its parent lifetime".into());
                }
            }
        }
        verify_current(document, &active)?;
        // Commit consumer changes only after the complete document validates.
        let mut accepted = self.clone();
        let previous = accepted.devices.clone();
        accepted.devices.clear();
        for (origin, record) in active.into_values() {
            let device = match previous.get(&origin) {
                Some(old) => Device {
                    id: old.id,
                    record: record.clone(),
                    observed: if old.record.target == record.target
                        && old.record.binding_revision == record.binding_revision
                    {
                        old.observed.clone()
                    } else {
                        None
                    },
                    intent: if old.record.binding_revision == record.binding_revision
                        || safe_intent_migration.contains(&origin)
                    {
                        old.intent.clone()
                    } else {
                        None
                    },
                },
                None => {
                    accepted.next_id += 1;
                    Device {
                        id: accepted.next_id,
                        record,
                        intent: None,
                        observed: None,
                    }
                }
            };
            accepted.devices.insert(origin, device);
        }
        accepted.retired.extend(
            previous
                .into_iter()
                .filter(|(origin, _)| !accepted.devices.contains_key(origin))
                .map(|(_, d)| d.id),
        );
        accepted.epoch = epoch.into();
        accepted.revision = revision;
        accepted.ledger = Some(ledger);
        accepted.document = Some(document.clone());
        accepted.reconcile_observations();
        *self = accepted;
        Ok(Acceptance::Applied)
    }
}

fn parse_generation(text: &str) -> Result<(&str, u64), String> {
    if text.len() > 128 {
        return Err("generation exceeds wire budget".into());
    }
    let (epoch, revision) = text
        .split_once('.')
        .ok_or("invalid observation generation")?;
    let number = revision
        .parse::<u64>()
        .map_err(|_| "invalid generation revision")?;
    if epoch.len() != 32
        || !epoch
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || number == 0
        || number.to_string() != revision
    {
        return Err("invalid observation generation".into());
    }
    Ok((epoch, number))
}

fn validate_active(active: &BTreeMap<String, (Origin, Projection)>) -> Result<(), String> {
    let mut places = BTreeSet::new();
    let mut origins = BTreeSet::new();
    let mut ancestors = BTreeMap::new();
    let mut physical_buses = BTreeMap::new();
    let outputs: BTreeMap<_, _> = active
        .values()
        .filter_map(|(origin, record)| match record.location {
            Location::Output { bus, .. } => Some((origin, bus)),
            _ => None,
        })
        .collect();
    for (origin, record) in active.values() {
        let target = Target::parse(&record.target).map_err(|e| e.to_string())?;
        let name = target.bus().ok_or("no bus ancestor in history")?;
        let index = match record.location {
            Location::Output { bus, .. }
            | Location::Group { bus, .. }
            | Location::Input { bus, .. }
            | Location::InputInstance { bus, .. } => bus,
        };
        if ancestors
            .insert(index, name.to_owned())
            .is_some_and(|old| old != name)
            || physical_buses
                .insert(name.to_owned(), index)
                .is_some_and(|old| old != index)
        {
            return Err("history splits a bus ancestor name or index".into());
        }
        let valid = match record.location {
            Location::Output { bus, short_address } => {
                bus < 4
                    && short_address < 64
                    && target.namespace() == Some(Namespace::Outputs)
                    && record.members.is_empty()
                    && target.instance().is_none()
                    && record.instance_type.is_none()
            }
            Location::Group { bus, group_address } => {
                bus < 4
                    && group_address < 16
                    && target.namespace() == Some(Namespace::Groups)
                    && record.members.iter().all(|m| outputs.get(m) == Some(&bus))
                    && target.instance().is_none()
                    && record.instance_type.is_none()
                    && record.color_representation.is_none()
            }
            Location::Input { bus, short_address } => {
                bus < 4
                    && short_address < 64
                    && target.namespace() == Some(Namespace::Inputs)
                    && target.instance().is_none()
                    && record.members.is_empty()
                    && record.instance_type.is_none()
                    && record.color_representation.is_none()
            }
            Location::InputInstance {
                bus,
                short_address,
                instance,
            } => {
                bus < 4
                    && short_address < 64
                    && instance < 32
                    && target.namespace() == Some(Namespace::Inputs)
                    && target.instance().is_some()
                    && record.members.is_empty()
                    && record.instance_type.is_some()
                    && record.color_representation.is_none()
            }
        };
        if !valid
            || record.binding_revision == 0
            || !places.insert(record.location)
            || !origins.insert(origin)
            || record.members.windows(2).any(|m| m[0] >= m[1])
        {
            return Err("invalid active identity projection".into());
        }
    }
    input_ancestors(active)?;
    Ok(())
}

fn input_ancestors(
    active: &BTreeMap<String, (Origin, Projection)>,
) -> Result<BTreeMap<Origin, Origin>, String> {
    let parents: BTreeMap<_, _> = active
        .values()
        .filter_map(|(origin, p)| match p.location {
            Location::Input { bus, short_address } => Some(((bus, short_address), (origin, p))),
            _ => None,
        })
        .collect();
    let mut result = BTreeMap::new();
    for (origin, child) in active.values() {
        if let Location::InputInstance {
            bus, short_address, ..
        } = child.location
        {
            let (parent_origin, parent) = parents
                .get(&(bus, short_address))
                .ok_or("orphan input child")?;
            let target = Target::parse(&child.target).map_err(|e| e.to_string())?;
            let parent_target = Target::parse(&parent.target).map_err(|e| e.to_string())?;
            if target.name() != parent_target.name() {
                return Err("input child has another module name".into());
            }
            result.insert(origin.clone(), (*parent_origin).clone());
        }
    }
    Ok(result)
}
fn verify_current(
    document: &Value,
    active: &BTreeMap<String, (Origin, Projection)>,
) -> Result<(), String> {
    let controller = document["name"].as_str().ok_or("no controller name")?;
    valid_name(controller)?;
    let chars: Vec<_> = controller.chars().collect();
    if controller.contains(['+', '#'])
        || controller.starts_with(['@', '*'])
        || (controller.starts_with('{') && controller.ends_with('}'))
        || controller.ends_with('@')
        || (chars.len() >= 2
            && chars[chars.len() - 1] == '_'
            && chars[chars.len() - 2].is_ascii_digit())
        || (chars.len() >= 3 && chars[0] == '_' && chars[1] != '_' && chars[2..].contains(&'_'))
    {
        return Err("invalid controller wire name".into());
    }
    valid_description(document)?;
    let mut seen = BTreeSet::new();
    let mut buses = BTreeSet::new();
    let mut bus_names = BTreeSet::new();
    let outputs: BTreeMap<_, _> = active
        .values()
        .map(|(origin, r)| (r.location, origin))
        .collect();
    for bus in document["buses"]
        .as_array()
        .ok_or("no complete bus array")?
    {
        let index = u8::try_from(bus["bus"].as_u64().ok_or("no bus index")?)
            .map_err(|_| "bus index too large")?;
        let name = bus["name"].as_str().ok_or("no bus name")?;
        valid_name(name)?;
        valid_description(bus)?;
        if index >= 4
            || !buses.insert(index)
            || !bus_names.insert(name)
            || !bus["enabled"].is_boolean()
        {
            return Err("invalid or duplicate bus metadata".into());
        }
        for (field, space) in [
            ("outputs", Namespace::Outputs),
            ("groups", Namespace::Groups),
        ] {
            for item in bus[field].as_array().ok_or("incomplete item array")? {
                valid_description(item)?;
                let expected =
                    Target::item(name, space, item["name"].as_str().ok_or("no item name")?)
                        .map_err(|e| e.to_string())?
                        .to_string();
                if item["target"].as_str() != Some(&expected) || !seen.insert(expected.clone()) {
                    return Err("invalid current target".into());
                }
                let (_, record) = active
                    .get(&expected)
                    .ok_or("current target not in history")?;
                let address = u8::try_from(
                    item[if field == "outputs" {
                        "short_address"
                    } else {
                        "group_address"
                    }]
                    .as_u64()
                    .ok_or("no address")?,
                )
                .map_err(|_| "address too large")?;
                let place = if field == "outputs" {
                    Location::Output {
                        bus: index,
                        short_address: address,
                    }
                } else {
                    Location::Group {
                        bus: index,
                        group_address: address,
                    }
                };
                if place != record.location
                    || item["binding_revision"].as_u64() != Some(record.binding_revision)
                {
                    return Err("current mapping disagrees with history".into());
                }
                let representation = item
                    .get("color_representation")
                    .cloned()
                    .unwrap_or(Value::Null);
                if representation
                    != serde_json::to_value(record.color_representation)
                        .map_err(|e| e.to_string())?
                {
                    return Err("current color representation disagrees with history".into());
                }
                if field == "groups" {
                    let mut members = Vec::new();
                    for member in item["members"].as_array().ok_or("no member array")? {
                        let address = u8::try_from(member.as_u64().ok_or("invalid member")?)
                            .map_err(|_| "member too large")?;
                        members.push(
                            (*outputs
                                .get(&Location::Output {
                                    bus: index,
                                    short_address: address,
                                })
                                .ok_or("missing member output")?)
                            .clone(),
                        );
                    }
                    members.sort();
                    if members != record.members {
                        return Err("effective group members disagree with history".into());
                    }
                }
            }
        }
        if let Some(inputs) = bus.get("inputs") {
            for input in inputs.as_array().ok_or("invalid input array")? {
                valid_description(input)?;
                let module_name = input["name"].as_str().ok_or("no input name")?;
                let address =
                    u8::try_from(input["short_address"].as_u64().ok_or("no input address")?)
                        .map_err(|_| "input address too large")?;
                let target = Target::item(name, Namespace::Inputs, module_name)
                    .map_err(|e| e.to_string())?
                    .to_string();
                let (_, record) = active.get(&target).ok_or("input not in history")?;
                if input
                    .get("color_representation")
                    .is_some_and(|v| !v.is_null())
                    || input["target"].as_str() != Some(&target)
                    || !seen.insert(target)
                    || record.location
                        != (Location::Input {
                            bus: index,
                            short_address: address,
                        })
                    || input["binding_revision"].as_u64() != Some(record.binding_revision)
                {
                    return Err("input mapping disagrees with history".into());
                }
                for instance in input["instances"].as_array().ok_or("no instance array")? {
                    valid_description(instance)?;
                    let target = Target::input_instance(
                        name,
                        module_name,
                        instance["name"].as_str().ok_or("no instance name")?,
                    )
                    .map_err(|e| e.to_string())?
                    .to_string();
                    let number =
                        u8::try_from(instance["number"].as_u64().ok_or("no instance number")?)
                            .map_err(|_| "instance number too large")?;
                    let kind = u8::try_from(
                        instance["instance_type"]
                            .as_u64()
                            .ok_or("no instance type")?,
                    )
                    .map_err(|_| "instance type too large")?;
                    let (_, record) = active.get(&target).ok_or("instance not in history")?;
                    if instance
                        .get("color_representation")
                        .is_some_and(|v| !v.is_null())
                        || instance["target"].as_str() != Some(&target)
                        || !seen.insert(target)
                        || record.location
                            != (Location::InputInstance {
                                bus: index,
                                short_address: address,
                                instance: number,
                            })
                        || record.instance_type != Some(kind)
                        || instance["binding_revision"].as_u64() != Some(record.binding_revision)
                    {
                        return Err("instance mapping disagrees with history".into());
                    }
                }
            }
        }
    }
    if seen.len() != active.len() {
        return Err("partial current inventory".into());
    }
    Ok(())
}

fn valid_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty()
        || name.contains('/')
        || matches!(name, "." | "..")
        || name.chars().any(char::is_control)
    {
        return Err("invalid inventory name".into());
    }
    Ok(())
}
fn valid_description(object: &Value) -> Result<(), String> {
    if object
        .get("description")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Err("description must be a string or null".into());
    }
    Ok(())
}
