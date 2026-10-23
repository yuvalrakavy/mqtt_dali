//! Part 103 per-instance event settings. Pure plans, not an I/O executor.
//!
//! Hold the entire plan under one bus permit, including DTR scratch writes.
//! Send the final addressed step as two ordinary 24-bit sends within the native
//! 100ms window (the ATX `t`/repeat flag supports only 16-bit frames). Neither
//! successful transmission nor a plan itself proves the setting was applied.
use super::{Address, Group, Instance, InstanceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("event scheme {0} is outside 0..4")]
    Scheme(u8),
    #[error("event priority {0} is outside 2..5")]
    Priority(u8),
    #[error("event filter {0:08X} exceeds 24 bits")]
    FilterWidth(u32),
    #[error("event filter {filter:06X} has unsupported bits for instance type {kind}")]
    FilterBits { kind: u8, filter: u32 },
    #[error("no standard common-input filter interpretation for instance type {0}")]
    UnsupportedType(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventScheme {
    Instance = 0,
    Device = 1,
    DeviceInstance = 2,
    DeviceGroup = 3,
    InstanceGroup = 4,
}
impl EventScheme {
    pub fn new(raw: u8) -> Result<Self, Error> {
        Ok(match raw {
            0 => Self::Instance,
            1 => Self::Device,
            2 => Self::DeviceInstance,
            3 => Self::DeviceGroup,
            4 => Self::InstanceGroup,
            _ => return Err(Error::Scheme(raw)),
        })
    }
    pub fn raw(self) -> u8 {
        self as u8
    }
}

/// Native event priorities: 2 is highest, 5 is lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventPriority(u8);
impl EventPriority {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if !(2..=5).contains(&raw) {
            return Err(Error::Priority(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// Raw 24-bit filter; FF bytes are data. Construction makes no type claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFilter(u32);
impl EventFilter {
    pub fn new(raw: u32) -> Result<Self, Error> {
        if raw > 0xffffff {
            return Err(Error::FilterWidth(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u32 {
        self.0
    }
    /// Check only the known common Parts 301–304. Unknown types remain raw.
    pub fn validate_for(self, kind: InstanceType) -> Result<Self, Error> {
        let mask = common_filter_mask(kind).ok_or(Error::UnsupportedType(kind.raw()))?;
        if self.0 & !mask != 0 {
            return Err(Error::FilterBits {
                kind: kind.raw(),
                filter: self.0,
            });
        }
        Ok(self)
    }
}

/// Supported filter bits for common inputs; not a hardware capability probe.
pub fn common_filter_mask(kind: InstanceType) -> Option<u32> {
    Some(match kind {
        InstanceType::Button => 0xff,
        InstanceType::Absolute | InstanceType::Light => 1,
        InstanceType::Occupancy => 0x1f,
        InstanceType::Unknown(_) => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSlot {
    Primary,
    Group1,
    Group2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Enabled(bool),
    Priority(EventPriority),
    Scheme(EventScheme),
    /// Caller must apply known-type validation before exposing semantic writes.
    Filter(EventFilter),
    Group(GroupSlot, Option<Group>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub frame: u32,
    /// Send this frame twice as ordinary 24-bit sends, not using ATX's `t` flag.
    pub repeat: bool,
}

/// Build at most four steps for one checked short address and instance.
pub fn plan(address: Address, instance: Instance, setting: Setting) -> Vec<Step> {
    let mut steps = Vec::with_capacity(4);
    let mut dtr = |index: u8, value: u8| {
        steps.push(Step {
            frame: 0xc13000 | (u32::from(index) << 8) | u32::from(value),
            repeat: false,
        });
    };
    let opcode = match setting {
        Setting::Enabled(true) => 0x62,
        Setting::Enabled(false) => 0x63,
        Setting::Priority(value) => {
            dtr(0, value.raw());
            0x61
        }
        Setting::Scheme(value) => {
            dtr(0, value.raw());
            0x67
        }
        Setting::Filter(value) => {
            let bytes = value.raw().to_le_bytes();
            for index in 0..3 {
                dtr(index, bytes[usize::from(index)]);
            }
            0x68
        }
        Setting::Group(slot, group) => {
            dtr(0, group.map_or(255, Group::raw));
            match slot {
                GroupSlot::Primary => 0x64,
                GroupSlot::Group1 => 0x65,
                GroupSlot::Group2 => 0x66,
            }
        }
    };
    steps.push(Step {
        frame: address.command(instance.raw(), opcode),
        repeat: true,
    });
    steps
}

#[cfg(test)]
mod tests;
