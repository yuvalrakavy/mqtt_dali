use super::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(super) struct Tracked {
    pub projection: Projection,
    pub origin: Origin,
}
pub(super) type Records = BTreeMap<String, Tracked>;

fn qualified(bus: &str, namespace: Namespace, name: &str, path: &str) -> Result<String, String> {
    Target::item(bus, namespace, name)
        .map(|t| t.to_string())
        .map_err(|error| {
            // Diagnostic measurement only; Target remains the validating codec.
            let escaped_len = |value: &str| {
                value
                    .bytes()
                    .map(|b| {
                        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-') {
                            1
                        } else {
                            3
                        }
                    })
                    .sum::<usize>()
            };
            let namespace_len = if namespace == Namespace::Outputs {
                7
            } else {
                6
            };
            let bytes = escaped_len(bus) + namespace_len + 2 + escaped_len(name);
            format!("{path}: {error}; encoded target is {bytes} bytes")
        })
}

pub(super) fn catalog(inventory: &Inventory) -> Result<Vec<(Location, String)>, String> {
    let mut result = Vec::new();
    for (b, bus) in inventory.buses.iter().enumerate() {
        for (i, output) in bus.outputs.iter().enumerate() {
            let target = qualified(
                &bus.name,
                Namespace::Outputs,
                &output.name,
                &format!("buses[{b}].outputs[{i}].name"),
            )?;
            result.push((
                Location::Output {
                    bus: bus.bus,
                    short_address: output.short_address,
                },
                target.to_string(),
            ));
        }
        for (i, group) in bus.groups.iter().enumerate() {
            let target = qualified(
                &bus.name,
                Namespace::Groups,
                &group.name,
                &format!("buses[{b}].groups[{i}].name"),
            )?;
            result.push((
                Location::Group {
                    bus: bus.bus,
                    group_address: group.group_address,
                },
                target.to_string(),
            ));
        }
        for (i, input) in bus.inputs.iter().enumerate() {
            let path = format!("buses[{b}].inputs[{i}]");
            let target = qualified(
                &bus.name,
                Namespace::Inputs,
                &input.name,
                &format!("{path}.name"),
            )?;
            result.push((
                Location::Input {
                    bus: bus.bus,
                    short_address: input.short_address,
                },
                target,
            ));
            for (j, instance) in input.instances.iter().enumerate() {
                let target = Target::input_instance(&bus.name, &input.name, &instance.name)
                    .map_err(|error| format!("{path}.instances[{j}].name: {error}"))?;
                result.push((
                    Location::InputInstance {
                        bus: bus.bus,
                        short_address: input.short_address,
                        instance: instance.number,
                    },
                    target.to_string(),
                ));
            }
        }
    }
    result.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(result)
}

pub(super) fn project(
    inventory: &Inventory,
    origins: &BTreeMap<Location, Origin>,
) -> Result<Vec<Projection>, String> {
    catalog(inventory)?
        .into_iter()
        .map(|(location, target)| {
            let mut members = Vec::new();
            if let Location::Group { bus, group_address } = location {
                let group = inventory
                    .buses
                    .iter()
                    .find(|b| b.bus == bus)
                    .unwrap()
                    .groups
                    .iter()
                    .find(|g| g.group_address == group_address)
                    .unwrap();
                for address in &group.members {
                    members.push(
                        origins
                            .get(&Location::Output {
                                bus,
                                short_address: *address,
                            })
                            .ok_or_else(|| {
                                format!("{target}: member address {address} has no active lifetime")
                            })?
                            .clone(),
                    );
                }
                members.sort();
            }
            Ok(Projection {
                target,
                binding_revision: 1,
                location,
                members,
                instance_type: if let Location::InputInstance {
                    bus,
                    short_address,
                    instance,
                } = location
                {
                    Some(
                        inventory
                            .buses
                            .iter()
                            .find(|b| b.bus == bus)
                            .unwrap()
                            .inputs
                            .iter()
                            .find(|i| i.short_address == short_address)
                            .unwrap()
                            .instances
                            .iter()
                            .find(|i| i.number == instance)
                            .unwrap()
                            .instance_type,
                    )
                } else {
                    None
                },
            })
        })
        .collect()
}

