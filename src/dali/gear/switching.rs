//! Checked Part208 threshold plans and correlated raw diagnostics. No I/O.
use super::{settings::Step, Destination, Level, ShortAddress};
use crate::atx::transactions::BusReply;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("DT7 upward threshold {0} must be within 1..255; 255 disables it")]
pub struct Error(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threshold {
    Level(Level),
    Disabled,
}
impl Threshold {
    pub fn from_raw(raw: u8) -> Self {
        match Level::new(raw) {
            Ok(value) => Self::Level(value),
            Err(_) => Self::Disabled,
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Level(value) => value.raw(),
            Self::Disabled => 255,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpThreshold(Threshold);
impl UpThreshold {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw == 0 {
            return Err(Error(raw));
        }
        Ok(Self(Threshold::from_raw(raw)))
    }
    pub fn value(self) -> Threshold {
        self.0
    }
    pub fn raw(self) -> u8 {
        self.0.raw()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    UpOn(UpThreshold),
    UpOff(UpThreshold),
    DownOn(Threshold),
    DownOff(Threshold),
}
impl Setting {
    /// DTR0 affects all gear. Keep these steps under one physical permit and
    /// repeat the final setter with the verified HAT profile. Do not insert a
    /// second enable between the two copies. Capability/readback is caller-owned.
    pub fn plan(self, destination: Destination) -> [Step; 3] {
        let (raw, opcode) = match self {
            Self::UpOn(value) => (value.raw(), 0xe1),
            Self::UpOff(value) => (value.raw(), 0xe2),
            Self::DownOn(value) => (value.raw(), 0xe3),
            Self::DownOff(value) => (value.raw(), 0xe4),
        };
        [
            Step {
                frame: u16::from_be_bytes([0xa3, raw]),
                repeat: false,
            },
            Step {
                frame: 0xc107,
                repeat: false,
            },
            Step {
                frame: destination.command(opcode),
                repeat: true,
            },
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features(u8);
impl Features {
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn adjustable_thresholds(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn physical_selection(self) -> bool {
        self.0 & 128 != 0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GearType(u8);
impl GearType {
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn electronic_switch(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn make_contact(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn break_contact(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn voltage_protection(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn inrush_limiting(self) -> bool {
        self.0 & 16 != 0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Features,
    SwitchStatus,
    UpOn,
    UpOff,
    DownOn,
    DownOff,
    GearType,
    ExtendedVersion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Features(Features),
    /// Raw diagnostic byte; it does not establish current contact position.
    SwitchStatus(u8),
    UpOn(UpThreshold),
    UpOff(UpThreshold),
    DownOn(Threshold),
    DownOff(Threshold),
    GearType(GearType),
    ExtendedVersion(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Value(Value),
    InvalidValue { query: Query, raw: u8 },
    NoResponse,
    ReceiveCollision(Option<u8>),
    TransmitCollision(Option<u8>),
}
impl Query {
    /// Send ENABLE DEVICE TYPE7 once, then query this individual address. A
    /// group/broadcast response cannot establish an individual device value.
    pub fn plan(self, address: ShortAddress) -> [u16; 2] {
        let opcode = match self {
            Self::Features => 0xf0,
            Self::SwitchStatus => 0xf1,
            Self::UpOn => 0xf2,
            Self::UpOff => 0xf3,
            Self::DownOn => 0xf4,
            Self::DownOff => 0xf5,
            Self::GearType => 0xf7,
            Self::ExtendedVersion => 0xff,
        };
        [0xc107, Destination::Short(address).command(opcode)]
    }
    /// Only decode a response already correlated by the transaction owner.
    pub fn decode(self, reply: BusReply) -> Reply {
        let raw = match reply {
            BusReply::NoResponse => return Reply::NoResponse,
            BusReply::ReceiveCollision(code) => return Reply::ReceiveCollision(code),
            BusReply::TransmitCollision(code) => return Reply::TransmitCollision(code),
            BusReply::Value(raw) => raw,
        };
        let value = match self {
            Self::Features => Value::Features(Features(raw)),
            Self::SwitchStatus => Value::SwitchStatus(raw),
            Self::UpOn | Self::UpOff => {
                let Ok(value) = UpThreshold::new(raw) else {
                    return Reply::InvalidValue { query: self, raw };
                };
                if self == Self::UpOn {
                    Value::UpOn(value)
                } else {
                    Value::UpOff(value)
                }
            }
            Self::DownOn => Value::DownOn(Threshold::from_raw(raw)),
            Self::DownOff => Value::DownOff(Threshold::from_raw(raw)),
            Self::GearType => Value::GearType(GearType(raw)),
            Self::ExtendedVersion => Value::ExtendedVersion(raw),
        };
        Reply::Value(value)
    }
}
#[cfg(test)]
mod tests;
