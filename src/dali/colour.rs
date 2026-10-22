//! Pure Part 209 (DT8) values and frame plans. No I/O or capability inference.
//!
//! Execute a whole plan under the existing exclusive transaction permit: DTRs
//! and device-type selection are shared bus state. A send is not a readback.
use super::gear::{Destination, ShortAddress};
use crate::atx::transactions::BusReply;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("colour temperature must be within 1..65534 mirek")]
    Temperature,
    #[error("xy coordinate must be within 0..65534, in units of 1/65536")]
    Coordinate,
    #[error("RGBWAF channel must be within 0..254 (normalized 0..1)")]
    Channel,
    #[error("colour intent must change at least one channel or coordinate")]
    Empty,
    #[error("RGBWAF physical channel index {0} is outside 0..5")]
    ChannelIndex(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temperature(u16);
impl Temperature {
    pub fn new(raw: u16) -> Result<Self, Error> {
        if !(1..=65534).contains(&raw) {
            return Err(Error::Temperature);
        }
        Ok(Self(raw))
    }
    pub fn from_kelvin(kelvin: f64) -> Result<Self, Error> {
        let mirek = 1_000_000.0 / kelvin;
        if !kelvin.is_finite() || kelvin <= 0.0 || !(1.0..=65534.0).contains(&mirek) {
            return Err(Error::Temperature);
        }
        Self::new(mirek.round() as u16)
    }
    pub fn raw(self) -> u16 {
        self.0
    }
    pub fn kelvin(self) -> f64 {
        1_000_000.0 / f64::from(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coordinate(u16);
impl Coordinate {
    pub fn new(raw: u16) -> Result<Self, Error> {
        if raw == 65535 {
            return Err(Error::Coordinate);
        }
        Ok(Self(raw))
    }
    pub fn from_normalized(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || !(0.0..=(65534.0 / 65536.0)).contains(&value) {
            return Err(Error::Coordinate);
        }
        Self::new((value * 65536.0).round() as u16)
    }
    pub fn raw(self) -> u16 {
        self.0
    }
    pub fn normalized(self) -> f64 {
        f64::from(self.0) / 65536.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel(u8);
impl Channel {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw == 255 {
            return Err(Error::Channel);
        }
        Ok(Self(raw))
    }
    pub fn from_normalized(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Channel);
        }
        Self::new((value * 254.0).round() as u8)
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
    Temperature(Temperature),
    Xy(Option<Coordinate>, Option<Coordinate>),
    Rgbwaf([Option<Channel>; 6]),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intent(Drive);
impl Intent {
    pub fn kind(self) -> ColourType {
        match self.0 {
            Drive::Temperature(_) => ColourType::Temperature,
            Drive::Xy(_, _) => ColourType::Xy,
            Drive::Rgbwaf(_) => ColourType::Rgbwaf,
        }
    }
    pub fn channels(self) -> Option<[Option<Channel>; 6]> {
        match self.0 {
            Drive::Rgbwaf(values) => Some(values),
            _ => None,
        }
    }
    pub fn temperature(value: Temperature) -> Self {
        Self(Drive::Temperature(value))
    }
    pub fn xy(x: Coordinate, y: Coordinate) -> Self {
        Self(Drive::Xy(Some(x), Some(y)))
    }
    /// An omitted coordinate is explicitly loaded as MASK before ACTIVATE.
    pub fn xy_parts(x: Option<Coordinate>, y: Option<Coordinate>) -> Result<Self, Error> {
        if x.is_none() && y.is_none() {
            return Err(Error::Empty);
        }
        Ok(Self(Drive::Xy(x, y)))
    }
    /// Channel order is red, green, blue, white, amber, free. None is MASK.
    pub fn rgbwaf(channels: [Option<Channel>; 6]) -> Result<Self, Error> {
        if channels.iter().all(Option::is_none) {
            return Err(Error::Empty);
        }
        Ok(Self(Drive::Rgbwaf(channels)))
    }
    /// Includes ACTIVATE, but no power/brightness or control-mode change.
    pub fn frames(self, target: Destination) -> Vec<u16> {
        let mut frames = self.staged_frames(target);
        extended(&mut frames, target, 0xe2);
        frames
    }
    /// Only stages temporary values. Store or ACTIVATE must follow explicitly.
    pub fn staged_frames(self, target: Destination) -> Vec<u16> {
        let mut frames = Vec::with_capacity(12);
        match self.0 {
            Drive::Temperature(value) => load_word(&mut frames, target, value.raw(), 0xe7),
            Drive::Xy(x, y) => {
                load_word(&mut frames, target, x.map_or(65535, Coordinate::raw), 0xe0);
                load_word(&mut frames, target, y.map_or(65535, Coordinate::raw), 0xe1);
            }
            Drive::Rgbwaf(channels) => {
                let values = channels.map(|channel| channel.map_or(255, Channel::raw));
                for (triple, opcode) in [(&values[..3], 0xeb), (&values[3..], 0xec)] {
                    frames.extend([
                        special(0xa3, triple[0]),
                        special(0xc3, triple[1]),
                        special(0xc5, triple[2]),
                    ]);
                    extended(&mut frames, target, opcode);
                }
            }
        }
        frames
    }
}

pub(super) fn special(opcode: u8, value: u8) -> u16 {
    u16::from_be_bytes([opcode, value])
}
pub(super) fn extended(frames: &mut Vec<u16>, target: Destination, opcode: u8) {
    frames.extend([0xc108, target.command(opcode)]);
}
pub(super) fn load_word(frames: &mut Vec<u16>, target: Destination, value: u16, opcode: u8) {
    let [low, high] = value.to_le_bytes();
    frames.extend([special(0xa3, low), special(0xc3, high)]);
    extended(frames, target, opcode);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Send(u16),
    Query(u16),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    GearFeatures,
    Status,
    TypeFeatures,
    RgbwafControl,
    ExtendedVersion,
}
impl Query {
    pub fn plan(self, address: ShortAddress) -> Vec<Step> {
        let opcode = match self {
            Self::GearFeatures => 0xf7,
            Self::Status => 0xf8,
            Self::TypeFeatures => 0xf9,
            Self::RgbwafControl => 0xfb,
            Self::ExtendedVersion => 0xff,
        };
        vec![
            Step::Send(0xc108),
            Step::Query(Destination::Short(address).command(opcode)),
        ]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Byte,
    Word,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueQuery {
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
    Coolest,
    PhysicalCoolest,
    Warmest,
    PhysicalWarmest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelIndex(u8);
impl ChannelIndex {
    pub fn new(raw: u8) -> Result<Self, Error> {
        if raw > 5 {
            return Err(Error::ChannelIndex(raw));
        }
        Ok(Self(raw))
    }
    pub fn query(self, address: ShortAddress) -> Vec<Step> {
        vec![
            Step::Send(special(0xa3, self.0)),
            Step::Send(0xc108),
            Step::Query(Destination::Short(address).command(0xfc)),
        ]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedColour {
    Unassigned,
    Red,
    Green,
    Blue,
    White,
    Amber,
    Free,
    Mask,
    Unknown(u8),
}
impl AssignedColour {
    pub fn decode(raw: u8) -> Self {
        match raw {
            0 => Self::Unassigned,
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Blue,
            4 => Self::White,
            5 => Self::Amber,
            6 => Self::Free,
            255 => Self::Mask,
            other => Self::Unknown(other),
        }
    }
    /// Logical colour slot, independent of the physical output channel queried.
    pub fn slot(self) -> Option<usize> {
        match self {
            Self::Red => Some(0),
            Self::Green => Some(1),
            Self::Blue => Some(2),
            Self::White => Some(3),
            Self::Amber => Some(4),
            Self::Free => Some(5),
            _ => None,
        }
    }
}
impl ValueQuery {
    pub fn width(self) -> Width {
        match self {
            Self::Red
            | Self::Green
            | Self::Blue
            | Self::White
            | Self::Amber
            | Self::Free
            | Self::RgbwafControl => Width::Byte,
            _ => Width::Word,
        }
    }
    /// Actual-level query updates colour reporting. Its reply is not a colour
    /// byte; then the value reply is the byte value or word MSB, followed by LSB.
    /// Stop on a failed query; never substitute a missing byte with zero.
    pub fn plan(self, address: ShortAddress) -> Vec<Step> {
        let selector = match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Temperature => 2,
            Self::Red => 9,
            Self::Green => 10,
            Self::Blue => 11,
            Self::White => 12,
            Self::Amber => 13,
            Self::Free => 14,
            Self::RgbwafControl => 15,
            Self::Coolest => 128,
            Self::PhysicalCoolest => 129,
            Self::Warmest => 130,
            Self::PhysicalWarmest => 131,
        };
        let target = Destination::Short(address);
        let mut steps = vec![
            Step::Query(target.command(0xa0)),
            Step::Send(special(0xa3, selector)),
            Step::Send(0xc108),
            Step::Query(target.command(0xfa)),
        ];
        if self.width() == Width::Word {
            steps.push(Step::Query(target.command(0x98)));
        }
        steps
    }
}

/// Only correlated transaction replies may enter these decoders. An arbitrary
/// monitor frame is not evidence of a query response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply<T> {
    Value(T),
    NoResponse,
    ReceiveCollision(Option<u8>),
    TransmitCollision(Option<u8>),
}
impl<T> Reply<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Reply<U> {
        match self {
            Self::Value(value) => Reply::Value(map(value)),
            Self::NoResponse => Reply::NoResponse,
            Self::ReceiveCollision(code) => Reply::ReceiveCollision(code),
            Self::TransmitCollision(code) => Reply::TransmitCollision(code),
        }
    }
}
/// Preserves bitmap/version FF. Apply ByteValue only for byte colour values.
pub fn byte_reply(reply: BusReply) -> Reply<u8> {
    match reply {
        BusReply::Value(raw) => Reply::Value(raw),
        BusReply::NoResponse => Reply::NoResponse,
        BusReply::ReceiveCollision(code) => Reply::ReceiveCollision(code),
        BusReply::TransmitCollision(code) => Reply::TransmitCollision(code),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteValue {
    Known(u8),
    Mask,
}
impl ByteValue {
    pub fn decode(raw: u8) -> Self {
        if raw == 255 {
            Self::Mask
        } else {
            Self::Known(raw)
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordValue {
    Known(u16),
    Mask,
}
pub fn word_reply(high: BusReply, low: BusReply) -> Reply<WordValue> {
    match byte_reply(high) {
        Reply::Value(msb) => byte_reply(low).map(|lsb| match u16::from_be_bytes([msb, lsb]) {
            65535 => WordValue::Mask,
            value => WordValue::Known(value),
        }),
        Reply::NoResponse => Reply::NoResponse,
        Reply::ReceiveCollision(code) => Reply::ReceiveCollision(code),
        Reply::TransmitCollision(code) => Reply::TransmitCollision(code),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features(u8);
impl Features {
    pub fn decode(raw: u8) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn xy(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn temperature(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn primary_count(self) -> u8 {
        (self.0 >> 2) & 7
    }
    pub fn rgbwaf_channels(self) -> u8 {
        (self.0 >> 5) & 7
    }
    pub fn counts_valid(self) -> bool {
        self.primary_count() <= 6 && self.rgbwaf_channels() <= 6
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColourType {
    Xy,
    Temperature,
    PrimaryN,
    Rgbwaf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status(u8);
impl Status {
    pub fn decode(raw: u8) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn xy_out_of_range(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn temperature_out_of_range(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn calibration_running(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn calibration_successful(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn active(self) -> Option<ColourType> {
        match self.0 & 0xf0 {
            0x10 => Some(ColourType::Xy),
            0x20 => Some(ColourType::Temperature),
            0x40 => Some(ColourType::PrimaryN),
            0x80 => Some(ColourType::Rgbwaf),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
