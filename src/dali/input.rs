//! Part 103 input addresses, query frames and native Part 301–304 event data.
//! This pure codec performs no I/O and does not assign an event to a named target.

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("input short address {0} is outside 0..63")]
    Address(u8),
    #[error("input instance {0} is outside 0..31")]
    Instance(u8),
    #[error("input group {0} is outside 0..31")]
    Group(u8),
    #[error("input frame {0:08X} exceeds 24 bits")]
    FrameWidth(u32),
    #[error("reserved input event source scheme in {0:06X}")]
    SourceScheme(u32),
    #[error("event instance type {reported} conflicts with discovered type {discovered}")]
    TypeMismatch { reported: u8, discovered: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(u8);
impl Address {
    pub fn new(value: u8) -> Result<Self, Error> {
        if value > 63 {
            return Err(Error::Address(value));
        }
        Ok(Self(value))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn query(self, query: DeviceQuery) -> u32 {
        self.command(0xfe, query.opcode())
    }
    pub fn query_instance(self, instance: Instance, query: InstanceQuery) -> u32 {
        self.command(instance.raw(), query.opcode())
    }
    fn command(self, instance: u8, opcode: u8) -> u32 {
        u32::from_be_bytes([0, (self.0 << 1) | 1, instance, opcode])
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instance(u8);
impl Instance {
    pub fn new(value: u8) -> Result<Self, Error> {
        if value > 31 {
            return Err(Error::Instance(value));
        }
        Ok(Self(value))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group(u8);
impl Group {
    pub fn new(value: u8) -> Result<Self, Error> {
        if value > 31 {
            return Err(Error::Group(value));
        }
        Ok(Self(value))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceType {
    Button,
    Absolute,
    Occupancy,
    Light,
    Unknown(u8),
}
impl InstanceType {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::Button,
            2 => Self::Absolute,
            3 => Self::Occupancy,
            4 => Self::Light,
            other => Self::Unknown(other),
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Button => 1,
            Self::Absolute => 2,
            Self::Occupancy => 3,
            Self::Light => 4,
            Self::Unknown(raw) => raw,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Device {
        address: Address,
        kind: InstanceType,
    },
    DeviceInstance {
        address: Address,
        instance: Instance,
    },
    Instance {
        instance: Instance,
        kind: InstanceType,
    },
    DeviceGroup {
        group: Group,
        kind: InstanceType,
    },
    InstanceGroup {
        group: Group,
        kind: InstanceType,
    },
}
impl Source {
    pub fn instance_type(self) -> Option<InstanceType> {
        match self {
            Self::DeviceInstance { .. } => None,
            Self::Device { kind, .. }
            | Self::Instance { kind, .. }
            | Self::DeviceGroup { kind, .. }
            | Self::InstanceGroup { kind, .. } => Some(kind),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    Released,
    Pressed,
    ShortPress,
    DoublePress,
    LongPressStart,
    LongPressRepeat,
    LongPressStop,
    Free,
    Stuck,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    Presence,
    Movement,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    Button(ButtonAction),
    Absolute {
        raw_10: u16,
    },
    Occupancy {
        occupied: bool,
        movement: bool,
        repeated: bool,
        sensor: SensorKind,
    },
    Light {
        raw_10: u16,
    },
    Unknown {
        kind: InstanceType,
        information: u16,
    },
    Unresolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    raw: u32,
    source: Source,
    information: u16,
}
impl Event {
    pub fn raw(self) -> u32 {
        self.raw
    }
    pub fn source(self) -> Source {
        self.source
    }
    pub fn information(self) -> u16 {
        self.information
    }
    /// Interpret data using its explicit type, or discovery for device/instance.
    /// This does not establish a unique endpoint or infer current button state.
    pub fn payload(self, discovered: Option<InstanceType>) -> Result<Payload, Error> {
        let kind = match (self.source.instance_type(), discovered) {
            (Some(reported), Some(discovered)) if reported.raw() != discovered.raw() => {
                return Err(Error::TypeMismatch {
                    reported: reported.raw(),
                    discovered: discovered.raw(),
                });
            }
            (Some(reported), _) => Some(reported),
            (None, discovered) => discovered,
        };
        let Some(kind) = kind else {
            return Ok(Payload::Unresolved);
        };
        let unknown = Payload::Unknown {
            kind,
            information: self.information,
        };
        Ok(match kind {
            InstanceType::Button => match self.information {
                0 => Payload::Button(ButtonAction::Released),
                1 => Payload::Button(ButtonAction::Pressed),
                2 => Payload::Button(ButtonAction::ShortPress),
                5 => Payload::Button(ButtonAction::DoublePress),
                9 => Payload::Button(ButtonAction::LongPressStart),
                11 => Payload::Button(ButtonAction::LongPressRepeat),
                12 => Payload::Button(ButtonAction::LongPressStop),
                14 => Payload::Button(ButtonAction::Free),
                15 => Payload::Button(ButtonAction::Stuck),
                _ => unknown,
            },
            InstanceType::Absolute => Payload::Absolute {
                raw_10: self.information,
            },
            InstanceType::Occupancy if self.information & !0x0f == 0 => Payload::Occupancy {
                occupied: self.information & 2 != 0,
                movement: self.information & 1 != 0,
                repeated: self.information & 4 != 0,
                sensor: if self.information & 8 != 0 {
                    SensorKind::Movement
                } else {
                    SensorKind::Presence
                },
            },
            InstanceType::Light => Payload::Light {
                raw_10: self.information,
            },
            _ => unknown,
        })
    }
}
/// Commands return None. Valid native events retain all source and data bits.
pub fn decode(raw: u32) -> Result<Option<Event>, Error> {
    if raw >= 1 << 24 {
        return Err(Error::FrameWidth(raw));
    }
    if raw & (1 << 16) != 0 {
        return Ok(None);
    }
    let upper = ((raw >> 17) & 31) as u8;
    let lower = ((raw >> 10) & 31) as u8;
    let address = Address(((raw >> 17) & 63) as u8);
    let source = match (
        raw & (1 << 23) != 0,
        raw & (1 << 22) != 0,
        raw & (1 << 15) != 0,
    ) {
        (false, _, false) => Source::Device {
            address,
            kind: InstanceType::from_raw(lower),
        },
        (false, _, true) => Source::DeviceInstance {
            address,
            instance: Instance(lower),
        },
        (true, false, false) => Source::DeviceGroup {
            group: Group(upper),
            kind: InstanceType::from_raw(lower),
        },
        (true, false, true) => Source::Instance {
            instance: Instance(lower),
            kind: InstanceType::from_raw(upper),
        },
        (true, true, false) => Source::InstanceGroup {
            group: Group(upper),
            kind: InstanceType::from_raw(lower),
        },
        _ => return Err(Error::SourceScheme(raw)),
    };
    Ok(Some(Event {
        raw,
        source,
        information: (raw & 0x3ff) as u16,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceQuery {
    Status,
    Version,
    NumberOfInstances,
}
impl DeviceQuery {
    fn opcode(self) -> u8 {
        match self {
            Self::Status => 0x30,
            Self::Version => 0x34,
            Self::NumberOfInstances => 0x35,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceQuery {
    Type,
    Resolution,
    Error,
    Status,
    Enabled,
    EventScheme,
    InputValue,
    InputValueLatch,
    EventFilterLow,
    EventFilterMiddle,
    EventFilterHigh,
}
impl InstanceQuery {
    fn opcode(self) -> u8 {
        match self {
            Self::Type => 0x80,
            Self::Resolution => 0x81,
            Self::Error => 0x82,
            Self::Status => 0x83,
            Self::Enabled => 0x86,
            Self::EventScheme => 0x8b,
            Self::InputValue => 0x8c,
            Self::InputValueLatch => 0x8d,
            Self::EventFilterLow => 0x90,
            Self::EventFilterMiddle => 0x91,
            Self::EventFilterHigh => 0x92,
        }
    }
}

#[cfg(test)]
mod tests;
