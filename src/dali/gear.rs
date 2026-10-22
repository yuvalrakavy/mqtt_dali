//! Part 102 output values and frames. Creating a frame performs no I/O.
//!
//! Normalized brightness scales the DALI control code, not luminous flux.
//! Queries accept only short addresses. No response or collision is never a
//! decoded fixture value; see `docs/dali-output-contract.md` for integration limits.
use crate::atx::transactions::BusReply;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("DALI output short address {0} is outside 0..63")]
    ShortAddress(u8),
    #[error("DALI output group address {0} is outside 0..15")]
    GroupAddress(u8),
    #[error("DALI level {0} is outside 0..254; 255 is MASK")]
    Level(u8),
    #[error("brightness must be finite and within 0..1")]
    Brightness,
    #[error("an output intent needs power or brightness")]
    EmptyIntent,
    #[error("power and brightness contradict each other")]
    ContradictoryIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortAddress(u8);
impl ShortAddress {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw > 63 {
            return Err(Error::ShortAddress(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn query(self, query: Query) -> u16 {
        Destination::Short(self).command(query.opcode())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupAddress(u8);
impl GroupAddress {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw > 15 {
            return Err(Error::GroupAddress(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Short(ShortAddress),
    Group(GroupAddress),
    Broadcast,
}
impl Destination {
    fn address(self) -> u8 {
        match self {
            Self::Short(address) => address.0 << 1,
            Self::Group(address) => 0x80 | (address.0 << 1),
            Self::Broadcast => 0xfe,
        }
    }
    pub fn set_level(self, level: Level) -> u16 {
        u16::from_be_bytes([self.address(), level.raw()])
    }
    pub(crate) fn command(self, opcode: u8) -> u16 {
        u16::from_be_bytes([self.address() | 1, opcode])
    }
    /// Immediate OFF; unlike DAPC it does not use the configured fade time.
    pub fn off(self) -> u16 {
        self.command(0x00)
    }
    pub fn recall_maximum(self) -> u16 {
        self.command(0x05)
    }
}

/// Ordinary control level. MASK cannot be constructed as a `Level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level(u8);
impl Level {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw == 255 {
            return Err(Error::Level(raw));
        }
        Ok(Self(raw))
    }
    /// Positive sub-step requests remain on; the device can further clamp to its
    /// configured minimum/maximum. This is not a photometric conversion.
    pub fn from_normalized(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Brightness);
        }
        if value == 0.0 {
            return Ok(Self(0));
        }
        Ok(Self(((value * 254.0).round() as u8).max(1)))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn normalized(self) -> f64 {
        f64::from(self.0) / 254.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drive {
    Off,
    Maximum,
    Level(Level),
}

/// Fully validated atomic power/brightness intent; it never partially plans a
/// contradictory pair. Execution and JSON parsing belong to the service layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intent(Drive);
impl Intent {
    pub fn new(power: Option<bool>, brightness: Option<f64>) -> Result<Self, Error> {
        let level = brightness.map(Level::from_normalized).transpose()?;
        let drive = match (power, level) {
            (None, None) => return Err(Error::EmptyIntent),
            (Some(false), Some(level)) if level.raw() > 0 => {
                return Err(Error::ContradictoryIntent)
            }
            (Some(true), Some(level)) if level.raw() == 0 => {
                return Err(Error::ContradictoryIntent)
            }
            (Some(false), _) => Drive::Off,
            (_, Some(level)) => Drive::Level(level),
            (Some(true), None) => Drive::Maximum,
        };
        Ok(Self(drive))
    }
    pub fn frame(self, destination: Destination) -> u16 {
        match self.0 {
            Drive::Off => destination.off(),
            Drive::Maximum => destination.recall_maximum(),
            Drive::Level(level) => destination.set_level(level),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Status,
    ActualLevel,
    MinimumLevel,
    MaximumLevel,
    PhysicalMinimum,
    Version,
    DeviceType,
}
impl Query {
    fn opcode(self) -> u8 {
        match self {
            Self::Status => 0x90,
            Self::Version => 0x97,
            Self::DeviceType => 0x99,
            Self::PhysicalMinimum => 0x9a,
            Self::ActualLevel => 0xa0,
            Self::MaximumLevel => 0xa1,
            Self::MinimumLevel => 0xa2,
        }
    }
    /// Decode only a reply already correlated by the ATX transaction owner.
    /// Arbitrary monitor records do not establish this query's response.
    pub fn decode(self, reply: BusReply) -> QueryReply {
        let raw = match reply {
            BusReply::NoResponse => return QueryReply::NoResponse,
            BusReply::ReceiveCollision(code) => return QueryReply::ReceiveCollision(code),
            BusReply::TransmitCollision(code) => return QueryReply::TransmitCollision(code),
            BusReply::Value(raw) => raw,
        };
        let value = match self {
            Self::Status => QueryValue::Status(Status(raw)),
            Self::ActualLevel => QueryValue::ActualLevel(LevelValue::decode(raw)),
            Self::MinimumLevel => QueryValue::MinimumLevel(LevelValue::decode(raw)),
            Self::MaximumLevel => QueryValue::MaximumLevel(LevelValue::decode(raw)),
            Self::PhysicalMinimum => QueryValue::PhysicalMinimum(LevelValue::decode(raw)),
            Self::Version => QueryValue::Version(raw),
            Self::DeviceType => QueryValue::DeviceType(match raw {
                254 => DeviceType::None,
                255 => DeviceType::Multiple,
                other => DeviceType::Specific(other),
            }),
        };
        QueryReply::Value(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryReply {
    Value(QueryValue),
    NoResponse,
    ReceiveCollision(Option<u8>),
    TransmitCollision(Option<u8>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryValue {
    Status(Status),
    ActualLevel(LevelValue),
    MinimumLevel(LevelValue),
    MaximumLevel(LevelValue),
    PhysicalMinimum(LevelValue),
    Version(u8),
    DeviceType(DeviceType),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelValue {
    Known(Level),
    UnavailableMask,
}
impl LevelValue {
    fn decode(raw: u8) -> Self {
        match raw {
            255 => Self::UnavailableMask,
            value => Self::Known(Level(value)),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Specific(u8),
    None,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status(u8);
impl Status {
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn gear_failure(self) -> bool {
        self.0 & 0x01 != 0
    }
    pub fn lamp_failure(self) -> bool {
        self.0 & 0x02 != 0
    }
    pub fn lamp_on(self) -> bool {
        self.0 & 0x04 != 0
    }
    pub fn limit_error(self) -> bool {
        self.0 & 0x08 != 0
    }
    /// Bit 4 set means a fade is running; it does not mean fade complete.
    pub fn fade_running(self) -> bool {
        self.0 & 0x10 != 0
    }
    pub fn reset_state(self) -> bool {
        self.0 & 0x20 != 0
    }
    pub fn missing_short_address(self) -> bool {
        self.0 & 0x40 != 0
    }
    pub fn power_failure(self) -> bool {
        self.0 & 0x80 != 0
    }
}

#[cfg(test)]
mod tests;
