//! Checked common Part 102 values and nonexecuting DTR0/setter plans.
use super::{Destination, Level};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("DALI minimum/maximum level {0} must be within 1..254")]
    Limit(u8),
    #[error("DALI fade time code {0} must be within 0..15")]
    FadeTime(u8),
    #[error("DALI fade rate code {0} must be within 1..15")]
    FadeRate(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Configured dimming limit, before checking a fixture's physical/opposite limit.
pub struct LimitLevel(Level);
impl LimitLevel {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw == 0 {
            return Err(Error::Limit(raw));
        }
        Level::new(raw).map(Self).map_err(|_| Error::Limit(raw))
    }
    pub fn raw(self) -> u8 {
        self.0.raw()
    }
    pub fn normalized(self) -> f64 {
        self.0.normalized()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerOnLevel {
    Fixed(Level),
    /// Raw 255: restore the level active before fixture mains power was lost.
    LastActive,
}
impl PowerOnLevel {
    pub fn from_raw(raw: u8) -> Self {
        match Level::new(raw) {
            Ok(level) => Self::Fixed(level),
            Err(_) => Self::LastActive,
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Fixed(level) => level.raw(),
            Self::LastActive => 255,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemFailureLevel {
    Fixed(Level),
    /// Raw 255: leave the actual level unchanged on DALI bus failure.
    Unchanged,
}
impl SystemFailureLevel {
    pub fn from_raw(raw: u8) -> Self {
        match Level::new(raw) {
            Ok(level) => Self::Fixed(level),
            Err(_) => Self::Unchanged,
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Fixed(level) => level.raw(),
            Self::Unchanged => 255,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadeTime(u8);
impl FadeTime {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw <= 15 {
            Ok(Self(raw))
        } else {
            Err(Error::FadeTime(raw))
        }
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    /// Native nominal seconds; None for code 0, whose effective extended/fast
    /// fade is not known here. This is not evidence of a completed physical fade.
    pub fn seconds(self) -> Option<f64> {
        (self.0 != 0).then(|| 0.5 * 2.0_f64.powf(f64::from(self.0) / 2.0))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadeRate(u8);
impl FadeRate {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if (1..=15).contains(&raw) {
            Ok(Self(raw))
        } else {
            Err(Error::FadeRate(raw))
        }
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn steps_per_second(self) -> f64 {
        506.0 / 2.0_f64.powf(f64::from(self.0) / 2.0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadeSettings(u8);
impl FadeSettings {
    pub fn from_raw(raw: u8) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn time(self) -> FadeTime {
        FadeTime(self.0 >> 4)
    }
    /// Reserved rate 0 remains raw diagnostic data, without a fabricated rate.
    pub fn rate(self) -> Option<FadeRate> {
        FadeRate::new(self.0 & 15).ok()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Minimum(LimitLevel),
    Maximum(LimitLevel),
    PowerOn(PowerOnLevel),
    SystemFailure(SystemFailureLevel),
    FadeTime(FadeTime),
    FadeRate(FadeRate),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub frame: u16,
    pub repeat: bool,
}
impl Setting {
    /// Pure frame construction. Execute both steps under one bus permit, using
    /// the verified HAT repeat policy; preflight physical limits and read back.
    pub fn plan(self, destination: Destination) -> [Step; 2] {
        let (raw, opcode) = match self {
            Self::Minimum(value) => (value.raw(), 0x2b),
            Self::Maximum(value) => (value.raw(), 0x2a),
            Self::PowerOn(value) => (value.raw(), 0x2d),
            Self::SystemFailure(value) => (value.raw(), 0x2c),
            Self::FadeTime(value) => (value.raw(), 0x2e),
            Self::FadeRate(value) => (value.raw(), 0x2f),
        };
        [
            Step {
                frame: u16::from_be_bytes([0xa3, raw]),
                repeat: false,
            },
            Step {
                frame: destination.command(opcode),
                repeat: true,
            },
        ]
    }
}
#[cfg(test)]
mod tests;
