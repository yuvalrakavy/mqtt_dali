//! Independent physical value/latch model; no bridge input codec dependencies.
use super::scenario::Input;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod button;
mod feedback;
pub mod physical;
mod sensor;

#[derive(Serialize)]
pub(super) struct State {
    #[serde(flatten)]
    pub config: Input,
    dtr0: u8,
    dtr1: u8,
    dtr2: u8,
    settings: Vec<Settings>,
    physical: Vec<physical::Physical>,
    // Distinct from flattened scenario `feedback`; omit for legacy profiles.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    feedback_state: Vec<feedback::State>,
    #[serde(skip)]
    latches: BTreeMap<u8, (Vec<u8>, usize)>,
    initialize_until: Option<u64>,
    randomise_ready_at: Option<u64>,
    withdrawn: bool,
    search: u32,
}

/// Deliberate emulator startup values, not universal device reset defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    enabled: bool,
    priority: u8,
    scheme: u8,
    filter: u32,
    groups: [u8; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    button_timers: Option<button::Timers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensor_settings: Option<sensor::Profile>,
}
impl Settings {
    pub(crate) fn validate(&self, kind: u8) -> Result<(), String> {
        if !(2..=5).contains(&self.priority)
            || self.scheme > 4
            || self.filter & !filter_mask(kind) != 0
            || self.groups.iter().any(|g| *g > 31 && *g != 255)
        {
            return Err("invalid native input event settings".into());
        }
        if self.button_timers.is_some() != (kind == 1)
            || self.sensor_settings.is_some() != matches!(kind, 2..=4)
        {
            return Err("input settings family does not match instance type".into());
        }
        if let Some(t) = &self.button_timers {
            t.validate()?;
        }
        if let Some(s) = &self.sensor_settings {
            s.validate(kind)?;
        }
        Ok(())
    }
}
fn filter_mask(kind: u8) -> u32 {
    match kind {
        1 => 0xff,
        2 | 4 => 1,
        3 => 0x1f,
        _ => 0xffffff,
    }
}
impl State {
    pub fn new(config: Input) -> Self {
        let settings = config.settings.clone().unwrap_or_else(|| {
            config
                .instances
                .iter()
                .map(|kind| Settings {
                    enabled: true,
                    priority: 4,
                    scheme: 2,
                    filter: if matches!(kind, 1..=4) {
                        filter_mask(*kind)
                    } else {
                        0
                    },
                    groups: [255; 3],
                    button_timers: (*kind == 1).then(button::Timers::new),
                    sensor_settings: sensor::Profile::new(*kind),
                })
                .collect()
        });
        let feedback_state = config
            .feedback
            .iter()
            .map(|p| feedback::State::new(p.instance))
            .collect();
        Self {
            physical: (0..config.instances.len())
                .map(|i| physical::Physical::from_config(&config, i))
                .collect(),
            config,
            dtr0: 0,
            dtr1: 0,
            dtr2: 0,
            settings,
            feedback_state,
            latches: BTreeMap::new(),
            initialize_until: None,
            randomise_ready_at: None,
            withdrawn: false,
            search: 0xffffff,
        }
    }
    pub fn captured_config(&self) -> Input {
        let mut config = self.config.clone();
        config.settings = Some(self.settings.clone());
        config.initial_physical = Some(self.physical.iter().map(|p| p.capture()).collect());
        config
    }
    pub fn advance_to(&mut self, now: u64) {
        if self.initialize_until.is_some_and(|until| now >= until) {
            self.initialize_until = None;
        }
    }
    /// Independent raw Part 103 commands. Repeat acceptance belongs to the bus.
    pub fn special(&mut self, command: u8, param: u8, now: u64, random: u32) -> Option<u8> {
        let eligible = self.initialize_until.is_some() && !self.withdrawn;
        let ready = self.randomise_ready_at.is_none_or(|at| now >= at);
        let exact = eligible && ready && self.config.random_address == Some(self.search);
        match (command, param) {
            (0x30, value) => self.dtr0 = value,
            (0x31, value) => self.dtr1 = value,
            (0x32, value) => self.dtr2 = value,
            (0, 0) => {
                self.initialize_until = None;
                self.withdrawn = false;
                self.randomise_ready_at = None;
            }
            (1, selector) => {
                if selector == 255
                    || (selector == 127 && self.config.short_address.is_none())
                    || (selector <= 63 && self.config.short_address == Some(selector))
                {
                    self.initialize_until = Some(now.saturating_add(900_000));
                    self.withdrawn = false;
                }
            }
            (2, 0) => {
                if self.initialize_until.is_some() {
                    self.config.random_address = Some(random);
                    self.randomise_ready_at = Some(now.saturating_add(200));
                }
            }
            (3, 0)
                if eligible
                    && ready
                    && self.config.random_address.is_some_and(|a| a <= self.search) =>
            {
                return Some(255)
            }
            (4, 0) if exact => self.withdrawn = true,
            (5, value) => self.search = (self.search & 0xffff) | (u32::from(value) << 16),
            (6, value) => self.search = (self.search & 0xff00ff) | (u32::from(value) << 8),
            (7, value) => self.search = (self.search & 0xffff00) | u32::from(value),
            (8, address) if exact && (address <= 63 || address == 255) => {
                self.config.short_address = if address == 255 { None } else { Some(address) }
            }
            (9, address)
                if exact && address <= 63 && self.config.short_address == Some(address) =>
            {
                return Some(255)
            }
            (10, 0) if exact => return Some(self.config.short_address.unwrap_or(255)),
            _ => {}
        }
        None
    }
    pub fn query(&mut self, instance: u8, opcode: u8) -> Option<u8> {
        // Feature addressing is a separate namespace from ordinary sensor
        // setters; the same opcode must never fall through to those handlers.
        if instance == 0xfc || (0x20..=0x3f).contains(&instance) {
            let scope = if instance == 0xfc {
                None
            } else {
                Some(instance & 31)
            };
            return self
                .feedback_state
                .iter_mut()
                .find(|p| p.instance == scope)?
                .command(opcode);
        }
        if opcode == 0x8e
            && (instance == 0xfe || usize::from(instance) < self.config.instances.len())
        {
            let scope = if instance == 0xfe {
                None
            } else {
                Some(instance)
            };
            return Some(if self.feedback_state.iter().any(|p| p.instance == scope) {
                32
            } else {
                254
            });
        }
        if instance == 0xfe {
            return match opcode {
                0x14 => {
                    if self.dtr0 == 255 {
                        self.config.short_address = None;
                    } else if self.dtr0 < 64 {
                        self.config.short_address = Some(self.dtr0);
                    }
                    None
                }
                0x30 => Some(0),
                0x35 => Some(self.config.instances.len() as u8),
                _ => None,
            };
        }
        let kind = *self.config.instances.get(usize::from(instance))?;
        if opcode == 0x80 {
            return Some(kind);
        }
        // Settings exist independently of a measurement value profile. The bus
        // admits repeated writes before this independent per-instance handler.
        let settings = &mut self.settings[usize::from(instance)];
        if matches!(opcode, 0..=3 | 0x0a..=0x0f) {
            return settings.button_timers.as_mut()?.command(opcode, self.dtr0);
        }
        match opcode {
            0x61 => {
                if (2..=5).contains(&self.dtr0) {
                    settings.priority = self.dtr0;
                }
                return None;
            }
            0x62 | 0x63 => {
                settings.enabled = opcode == 0x62;
                return None;
            }
            0x64..=0x66 => {
                if self.dtr0 <= 31 || self.dtr0 == 255 {
                    settings.groups[usize::from(opcode - 0x64)] = self.dtr0;
                }
                return None;
            }
            0x67 => {
                if self.dtr0 <= 4 {
                    settings.scheme = self.dtr0;
                }
                return None;
            }
            0x68 => {
                settings.filter =
                    u32::from_le_bytes([self.dtr0, self.dtr1, self.dtr2, 0]) & filter_mask(kind);
                return None;
            }
            0x84 => return Some(settings.priority),
            0x86 => return settings.enabled.then_some(255),
            0x88..=0x8a => return Some(settings.groups[usize::from(opcode - 0x88)]),
            0x8b => return Some(settings.scheme),
            0x90..=0x92 => return Some((settings.filter >> (8 * (opcode - 0x90))) as u8),
            _ => {}
        }
        if let Some(profile) = settings.sensor_settings.as_mut() {
            if let Some(reply) = profile.command(opcode, self.dtr0) {
                return reply;
            }
        }
        let value = self.config.values.iter().find(|v| v.instance == instance)?;
        match opcode {
            0x81 => Some(value.resolution),
            0x8c => {
                let bytes = value.bytes.clone();
                let first = bytes.first().copied();
                self.latches.insert(instance, (bytes, 1));
                first
            }
            0x8d => {
                let (bytes, cursor) = self.latches.get_mut(&instance)?;
                let byte = bytes.get(*cursor).copied();
                if byte.is_some() {
                    *cursor += 1;
                }
                byte
            }
            _ => None,
        }
    }
    pub fn set_value(&mut self, instance: u8, bytes: Vec<u8>) -> Result<(), String> {
        let value = self
            .config
            .values
            .iter_mut()
            .find(|v| v.instance == instance)
            .ok_or("input value profile does not exist")?;
        if bytes.len() != usize::from(value.resolution).div_ceil(8) {
            return Err("input value byte count does not match resolution".into());
        }
        value.bytes = bytes;
        Ok(())
    }
}
