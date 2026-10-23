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
    #[serde(default, skip_serializing_if = "is_false")]
    pub lamp_failure: bool,
    #[serde(default)]
    pub groups: u16,
    #[serde(default = "scenes")]
    pub scenes: Vec<u8>,
    #[serde(default)]
    pub device_type: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<super::memory::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switching: Option<Switching>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<GearSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_memory: Option<super::colour::Memory>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GearSettings {
    pub min_level: u8,
    pub max_level: u8,
    pub power_on_level: u8,
    pub failure_level: u8,
    pub fade: u8,
    pub virtual_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switching {
    #[serde(default)]
    pub adjustable_thresholds: bool,
    #[serde(default = "switching_thresholds")]
    pub thresholds: [u8; 4],
    #[serde(default = "relay_type")]
    pub gear_type: u8,
}
fn switching_thresholds() -> [u8; 4] {
    [1, 255, 255, 0]
}
fn relay_type() -> u8 {
    2
}
impl Default for Switching {
    fn default() -> Self {
        Self {
            adjustable_thresholds: false,
            thresholds: switching_thresholds(),
            gear_type: relay_type(),
        }
    }
}
impl Switching {
    fn validate(&self) -> Result<(), String> {
        if self.thresholds[0] == 0 || self.thresholds[1] == 0 {
            return Err("switching upward thresholds require 1..255".into());
        }
        if !self.adjustable_thresholds && self.thresholds != switching_thresholds() {
            return Err("fixed switching profile requires default thresholds".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColourType {
    Xy,
    Temperature,
    Rgbwaf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colour {
    pub active: ColourType,
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_activation: bool,
    pub xy: Option<[u16; 2]>,
    pub temperature: Option<Temperature>,
    pub rgbwaf: Option<Vec<u8>>,
}
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Temperature {
    pub value: u16,
    pub coolest: u16,
    pub warmest: u16,
    pub physical_coolest: u16,
    pub physical_warmest: u16,
}
impl Colour {
    fn validate(&self) -> Result<(), String> {
        let active_supported = match self.active {
            ColourType::Xy => self.xy.is_some(),
            ColourType::Temperature => self.temperature.is_some(),
            ColourType::Rgbwaf => self.rgbwaf.is_some(),
        };
        if !active_supported
            || self.xy.is_some_and(|pair| pair.contains(&65535))
            || self.rgbwaf.as_ref().is_some_and(|channels| {
                channels.is_empty() || channels.len() > 6 || channels.contains(&255)
            })
            || self.temperature.as_ref().is_some_and(|tc| {
                tc.physical_coolest == 0
                    || tc.physical_warmest == 65535
                    || tc.physical_coolest > tc.coolest
                    || tc.coolest > tc.value
                    || tc.value > tc.warmest
                    || tc.warmest > tc.physical_warmest
            })
        {
            return Err(
                "invalid colour representation, value or ordered temperature limits".into(),
            );
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub id: String,
    pub short_address: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_address: Option<u32>,
    pub instances: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<super::memory::Profile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<InputValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback: Vec<Feedback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "initial_settings")]
    pub settings: Option<Vec<super::input::Settings>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_physical: Option<Vec<PhysicalStart>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalStart {
    pub pressed: bool,
    pub raw_10: u16,
    pub occupied: bool,
    pub movement: bool,
}
/// Opt-in visible on/off feedback, independent from input event configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    /// None selects device-level feedback; otherwise an existing instance.
    pub instance: Option<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputValue {
    pub instance: u8,
    pub resolution: u8,
    /// Full value response bytes, including low repeated padding bits.
    pub bytes: Vec<u8>,
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
                if let Some(memory) = &output.memory {
                    memory.validate()?;
                }
                if let Some(s) = &output.settings {
                    if s.min_level == 0
                        || s.min_level > s.max_level
                        || s.max_level > 254
                        || s.virtual_level > 254
                    {
                        return Err("invalid native gear limits or virtual level".into());
                    }
                }
                if let Some(memory) = &output.colour_memory {
                    memory.validate(
                        output
                            .colour
                            .as_ref()
                            .ok_or("colour memory requires a colour profile")?,
                    )?;
                }
                if output.device_type == 7 && !matches!(output.level, 0 | 254) {
                    return Err("switching initial actual level must be 0 or 254".into());
                }
                if let Some(switching) = &output.switching {
                    if output.device_type != 7 {
                        return Err("switching profile requires device type 7".into());
                    }
                    switching.validate()?;
                }
                if let Some(colour) = &output.colour {
                    if output.device_type != 8 {
                        return Err("colour profile requires device type 8".into());
                    }
                    colour.validate()?;
                }
                if output.level == 255
                    || output.scenes.len() != 16
                    || output.random_address.is_some_and(|a| a > 0xffffff)
                {
                    return Err("invalid output level, scenes or random address".into());
                }
            }
            if bus
                .inputs
                .iter()
                .any(|i| i.instances.len() > 32 || i.instances.iter().any(|t| *t > 31))
            {
                return Err("input requires at most 32 valid instance types".into());
            }
            for input in &bus.inputs {
                if let Some(memory) = &input.memory {
                    memory.validate()?;
                    if memory.banks.iter().any(|bank| bank.latch) {
                        return Err("input modules do not support output memory latches".into());
                    }
                }
                if let Some(states) = &input.initial_physical {
                    if states.len() != input.instances.len()
                        || states.iter().any(|s| s.raw_10 > 1023)
                    {
                        return Err("initial physical state must cover every instance with valid raw10 values".into());
                    }
                }
                if let Some(settings) = &input.settings {
                    if settings.len() != input.instances.len() {
                        return Err("native input settings must cover every instance".into());
                    }
                    for (settings, kind) in settings.iter().zip(&input.instances) {
                        settings.validate(*kind)?;
                    }
                }
                if input.random_address.is_some_and(|a| a > 0xffffff) {
                    return Err("input random address exceeds 24 bits".into());
                }
                let mut feedback_scopes = HashSet::new();
                for profile in &input.feedback {
                    if !feedback_scopes.insert(profile.instance)
                        || profile
                            .instance
                            .is_some_and(|i| usize::from(i) >= input.instances.len())
                    {
                        return Err("invalid/duplicate feedback scope: select device or an existing instance".into());
                    }
                }
                let mut instances = HashSet::new();
                for value in &input.values {
                    if usize::from(value.instance) >= input.instances.len()
                        || !instances.insert(value.instance)
                        || value.resolution == 0
                        || value.bytes.len() != usize::from(value.resolution).div_ceil(8)
                    {
                        return Err(
                            "invalid/duplicate input value profile, resolution or byte count"
                                .into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