pub(super) fn baseline(inventory: &Inventory) -> Result<Ledger, String> {
    let origins = catalog(inventory)?
        .into_iter()
        .map(|(location, target)| {
            (
                location,
                Origin {
                    target,
                    revision: 1,
                },
            )
        })
        .collect();
    Ok(Ledger {
        baseline: project(inventory, &origins)?,
        batches: vec![],
    })
}

fn shape(projection: &Projection) -> Result<(), String> {
    let target = Target::parse(&projection.target).map_err(|e| e.to_string())?;
    let valid = match projection.location {
        Location::Output { bus, short_address } => {
            bus < 4
                && short_address < 64
                && target.namespace() == Some(Namespace::Outputs)
                && projection.members.is_empty()
                && target.instance().is_none()
                && projection.instance_type.is_none()
        }
        Location::Group { bus, group_address } => {
            bus < 4
                && group_address < 16
                && target.namespace() == Some(Namespace::Groups)
                && target.instance().is_none()
                && projection.instance_type.is_none()
        }
        Location::Input { bus, short_address } => {
            bus < 4
                && short_address < 64
                && target.namespace() == Some(Namespace::Inputs)
                && target.instance().is_none()
                && projection.members.is_empty()
                && projection.instance_type.is_none()
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
                && projection.members.is_empty()
                && projection.instance_type.is_some()
        }
    };
    if !valid || projection.binding_revision == 0 {
        return Err(format!(
            "{}: invalid identity projection",
            projection.target
        ));
    }
    if projection.members.windows(2).any(|m| m[0] >= m[1]) {
        return Err(format!(
            "{}: member origins must be sorted and unique",
            projection.target
        ));
    }
    Ok(())
}

fn valid_records(records: &Records) -> Result<(), String> {
    let mut locations = BTreeSet::new();
    let mut origins = BTreeSet::new();
    let mut output_origins = BTreeMap::new();
    let mut bus_names = BTreeMap::new();
    let mut bus_indices = BTreeMap::new();
    for record in records.values() {
        let projection = &record.projection;
        shape(projection)?;
        let target = Target::parse(&projection.target).map_err(|e| e.to_string())?;
        let name = target.bus().ok_or("identity projection has no bus")?;
        let bus = match projection.location {
            Location::Output { bus, .. }
            | Location::Group { bus, .. }
            | Location::Input { bus, .. }
            | Location::InputInstance { bus, .. } => bus,
        };
        if bus_names
            .insert(bus, name.to_owned())
            .is_some_and(|old| old != name)
            || bus_indices
                .insert(name.to_owned(), bus)
                .is_some_and(|old| old != bus)
        {
            return Err("identity ledger has inconsistent bus ancestor names or indices".into());
        }
        if !locations.insert(projection.location) || !origins.insert(&record.origin) {
            return Err("duplicate physical location or active lifetime in identity ledger".into());
        }
        if let Location::Output { bus, .. } = projection.location {
            output_origins.insert(&record.origin, bus);
        }
    }
    for record in records.values() {
        if let Location::Group { bus, .. } = record.projection.location {
            for member in &record.projection.members {
                if output_origins.get(member) != Some(&bus) {
                    return Err(format!(
                        "{}: group member origin is not an active same-bus output",
                        record.projection.target
                    ));
                }
            }
        }
    }
    input_parents(records)?;
    Ok(())
}

/// Parent lifetime is checked separately from address/name ancestry.
fn input_parents(records: &Records) -> Result<BTreeMap<Origin, Origin>, String> {
    let modules: BTreeMap<_, _> = records
        .values()
        .filter_map(|r| match r.projection.location {
            Location::Input { bus, short_address } => Some(((bus, short_address), r)),
            _ => None,
        })
        .collect();
    let mut result = BTreeMap::new();
    for child in records.values() {
        if let Location::InputInstance {
            bus, short_address, ..
        } = child.projection.location
        {
            let parent = modules
                .get(&(bus, short_address))
                .ok_or("input instance has no active parent")?;
            let child_target =
                Target::parse(&child.projection.target).map_err(|e| e.to_string())?;
            let parent_target =
                Target::parse(&parent.projection.target).map_err(|e| e.to_string())?;
            if child_target.name() != parent_target.name() {
                return Err("input instance disagrees with parent name".into());
            }
            result.insert(child.origin.clone(), parent.origin.clone());
        }
    }
    Ok(result)
}

