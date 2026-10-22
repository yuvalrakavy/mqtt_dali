use super::*;
use crate::config::Bus;
use std::collections::BTreeSet;

pub(super) fn change(model: &Model, expected: u64, edits: &[Edit]) -> Result<Model, String> {
    if expected != model.inventory.revision {
        return Err("inventory revision conflict".into());
    }
    let revision = model
        .inventory
        .revision_high_water
        .unwrap_or(model.inventory.revision)
        .checked_add(1)
        .ok_or("inventory revision counter exhausted")?;
    let mut inventory = model.inventory.clone();
    let mut origins: BTreeMap<_, _> = model
        .records
        .values()
        .map(|r| (r.projection.location, r.origin.clone()))
        .collect();
    let original_origins = origins.clone();
    for edit in edits {
        apply(&mut inventory, &mut origins, revision, edit)?;
    }
    if inventory == model.inventory && origins == original_origins {
        return Ok(model.clone());
    }
    crate::config::validate_structure(&inventory)?;
    // A fresh item may have been renamed/readdressed inside this atomic edit.
    // Its public lifetime begins only at the final committed target.
    for (location, target) in ledger::catalog(&inventory)? {
        let origin = origins
            .get_mut(&location)
            .ok_or("edited item has no lifetime")?;
        if origin.revision == revision {
            origin.target = target;
        }
    }
    let old: BTreeMap<_, _> = model
        .records
        .values()
        .map(|r| (&r.origin, &r.projection))
        .collect();
    let mut proposed = ledger::project(&inventory, &origins)?;
    let mut next = BTreeMap::new();
    for projection in &mut proposed {
        let origin = &origins[&projection.location];
        projection.binding_revision = match old.get(origin) {
            Some(before)
                if before.target == projection.target
                    && before.location == projection.location
                    && before.members == projection.members
                    && before.instance_type == projection.instance_type
                    && before.color_representation == projection.color_representation =>
            {
                before.binding_revision
            }
            _ => revision,
        };
        next.insert(origin.clone(), projection.clone());
    }
    let mut transitions = Vec::new();
    for (origin, before) in &old {
        let after = next.remove(*origin);
        if after.as_ref() != Some(*before) {
            transitions.push(Transition {
                from: Some((*before).clone()),
                to: after,
            });
        }
    }
    transitions.extend(next.into_values().map(|to| Transition {
        from: None,
        to: Some(to),
    }));
    let mut ledger = model.ledger.clone();
    if !transitions.is_empty() {
        ledger.batches.push(Batch {
            revision,
            transitions,
        });
    }
    inventory.identity_ledger = Some(ledger);
    inventory.revision = revision;
    inventory.revision_high_water = Some(revision);
    Model::new(inventory)
}

