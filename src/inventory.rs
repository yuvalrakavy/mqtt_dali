//! Complete inventory snapshots and durable named identity transitions.
use crate::config::Inventory;
use crate::targets::{Namespace, Target};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod edit;
mod ledger;

pub const MAX_STATE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub target: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Location {
    Output {
        bus: u8,
        short_address: u8,
    },
    Group {
        bus: u8,
        group_address: u8,
    },
    Input {
        bus: u8,
        short_address: u8,
    },
    InputInstance {
        bus: u8,
        short_address: u8,
        instance: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub target: String,
    pub binding_revision: u64,
    pub location: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Origin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_type: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_representation: Option<crate::config::ColorRepresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub from: Option<Projection>,
    pub to: Option<Projection>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub revision: u64,
    pub transitions: Vec<Transition>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub baseline: Vec<Projection>,
    pub batches: Vec<Batch>,
}

#[derive(Debug, Clone)]
pub enum Edit {
    OutputColorRepresentation {
        bus: u8,
        address: u8,
        representation: Option<crate::config::ColorRepresentation>,
    },
    RenameInput {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameInputInstance {
        bus: u8,
        address: u8,
        instance: u8,
        name: String,
    },
    AddInputInstance {
        bus: u8,
        address: u8,
        instance: crate::config::InputInstance,
    },
    AddInput {
        bus: u8,
        input: crate::config::Input,
    },
    RemoveInput {
        bus: u8,
        address: u8,
    },
    ReplaceInput {
        bus: u8,
        input: crate::config::Input,
    },
    RenameOutput {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameGroup {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameBus {
        bus: u8,
        name: String,
    },
    ReaddressOutputs {
        bus: u8,
        addresses: Vec<(u8, u8)>,
    },
    AddOutput {
        bus: u8,
        output: crate::config::Output,
    },
    RemoveOutput {
        bus: u8,
        address: u8,
    },
    ReplaceOutput {
        bus: u8,
        output: crate::config::Output,
    },
    AddGroup {
        bus: u8,
        group: crate::config::Group,
    },
    RemoveGroup {
        bus: u8,
        address: u8,
    },
    GroupMembers {
        bus: u8,
        address: u8,
        members: Vec<u8>,
    },
    Description {
        description: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Model {
    inventory: Inventory,
    ledger: Ledger,
    projections: Vec<Projection>,
    state: Vec<u8>,
    records: ledger::Records,
}
impl Model {
    pub fn edit(&self, expected_revision: u64, edits: &[Edit]) -> Result<Self, String> {
        edit::change(self, expected_revision, edits)
    }
    pub fn new(inventory: Inventory) -> Result<Self, String> {
        crate::config::validate_structure(&inventory)?;
        let ledger = match &inventory.identity_ledger {
            Some(ledger) => ledger.clone(),
            None => ledger::baseline(&inventory)?,
        };
        let records = ledger::replay(&ledger, inventory.revision)?;
        ledger::match_current(&inventory, &records)?;
        let projections = records
            .values()
            .map(|r| r.projection.clone())
            .collect::<Vec<_>>();
        let state = state_bytes(&inventory, &ledger, &projections)?;
        Ok(Self {
            inventory,
            ledger,
            projections,
            state,
            records,
        })
    }
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }
    pub fn projections(&self) -> Vec<Projection> {
        self.projections.clone()
    }
    pub fn state_bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self.state.clone())
    }
}

fn state_bytes(
    inventory: &Inventory,
    ledger: &Ledger,
    projections: &[Projection],
) -> Result<Vec<u8>, String> {
    let mut snapshot = serde_json::to_value(inventory).map_err(|e| e.to_string())?;
    snapshot["schema_version"] = 1.into();
    snapshot
        .as_object_mut()
        .unwrap()
        .remove("revision_high_water");
    snapshot["identity_ledger"] = serde_json::to_value(ledger).map_err(|e| e.to_string())?;
    let by_location: BTreeMap<_, _> = projections.iter().map(|p| (p.location, p)).collect();
    for (bus_index, bus) in inventory.buses.iter().enumerate() {
        for (index, output) in bus.outputs.iter().enumerate() {
            let projection = by_location[&Location::Output {
                bus: bus.bus,
                short_address: output.short_address,
            }];
            let item = &mut snapshot["buses"][bus_index]["outputs"][index];
            item["target"] = projection.target.clone().into();
            item["binding_revision"] = projection.binding_revision.into();
        }
        for (index, group) in bus.groups.iter().enumerate() {
            let projection = by_location[&Location::Group {
                bus: bus.bus,
                group_address: group.group_address,
            }];
            let item = &mut snapshot["buses"][bus_index]["groups"][index];
            item["target"] = projection.target.clone().into();
            item["binding_revision"] = projection.binding_revision.into();
        }
        for (index, input) in bus.inputs.iter().enumerate() {
            let projection = by_location[&Location::Input {
                bus: bus.bus,
                short_address: input.short_address,
            }];
            let item = &mut snapshot["buses"][bus_index]["inputs"][index];
            item["target"] = projection.target.clone().into();
            item["binding_revision"] = projection.binding_revision.into();
            for (j, instance) in input.instances.iter().enumerate() {
                let projection = by_location[&Location::InputInstance {
                    bus: bus.bus,
                    short_address: input.short_address,
                    instance: instance.number,
                }];
                item["instances"][j]["target"] = projection.target.clone().into();
                item["instances"][j]["binding_revision"] = projection.binding_revision.into();
            }
        }
    }
    let bytes = serde_json::to_vec(&serde_json::json!({"Inventory": snapshot}))
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err(format!(
            "complete inventory State is {} bytes; limit is {MAX_STATE_BYTES}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod tests;
