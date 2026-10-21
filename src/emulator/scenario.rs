use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClockMode {
    #[default]
    Realtime,
    Frozen,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    OneTerminal,
    TwoTerminals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub version: u8,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub clock: ClockMode,
    #[serde(default)]
    pub repeat: Repeat,
    pub buses: Vec<Bus>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bus {
    pub number: u8,
    #[serde(default = "powered")]
    pub powered: bool,
    #[serde(default)]
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub inputs: Vec<Input>,
}
fn powered() -> bool {
    true
}
fn scenes() -> Vec<u8> {
    vec![255; 16]
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub id: String,
    pub short_address: Option<u8>,
    #[serde(default)]
    pub random_address: Option<u32>,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub groups: u16,
    #[serde(default = "scenes")]
    pub scenes: Vec<u8>,
    #[serde(default)]
    pub device_type: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub id: String,
    pub short_address: Option<u8>,
    pub instances: Vec<u8>,
}
impl Scenario {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.buses.is_empty() || self.buses.len() > 4 {
            return Err("scenario requires version 1 and 1–4 buses".into());
        }
        let mut numbers = HashSet::new();
        let mut ids = HashSet::new();
        for bus in &self.buses {
            if bus.number > 3
                || !numbers.insert(bus.number)
                || bus.outputs.len() > 64
                || bus.inputs.len() > 64
            {
                return Err(
                    "invalid/duplicate bus or more than 64 fixtures in an address space".into(),
                );
            }
            for (id, short_address) in bus
                .outputs
                .iter()
                .map(|o| (&o.id, o.short_address))
                .chain(bus.inputs.iter().map(|i| (&i.id, i.short_address)))
            {
                if id.is_empty()
                    || id.len() > 128
                    || !ids.insert(id)
                    || short_address.is_some_and(|a| a > 63)
                {
                    return Err("invalid/duplicate fixture ID or short address".into());
                }
            }
            for output in &bus.outputs {
                if output.level == 255
                    || output.scenes.len() != 16
                    || output.random_address.is_some_and(|a| a > 0xffffff)
                {
                    return Err("invalid output level, scenes or random address".into());
                }
            }
            if bus.inputs.iter().any(|i| {
                i.instances.is_empty()
                    || i.instances.len() > 32
                    || i.instances.iter().any(|t| *t > 31)
            }) {
                return Err("input requires 1–32 valid instance types".into());
            }
        }
        Ok(())
    }
}