fn bus(inventory: &mut Inventory, index: u8) -> Result<&mut Bus, String> {
    inventory
        .buses
        .iter_mut()
        .find(|b| b.bus == index)
        .ok_or_else(|| format!("unknown bus {index}"))
}
fn fresh(origins: &mut BTreeMap<Location, Origin>, location: Location, revision: u64) {
    // Finalize the target after all edits; this placeholder never escapes.
    origins.insert(
        location,
        Origin {
            target: String::new(),
            revision,
        },
    );
}
fn apply(
    inventory: &mut Inventory,
    origins: &mut BTreeMap<Location, Origin>,
    revision: u64,
    edit: &Edit,
) -> Result<(), String> {
    match edit {
        Edit::OutputColorRepresentation {
            bus: index,
            address,
            representation,
        } => {
            bus(inventory, *index)?
                .outputs
                .iter_mut()
                .find(|o| o.short_address == *address)
                .ok_or("unknown output")?
                .color_representation = *representation;
        }
        Edit::RenameInput {
            bus: index,
            address,
            name,
        } => {
            bus(inventory, *index)?
                .inputs
                .iter_mut()
                .find(|i| i.short_address == *address)
                .ok_or("unknown input")?
                .name = name.clone();
        }
        Edit::RenameInputInstance {
            bus: index,
            address,
            instance,
            name,
        } => {
            bus(inventory, *index)?
                .inputs
                .iter_mut()
                .find(|i| i.short_address == *address)
                .ok_or("unknown input")?
                .instances
                .iter_mut()
                .find(|i| i.number == *instance)
                .ok_or("unknown input instance")?
                .name = name.clone();
        }
        Edit::AddInputInstance {
            bus: index,
            address,
            instance,
        } => {
            let input = bus(inventory, *index)?
                .inputs
                .iter_mut()
                .find(|i| i.short_address == *address)
                .ok_or("unknown input")?;
            if input.instances.iter().any(|i| i.number == instance.number) {
                return Err("input instance already reserved".into());
            }
            input.instances.push(instance.clone());
            fresh(
                origins,
                Location::InputInstance {
                    bus: *index,
                    short_address: *address,
                    instance: instance.number,
                },
                revision,
            );
        }
        Edit::AddInput { bus: index, input } => {
            let bus = bus(inventory, *index)?;
            if bus
                .inputs
                .iter()
                .any(|i| i.short_address == input.short_address)
            {
                return Err("input address already reserved".into());
            }
            bus.inputs.push(input.clone());
            fresh_input(origins, *index, input, revision);
        }
        Edit::ReplaceInput { bus: index, input } => {
            let old = bus(inventory, *index)?
                .inputs
                .iter_mut()
                .find(|i| i.short_address == input.short_address)
                .ok_or("unknown replacement input")?;
            retire_input(origins, *index, old);
            *old = input.clone();
            fresh_input(origins, *index, input, revision);
        }
        Edit::RemoveInput {
            bus: index,
            address,
        } => {
            let bus = bus(inventory, *index)?;
            let at = bus
                .inputs
                .iter()
                .position(|i| i.short_address == *address)
                .ok_or("unknown input")?;
            let old = bus.inputs.remove(at);
            retire_input(origins, *index, &old);
        }
        Edit::Description { description } => inventory.description = description.clone(),
        Edit::RenameBus { bus: index, name } => bus(inventory, *index)?.name = name.clone(),
        Edit::RenameOutput {
            bus: index,
            address,
            name,
        } => {
            bus(inventory, *index)?
                .outputs
                .iter_mut()
                .find(|o| o.short_address == *address)
                .ok_or("unknown output")?
                .name = name.clone();
        }
        Edit::RenameGroup {
            bus: index,
            address,
            name,
        } => {
            bus(inventory, *index)?
                .groups
                .iter_mut()
                .find(|g| g.group_address == *address)
                .ok_or("unknown group")?
                .name = name.clone();
        }
        Edit::AddOutput { bus: index, output } => {
            let bus = bus(inventory, *index)?;
            if bus
                .outputs
                .iter()
                .any(|o| o.short_address == output.short_address)
            {
                return Err("output address already reserved".into());
            }
            bus.outputs.push(output.clone());
            fresh(
                origins,
                Location::Output {
                    bus: *index,
                    short_address: output.short_address,
                },
                revision,
            );
        }
        Edit::ReplaceOutput { bus: index, output } => {
            *bus(inventory, *index)?
                .outputs
                .iter_mut()
                .find(|o| o.short_address == output.short_address)
                .ok_or("unknown replacement output")? = output.clone();
            fresh(
                origins,
                Location::Output {
                    bus: *index,
                    short_address: output.short_address,
                },
                revision,
            );
        }
        Edit::RemoveOutput {
            bus: index,
            address,
        } => {
            let bus = bus(inventory, *index)?;
            let at = bus
                .outputs
                .iter()
                .position(|o| o.short_address == *address)
                .ok_or("unknown output")?;
            bus.outputs.remove(at);
            for group in &mut bus.groups {
                group.members.retain(|a| a != address);
            }
            origins.remove(&Location::Output {
                bus: *index,
                short_address: *address,
            });
        }
        Edit::AddGroup { bus: index, group } => {
            let bus = bus(inventory, *index)?;
            if bus
                .groups
                .iter()
                .any(|g| g.group_address == group.group_address)
            {
                return Err("group address already reserved".into());
            }
            bus.groups.push(group.clone());
            fresh(
                origins,
                Location::Group {
                    bus: *index,
                    group_address: group.group_address,
                },
                revision,
            );
        }
        Edit::RemoveGroup {
            bus: index,
            address,
        } => {
            let bus = bus(inventory, *index)?;
            let at = bus
                .groups
                .iter()
                .position(|g| g.group_address == *address)
                .ok_or("unknown group")?;
            bus.groups.remove(at);
            origins.remove(&Location::Group {
                bus: *index,
                group_address: *address,
            });
        }
        Edit::GroupMembers {
            bus: index,
            address,
            members,
        } => {
            bus(inventory, *index)?
                .groups
                .iter_mut()
                .find(|g| g.group_address == *address)
                .ok_or("unknown group")?
                .members = members.clone();
        }
        Edit::ReaddressOutputs {
            bus: index,
            addresses,
        } => {
            let bus = bus(inventory, *index)?;
            let mut mapping = BTreeMap::new();
            let mut destinations = BTreeSet::new();
            for (from, to) in addresses {
                if *to >= 64
                    || !bus.outputs.iter().any(|o| o.short_address == *from)
                    || mapping.insert(*from, *to).is_some()
                    || !destinations.insert(*to)
                {
                    return Err("invalid, duplicate or absent address reassignment".into());
                }
            }
            for output in &bus.outputs {
                if destinations.contains(&output.short_address)
                    && !mapping.contains_key(&output.short_address)
                {
                    return Err("reassignment would occupy a reserved address".into());
                }
            }
            let carried: Vec<_> = mapping
                .iter()
                .map(|(from, to)| {
                    let origin = origins
                        .remove(&Location::Output {
                            bus: *index,
                            short_address: *from,
                        })
                        .unwrap();
                    (*to, origin)
                })
                .collect();
            for (to, origin) in carried {
                origins.insert(
                    Location::Output {
                        bus: *index,
                        short_address: to,
                    },
                    origin,
                );
            }
            for output in &mut bus.outputs {
                if let Some(to) = mapping.get(&output.short_address) {
                    output.short_address = *to;
                }
            }
            for group in &mut bus.groups {
                for member in &mut group.members {
                    if let Some(to) = mapping.get(member) {
                        *member = *to;
                    }
                }
            }
        }
    }
    Ok(())
}

fn fresh_input(
    origins: &mut BTreeMap<Location, Origin>,
    bus: u8,
    input: &crate::config::Input,
    revision: u64,
) {
    fresh(
        origins,
        Location::Input {
            bus,
            short_address: input.short_address,
        },
        revision,
    );
    for instance in &input.instances {
        fresh(
            origins,
            Location::InputInstance {
                bus,
                short_address: input.short_address,
                instance: instance.number,
            },
            revision,
        );
    }
}
fn retire_input(origins: &mut BTreeMap<Location, Origin>, bus: u8, input: &crate::config::Input) {
    origins.remove(&Location::Input {
        bus,
        short_address: input.short_address,
    });
    for instance in &input.instances {
        origins.remove(&Location::InputInstance {
            bus,
            short_address: input.short_address,
            instance: instance.number,
        });
    }
}
