//! Native scene storage and report plans. All DTR/report steps require one bus
//! permit. A plan describes wire operations, never confirms a physical change.
use super::{
    colour::{self, Channel, ColourType, Coordinate, Reply, Temperature, Width},
    gear::{self, Destination, ShortAddress},
};
use crate::atx::transactions::BusReply;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("scene slot {0} is outside 0..15")]
    Slot(u8),
    #[error("RGBWAF control {0} uses reserved mode bits; use None for MASK")]
    RgbwafControl(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot(u8);
impl Slot {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw > 15 {
            return Err(Error::Slot(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn recall(self, target: Destination) -> u16 {
        target.command(0x10 + self.0)
    }
    pub fn query(self, address: ShortAddress) -> u16 {
        Destination::Short(address).command(0xb0 + self.0)
    }
    /// Stores DTR brightness AND the existing temporary colour, if selected.
    /// A MASK temporary type preserves the slot's previous stored colour.
    pub fn store(self, address: ShortAddress, level: Level) -> Vec<Step> {
        vec![
            Step::Send(colour::special(0xa3, level.raw())),
            Step::Repeat(Destination::Short(address).command(0x40 + self.0)),
        ]
    }
    /// Removes both brightness and colour participation.
    pub fn remove(self, address: ShortAddress) -> Step {
        Step::Repeat(Destination::Short(address).command(0x50 + self.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Known(gear::Level),
    Mask,
}
impl Level {
    pub fn decode(raw: u8) -> Self {
        match gear::Level::new(raw) {
            Ok(value) => Self::Known(value),
            Err(_) => Self::Mask,
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Known(value) => value.raw(),
            Self::Mask => 255,
        }
    }
}
pub fn level_reply(reply: BusReply) -> Reply<Level> {
    colour::byte_reply(reply).map(Level::decode)
}

/// Repeat means a twice-sent setting under the transport's configured repeat
/// policy, not two independent asynchronous one-shot requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Send(u16),
    Repeat(u16),
    Query(u16),
}

/// Copy the current control level into DTR0 and read it. DT7 copies its virtual
/// level, while QUERY ACTUAL LEVEL reports the binary contact. FF poisons stale
/// DTR data before STORE; it cannot be a numeric capture result. Own the whole
/// plan and require the configured transport repeat policy for STORE.
pub fn capture_level(address: ShortAddress) -> [Step; 3] {
    let target = Destination::Short(address);
    [
        Step::Send(colour::special(0xa3, 255)),
        Step::Repeat(target.command(0x21)),
        Step::Query(target.command(0x98)),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportType {
    Known(ColourType),
    Mask,
    Unsupported(u8),
}
impl ReportType {
    pub fn decode(raw: u8) -> Self {
        match raw {
            0x10 => Self::Known(ColourType::Xy),
            0x20 => Self::Known(ColourType::Temperature),
            0x40 => Self::Known(ColourType::PrimaryN),
            0x80 => Self::Known(ColourType::Rgbwaf),
            255 => Self::Mask,
            other => Self::Unsupported(other),
        }
    }
}
pub fn type_reply(reply: BusReply) -> Reply<ReportType> {
    colour::byte_reply(reply).map(ReportType::decode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportValue {
    X,
    Y,
    Temperature,
    Red,
    Green,
    Blue,
    White,
    Amber,
    Free,
    RgbwafControl,
    Type,
}
impl ReportValue {
    pub fn width(self) -> Width {
        match self {
            Self::X | Self::Y | Self::Temperature => Width::Word,
            _ => Width::Byte,
        }
    }
    pub fn selector(self) -> u8 {
        match self {
            Self::X => 224,
            Self::Y => 225,
            Self::Temperature => 226,
            Self::Red => 233,
            Self::Green => 234,
            Self::Blue => 235,
            Self::White => 236,
            Self::Amber => 237,
            Self::Free => 238,
            Self::RgbwafControl => 239,
            Self::Type => 240,
        }
    }
    /// The first query selects the stored scene report. Never refresh actual
    /// level between that query and the report: it overwrites the report.
    /// Stop on any missing/collided query rather than pairing stale bytes.
    pub fn plan(self, address: ShortAddress, scene: Slot) -> Vec<Step> {
        let target = Destination::Short(address);
        let mut steps = vec![
            Step::Query(scene.query(address)),
            Step::Send(colour::special(0xa3, self.selector())),
            Step::Send(0xc108),
            Step::Query(target.command(0xfa)),
        ];
        if self.width() == Width::Word {
            steps.push(Step::Query(target.command(0x98)));
        }
        steps
    }
}

/// Copies the previously selected report, including MASK type/attributes.
/// Select it explicitly with QuerySceneLevel or QueryActualLevel first.
pub fn copy_report_to_temporary(address: ShortAddress) -> [u16; 2] {
    [0xc108, Destination::Short(address).command(0xee)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbwafControl(u8);
impl RgbwafControl {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw & 0xc0 == 0xc0 {
            return Err(Error::RgbwafControl(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// Stored attributes may all be MASK while their family is still selected.
/// MASK colour TYPE is separate; copy a known masked report for that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    Temperature(Option<Temperature>),
    Xy {
        x: Option<Coordinate>,
        y: Option<Coordinate>,
    },
    Rgbwaf {
        channels: [Option<Channel>; 6],
        control: Option<RgbwafControl>,
    },
}
impl Colour {
    /// Stages only. Neither master brightness nor actual colour is activated.
    pub fn staged_frames(self, address: ShortAddress) -> Vec<u16> {
        let target = Destination::Short(address);
        let mut frames = Vec::new();
        match self {
            Self::Temperature(value) => colour::load_word(
                &mut frames,
                target,
                value.map_or(65535, Temperature::raw),
                0xe7,
            ),
            Self::Xy { x, y } => {
                colour::load_word(&mut frames, target, x.map_or(65535, Coordinate::raw), 0xe0);
                colour::load_word(&mut frames, target, y.map_or(65535, Coordinate::raw), 0xe1);
            }
            Self::Rgbwaf { channels, control } => {
                let values = channels.map(|value| value.map_or(255, Channel::raw));
                for (triple, opcode) in [(&values[..3], 0xeb), (&values[3..], 0xec)] {
                    frames.extend([
                        colour::special(0xa3, triple[0]),
                        colour::special(0xc3, triple[1]),
                        colour::special(0xc5, triple[2]),
                    ]);
                    colour::extended(&mut frames, target, opcode);
                }
                frames.push(colour::special(
                    0xa3,
                    control.map_or(255, RgbwafControl::raw),
                ));
                colour::extended(&mut frames, target, 0xed);
            }
        }
        frames
    }
}

#[cfg(test)]
mod tests;