pub(super) fn replay(ledger: &Ledger, revision: u64) -> Result<Records, String> {
    let mut records = Records::new();
    for projection in &ledger.baseline {
        if projection.binding_revision != 1 {
            return Err("baseline binding revision must be 1".into());
        }
        let record = Tracked {
            projection: projection.clone(),
            origin: Origin {
                target: projection.target.clone(),
                revision: 1,
            },
        };
        if records.insert(projection.target.clone(), record).is_some() {
            return Err("duplicate baseline target".into());
        }
    }
    valid_records(&records)?;
    let mut previous = 1;
    for batch in &ledger.batches {
        let parents = input_parents(&records)?;
        if batch.revision <= previous || batch.revision > revision || batch.transitions.is_empty() {
            return Err("identity batch revision/order/content is invalid".into());
        }
        previous = batch.revision;
        let mut sources = BTreeSet::new();
        let mut destinations = BTreeSet::new();
        for edge in &batch.transitions {
            if edge.from.is_none() && edge.to.is_none() {
                return Err("empty identity transition".into());
            }
            if let Some(from) = &edge.from {
                if !sources.insert(&from.target)
                    || records.get(&from.target).map(|r| &r.projection) != Some(from)
                {
                    return Err("duplicate or inconsistent transition source".into());
                }
            }
            if let Some(to) = &edge.to {
                shape(to)?;
                if to.binding_revision != batch.revision || !destinations.insert(&to.target) {
                    return Err(
                        "duplicate destination or incorrect transition binding revision".into(),
                    );
                }
            }
            if let (Some(from), Some(to)) = (&edge.from, &edge.to) {
                if std::mem::discriminant(&from.location) != std::mem::discriminant(&to.location) {
                    return Err("a continuity transition cannot change address spaces".into());
                }
            }
        }
        let mut carried = BTreeMap::new();
        for source in sources {
            carried.insert(source.clone(), records.remove(source).unwrap().origin);
        }
        for edge in &batch.transitions {
            if let Some(to) = &edge.to {
                let origin = edge
                    .from
                    .as_ref()
                    .map(|from| carried[&from.target].clone())
                    .unwrap_or_else(|| Origin {
                        target: to.target.clone(),
                        revision: batch.revision,
                    });
                if records
                    .insert(
                        to.target.clone(),
                        Tracked {
                            projection: to.clone(),
                            origin,
                        },
                    )
                    .is_some()
                {
                    return Err("transition destination collides with an unchanged target".into());
                }
            }
        }
        valid_records(&records)?;
        for (child, parent) in input_parents(&records)? {
            if parents.get(&child).is_some_and(|old| *old != parent) {
                return Err("continuing input instance changed parent lifetime".into());
            }
        }
    }
    Ok(records)
}

pub(super) fn match_current(inventory: &Inventory, records: &Records) -> Result<(), String> {
    let by_location: BTreeMap<_, _> = records
        .values()
        .map(|r| (r.projection.location, r))
        .collect();
    let origins = by_location
        .iter()
        .map(|(location, record)| (*location, record.origin.clone()))
        .collect();
    let mut current = project(inventory, &origins)?;
    if current.len() != records.len() {
        return Err("identity history does not match the current item set".into());
    }
    for projection in &mut current {
        let record = by_location
            .get(&projection.location)
            .ok_or("configured location is absent from identity history")?;
        projection.binding_revision = record.projection.binding_revision;
        if *projection != record.projection {
            return Err(format!(
                "{}: current mapping/membership disagrees with identity history",
                projection.target
            ));
        }
    }
    Ok(())
}
