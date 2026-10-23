//! Part 301 button timing: pure checked values and addressed query/write plans.
//!
//! Before executing a write, the owner must establish instance type 1 and read
//! the device's short/double minimum, then call [`Value::check_minimum`]. Hold
//! the whole plan under one bus permit; transmit its final step twice within
//! 100ms as ordinary 24-bit frames. A send is not proof of actual application.
use super::{settings::Step, Address, Instance};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    Short,
    Double,
    Repeat,
    Stuck,
}
impl Timer {
    fn setting_opcode(self) -> u8 {
        match self {
            Self::Short => 0,
            Self::Double => 1,
            Self::Repeat => 2,
            Self::Stuck => 3,
        }
    }
    pub fn query(self, address: Address, instance: Instance) -> u32 {
        let opcode = match self {
            Self::Short => 0x0a,
            Self::Double => 0x0c,
            Self::Repeat => 0x0e,
            Self::Stuck => 0x0f,
        };
        address.command(instance.raw(), opcode)
    }
    /// Only short/double timers have a native minimum query.
    pub fn minimum_query(self, address: Address, instance: Instance) -> Option<u32> {
        let opcode = match self {
            Self::Short => 0x0b,
            Self::Double => 0x0d,
            Self::Repeat | Self::Stuck => return None,
        };
        Some(address.command(instance.raw(), opcode))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("button timer {timer:?} raw value {raw} is outside its static range")]
    Range { timer: Timer, raw: u8 },
    #[error("button timer {timer:?} reported an invalid minimum {minimum}")]
    Minimum { timer: Timer, minimum: u8 },
    #[error("button timer {timer:?} raw value {raw} is below reported minimum {minimum}")]
    BelowMinimum { timer: Timer, raw: u8, minimum: u8 },
    #[error("button timer {0:?} has no native minimum query")]
    NoMinimum(Timer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value {
    timer: Timer,
    raw: u8,
}
impl Value {
    /// Static wire-range validation only; not a device capability/limit proof.
    pub fn new(timer: Timer, raw: u8) -> Result<Self, Error> {
        let valid = match timer {
            Timer::Short => raw >= 10,
            Timer::Double => raw == 0 || (10..=100).contains(&raw),
            Timer::Repeat => (5..=100).contains(&raw),
            Timer::Stuck => raw >= 5,
        };
        if !valid {
            return Err(Error::Range { timer, raw });
        }
        Ok(Self { timer, raw })
    }
    pub fn timer(self) -> Timer {
        self.timer
    }
    pub fn raw(self) -> u8 {
        self.raw
    }

    /// Validate a freshly queried device minimum before short/double writes.
    /// Double zero disables timing, but still requires a valid minimum reply.
    pub fn check_minimum(self, minimum: u8) -> Result<Self, Error> {
        let valid = match self.timer {
            Timer::Short => minimum >= 10,
            Timer::Double => (10..=100).contains(&minimum),
            timer => return Err(Error::NoMinimum(timer)),
        };
        if !valid {
            return Err(Error::Minimum {
                timer: self.timer,
                minimum,
            });
        }
        if self.raw < minimum && !(self.timer == Timer::Double && self.raw == 0) {
            return Err(Error::BelowMinimum {
                timer: self.timer,
                raw: self.raw,
                minimum,
            });
        }
        Ok(self)
    }
    /// Exact native time; double zero means disabled.
    pub fn duration(self) -> Duration {
        let unit_ms = if self.timer == Timer::Stuck { 1000 } else { 20 };
        Duration::from_millis(u64::from(self.raw) * unit_ms)
    }
    pub fn plan(self, address: Address, instance: Instance) -> [Step; 2] {
        [
            Step {
                frame: 0xc13000 | u32::from(self.raw),
                repeat: false,
            },
            Step {
                frame: address.command(instance.raw(), self.timer.setting_opcode()),
                repeat: true,
            },
        ]
    }
}

#[cfg(test)]
mod tests;
