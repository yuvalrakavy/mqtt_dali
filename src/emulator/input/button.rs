//! Independent Part 301 configuration profile; no production codec imports.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timers {
    pub(super) short: u8,
    short_min: u8,
    pub(super) double: u8,
    double_min: u8,
    pub(super) repeat: u8,
    pub(super) stuck: u8,
}
impl Timers {
    pub fn validate(&self) -> Result<(), String> {
        if self.short_min < 10
            || self.short < self.short_min
            || !(10..=100).contains(&self.double_min)
            || (self.double != 0 && !(self.double_min..=100).contains(&self.double))
            || !(5..=100).contains(&self.repeat)
            || self.stuck < 5
        {
            return Err("invalid native button timers".into());
        }
        Ok(())
    }
    /// Explicit emulator profile, not universal hardware defaults or reset rules.
    pub fn new() -> Self {
        Self {
            short: 25,
            short_min: 25,
            double: 0,
            double_min: 10,
            repeat: 8,
            stuck: 20,
        }
    }
    /// The bus has already enforced repeats. Invalid writes are ignored by this
    /// limited profile; real device clamping behavior still requires validation.
    pub fn command(&mut self, opcode: u8, dtr0: u8) -> Option<u8> {
        match opcode {
            0 if dtr0 >= self.short_min => self.short = dtr0,
            1 if dtr0 == 0 || (self.double_min..=100).contains(&dtr0) => self.double = dtr0,
            2 if (5..=100).contains(&dtr0) => self.repeat = dtr0,
            3 if dtr0 >= 5 => self.stuck = dtr0,
            0x0a => return Some(self.short),
            0x0b => return Some(self.short_min),
            0x0c => return Some(self.double),
            0x0d => return Some(self.double_min),
            0x0e => return Some(self.repeat),
            0x0f => return Some(self.stuck),
            _ => {}
        }
        None
    }
}
