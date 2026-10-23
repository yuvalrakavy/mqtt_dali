//! Read-only Part102/103 memory framing and version-gated bank0 decoding.
//! A plan uses scratch DTRs; hold it under one physical-operation permit.
use super::{gear, input};
use crate::atx::wire::DaliFrame;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Output(gear::ShortAddress),
    Input(input::Address),
}
impl Target {
    pub fn address(self) -> u8 {
        match self {
            Self::Output(a) => a.raw(),
            Self::Input(a) => a.raw(),
        }
    }
    pub fn bank(self, bank: u8) -> DaliFrame {
        match self {
            Self::Output(_) => DaliFrame::Bits16(0xc300 | u16::from(bank)),
            Self::Input(_) => DaliFrame::Bits24(0xc13100 | u32::from(bank)),
        }
    }
    pub fn offset(self, offset: u8) -> DaliFrame {
        match self {
            Self::Output(_) => DaliFrame::Bits16(0xa300 | u16::from(offset)),
            Self::Input(_) => DaliFrame::Bits24(0xc13000 | u32::from(offset)),
        }
    }
    pub fn read_frame(self) -> DaliFrame {
        match self {
            Self::Output(a) => DaliFrame::Bits16((u16::from(a.raw()) << 9) | 0x01c5),
            Self::Input(a) => DaliFrame::Bits24((u32::from((a.raw() << 1) | 1) << 16) | 0xfe3c),
        }
    }
    pub fn version(self) -> DaliFrame {
        match self {
            Self::Output(a) => DaliFrame::Bits16((u16::from(a.raw()) << 9) | 0x0197),
            Self::Input(a) => DaliFrame::Bits24((u32::from((a.raw() << 1) | 1) << 16) | 0xfe34),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    start: u8,
    count: u8,
}
impl Window {
    pub fn new(start: u8, count: u8) -> Result<Self, &'static str> {
        if count == 0 || count > 64 || u16::from(start) + u16::from(count) > 255 {
            return Err("memory window requires 1..64 bytes within locations 0..254");
        }
        Ok(Self { start, count })
    }
    pub fn start(self) -> u8 {
        self.start
    }
    pub fn count(self) -> u8 {
        self.count
    }
    pub fn addresses(self) -> impl Iterator<Item = u8> {
        self.start..=self.start + self.count - 1
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Legacy,
    Modern,
    Unknown,
}
impl Layout {
    pub fn from_version(version: Option<u8>) -> Self {
        match version {
            Some(1) => Self::Legacy,
            Some(8..=254) => Self::Modern,
            _ => Self::Unknown,
        }
    }
    pub fn last_location(self) -> u8 {
        match self {
            Self::Legacy => 14,
            Self::Modern => 26,
            Self::Unknown => 10,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub layout: Layout,
    pub gtin: Option<String>,
    pub serial_number: Option<String>,
    pub serial_bits: Option<u8>,
    pub firmware_version: Option<[u8; 2]>,
    pub hardware_version: Option<[u8; 2]>,
    pub last_memory_bank: Option<u8>,
}
/// Missing bytes stay absent. Numeric FF is retained by raw reports; all-FF
/// identity/version fields have no usable interpretation.
pub fn identity(version: Option<u8>, bytes: &[Option<u8>]) -> Identity {
    let layout = Layout::from_version(version);
    let field = |start: usize, len: usize| -> Option<Vec<u8>> {
        let values = bytes
            .get(start..start + len)?
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()?;
        (!values.iter().all(|value| *value == 255)).then_some(values)
    };
    let version_field = |start| field(start, 2).map(|value| [value[0], value[1]]);
    let gtin = field(3, 6).and_then(|value| {
        let value = value
            .into_iter()
            .fold(0u64, |value, byte| (value << 8) | u64::from(byte));
        (value <= 99_999_999_999_999).then(|| format!("{value:014}"))
    });
    let serial_bits = match layout {
        Layout::Legacy => Some(32),
        Layout::Modern => Some(64),
        Layout::Unknown => None,
    };
    let serial_number = serial_bits
        .and_then(|bits| field(11, usize::from(bits / 8)))
        .map(|bytes| bytes.iter().map(|byte| format!("{byte:02X}")).collect());
    Identity {
        layout,
        gtin,
        serial_number,
        serial_bits,
        firmware_version: version_field(9),
        hardware_version: (layout == Layout::Modern)
            .then(|| version_field(19))
            .flatten(),
        last_memory_bank: bytes.get(2).copied().flatten(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_codec_literals_keep_spaces_and_boundaries() {
        let output = Target::Output(gear::ShortAddress::new(63).unwrap());
        let input = Target::Input(input::Address::new(63).unwrap());
        assert_eq!(output.bank(255), DaliFrame::Bits16(0xc3ff));
        assert_eq!(output.offset(254), DaliFrame::Bits16(0xa3fe));
        assert_eq!(output.read_frame(), DaliFrame::Bits16(0x7fc5));
        assert_eq!(output.version(), DaliFrame::Bits16(0x7f97));
        assert_eq!(input.bank(255), DaliFrame::Bits24(0xc131ff));
        assert_eq!(input.offset(254), DaliFrame::Bits24(0xc130fe));
        assert_eq!(input.read_frame(), DaliFrame::Bits24(0x7ffe3c));
        assert_eq!(input.version(), DaliFrame::Bits24(0x7ffe34));
        assert_eq!(
            Target::Output(gear::ShortAddress::new(0).unwrap()).read_frame(),
            DaliFrame::Bits16(0x01c5)
        );
        assert_eq!(
            Target::Input(input::Address::new(0).unwrap()).read_frame(),
            DaliFrame::Bits24(0x01fe3c)
        );
        assert_eq!(
            Window::new(191, 64)
                .unwrap()
                .addresses()
                .collect::<Vec<_>>(),
            (191..=254).collect::<Vec<_>>()
        );
        for (start, count) in [(255, 1), (254, 2), (0, 0), (0, 65)] {
            assert!(Window::new(start, count).is_err());
        }
    }
    #[test]
    fn memory_identity_decoding_requires_layout_and_exact_bytes() {
        let mut bytes = vec![Some(0); 27];
        bytes[0] = Some(26);
        bytes[2] = Some(255);
        bytes[8] = Some(1);
        bytes[9] = Some(2);
        bytes[10] = Some(3);
        for (index, value) in [0, 1, 2, 3, 4, 5, 254, 255].into_iter().enumerate() {
            bytes[11 + index] = Some(value);
        }
        bytes[19] = Some(4);
        bytes[20] = Some(5);
        let modern = identity(Some(8), &bytes);
        assert_eq!(modern.gtin.as_deref(), Some("00000000000001"));
        assert_eq!(
            modern.serial_number.as_deref(),
            Some("000102030405FEFF"),
            "64-bit serial lost precision or leading zeros"
        );
        assert_eq!(modern.hardware_version, Some([4, 5]));
        assert_eq!(modern.last_memory_bank, Some(255));
        let legacy = identity(Some(1), &bytes);
        assert_eq!(
            legacy.serial_number.as_deref(),
            Some("00010203"),
            "legacy serial absorbed manufacturer bytes"
        );
        assert_eq!(legacy.hardware_version, None);
        for version in [None, Some(0), Some(7), Some(255)] {
            let unknown = identity(version, &bytes);
            assert_eq!(unknown.serial_number, None);
            assert_eq!(unknown.hardware_version, None);
            assert_eq!(unknown.firmware_version, Some([2, 3]));
        }
        bytes[13] = None;
        assert_eq!(
            identity(Some(8), &bytes).serial_number,
            None,
            "missing byte was replaced or shifted"
        );
        assert_eq!(identity(Some(8), &[Some(255); 27]).gtin, None);
        assert_eq!(identity(Some(8), &[Some(255); 27]).serial_number, None);
        assert_eq!(identity(Some(8), &[Some(255); 27]).firmware_version, None);
    }
}
