//! Example consumer algorithm. Shares wire record types, never producer replay,
//! mutation or configuration validation. No Store or hardware dependency.
use mqtt_dali::inventory::{Ledger, Location, Origin, Projection};
use mqtt_dali::targets::{Namespace, Target};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct Device {
    pub id: u64,
    pub record: Projection,
    pub intent: Option<Value>,
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
}
impl Consumer {
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
    pub fn accept(&mut self, state: &[u8]) -> Result<(), String> {
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
            return Ok(());
        }
        if revision == self.revision {
            return if self.document.as_ref() == Some(document) {
                Ok(())
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
        *self = accepted;
        Ok(())
    }
}

fn validate_active(active: &BTreeMap<String, (Origin, Projection)>) -> Result<(), String> {
    let mut places = BTreeSet::new();
    let mut origins = BTreeSet::new();
    let outputs: BTreeMap<_, _> = active
        .values()
        .filter_map(|(origin, record)| match record.location {
            Location::Output { bus, .. } => Some((origin, bus)),
            _ => None,
        })
        .collect();
    for (origin, record) in active.values() {
        let target = Target::parse(&record.target).map_err(|e| e.to_string())?;
        let valid = match record.location {
            Location::Output { bus, short_address } => {
                bus < 4
                    && short_address < 64
                    && target.namespace() == Some(Namespace::Outputs)
                    && record.members.is_empty()
            }
            Location::Group { bus, group_address } => {
                bus < 4
                    && group_address < 16
                    && target.namespace() == Some(Namespace::Groups)
                    && record.members.iter().all(|m| outputs.get(m) == Some(&bus))
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
    Ok(())
}
fn verify_current(
    document: &Value,
    active: &BTreeMap<String, (Origin, Projection)>,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let mut buses = BTreeSet::new();
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
        if !buses.insert(index) {
            return Err("duplicate bus".into());
        }
        for (field, space) in [
            ("outputs", Namespace::Outputs),
            ("groups", Namespace::Groups),
        ] {
            for item in bus[field].as_array().ok_or("incomplete item array")? {
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
    }
    if seen.len() != active.len() {
        return Err("partial current inventory".into());
    }
    Ok(())
}
