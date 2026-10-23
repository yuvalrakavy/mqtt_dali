//! Exact Part252 energy/power interpretation; no ATX I/O or floating point.
use super::gear::ShortAddress;
use crate::atx::wire::DaliFrame;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub enum Latch {
    Freeze,
    Release,
}
impl Latch {
    pub fn frame(self) -> DaliFrame {
        DaliFrame::Bits16(match self {
            Self::Freeze => 0xc9aa,
            Self::Release => 0xc9ff,
        })
    }
}
/// Send this addressed command with the existing repeated-command policy.
pub fn enable_write_memory(address: ShortAddress) -> DaliFrame {
    DaliFrame::Bits16((u16::from(address.raw()) << 9) | 0x0181)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    Active,
    Apparent,
    Loadside,
}
impl Bank {
    pub fn new(raw: u8) -> Result<Self, &'static str> {
        match raw {
            202 => Ok(Self::Active),
            203 => Ok(Self::Apparent),
            204 => Ok(Self::Loadside),
            _ => Err("energy bank must be 202, 203 or 204"),
        }
    }
    pub fn number(self) -> u8 {
        match self {
            Self::Active => 202,
            Self::Apparent => 203,
            Self::Loadside => 204,
        }
    }
    pub fn units(self) -> (&'static str, &'static str) {
        if self == Self::Apparent {
            ("VAh", "VA")
        } else {
            ("Wh", "W")
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatStatus {
    Supported,
    Unrecognized,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Value,
    TemporarilyUnavailable,
    Unavailable,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub status: Status,
    pub raw: Option<u64>,
    pub scale: Option<i8>,
    pub value: Option<String>,
    pub unit: &'static str,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Data {
    pub version: Option<u8>,
    pub format_status: FormatStatus,
    pub fields: BTreeMap<&'static str, Field>,
}
fn decimal(raw: u64, scale: i8) -> String {
    if scale >= 0 {
        return (u128::from(raw) * 10_u128.pow(scale as u32)).to_string();
    }
    let digits = usize::from(scale.unsigned_abs());
    let divisor = 10_u64.pow(digits as u32);
    let whole = raw / divisor;
    let fraction = raw % divisor;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:0digits$}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}
fn field(capture: &[Option<u8>], start: usize, width: usize, unit: &'static str) -> Field {
    let scale = capture.get(start).copied().flatten().map(|raw| raw as i8);
    let raw = capture
        .get(start + 1..start + width + 1)
        .and_then(|bytes| bytes.iter().copied().collect::<Option<Vec<_>>>())
        .map(|bytes| {
            bytes
                .into_iter()
                .fold(0_u64, |raw, byte| (raw << 8) | u64::from(byte))
        });
    let maximum = (1_u64 << (width * 8)) - 1;
    let status = match (scale, raw) {
        (Some(scale), _) if !(-6..=6).contains(&scale) => Status::Invalid,
        (Some(_), Some(raw)) if raw == maximum - 1 => Status::TemporarilyUnavailable,
        (Some(_), Some(raw)) if raw == maximum => Status::Invalid,
        (Some(_), Some(_)) => Status::Value,
        _ => Status::Unavailable,
    };
    Field {
        status,
        raw,
        scale,
        value: (status == Status::Value).then(|| decimal(raw.unwrap(), scale.unwrap())),
        unit,
    }
}
/// Interpret only a known, header-bounded Part252 version1 layout.
pub fn decode(bank: Bank, capture: &[Option<u8>]) -> Data {
    let mut data = Data {
        version: None,
        format_status: FormatStatus::Unavailable,
        fields: BTreeMap::new(),
    };
    let Some(last) = capture
        .first()
        .copied()
        .flatten()
        .filter(|last| *last != 255)
    else {
        return data;
    };
    let capture = &capture[..capture.len().min(usize::from(last) + 1)];
    let Some(version) = capture.get(3).copied().flatten() else {
        return data;
    };
    data.version = Some(version);
    if version != 1 {
        data.format_status = FormatStatus::Unrecognized;
        return data;
    }
    data.format_status = FormatStatus::Supported;
    let (energy_unit, power_unit) = bank.units();
    data.fields = BTreeMap::from([
        ("energy", field(capture, 4, 6, energy_unit)),
        ("power", field(capture, 11, 4, power_unit)),
    ]);
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn energy_codec_latch_frames_are_literal_and_only_allow_freeze_or_release() {
        for (address, frame) in [(0, 0x0181), (17, 0x2381), (63, 0x7f81)] {
            assert_eq!(
                enable_write_memory(ShortAddress::new(address).unwrap()),
                DaliFrame::Bits16(frame),
                "energy snapshot enable-write literal changed"
            );
        }
        assert_eq!(
            Latch::Freeze.frame(),
            DaliFrame::Bits16(0xc9aa),
            "energy latch literal changed"
        );
        assert_eq!(
            Latch::Release.frame(),
            DaliFrame::Bits16(0xc9ff),
            "energy release literal changed"
        );
    }
    fn bytes() -> Vec<Option<u8>> {
        // Active energy123.456Wh, power42.5W; raw scales are signed bytes.
        [15, 0, 255, 1, 253, 0, 0, 0, 1, 226, 64, 255, 0, 0, 1, 169]
            .into_iter()
            .map(Some)
            .collect()
    }
    fn known(bank: Bank, bytes: &[Option<u8>]) -> Data {
        let data = decode(bank, bytes);
        assert_eq!(
            data.format_status,
            FormatStatus::Supported,
            "Part252 supported bank was not decoded"
        );
        assert_eq!(data.version, Some(1));
        assert_eq!(data.fields.len(), 2);
        data
    }
    fn value(field: &Field, raw: u64, scale: i8, text: &str, unit: &str) {
        assert_eq!(field.status, Status::Value);
        assert_eq!(field.raw, Some(raw));
        assert_eq!(field.scale, Some(scale));
        assert_eq!(
            field.value.as_deref(),
            Some(text),
            "scaled decimal lost precision"
        );
        assert_eq!(field.unit, unit);
    }
    #[test]
    fn energy_data_signed_scales_and_units_preserve_exact_decimal_strings() {
        for (bank, energy_unit, power_unit) in [
            (Bank::Active, "Wh", "W"),
            (Bank::Apparent, "VAh", "VA"),
            (Bank::Loadside, "Wh", "W"),
        ] {
            let d = known(bank, &bytes());
            value(&d.fields["energy"], 123456, -3, "123.456", energy_unit);
            value(&d.fields["power"], 425, -1, "42.5", power_unit);
            let json = serde_json::to_value(d).unwrap();
            assert!(json["fields"]["energy"]["value"].is_string());
        }
        for raw in [0, 1, 201, 205, 255] {
            assert!(Bank::new(raw).is_err());
        }
        assert_eq!(Bank::new(204).unwrap().number(), 204);
    }
    #[test]
    fn energy_data_maxima_and_scale_extremes_never_use_floating_point() {
        let mut b = bytes();
        b[5..11].copy_from_slice(&[
            Some(255),
            Some(255),
            Some(255),
            Some(255),
            Some(255),
            Some(253),
        ]);
        b[12..16].copy_from_slice(&[Some(255), Some(255), Some(255), Some(253)]);
        for (scale, signed, energy, power) in [
            (250, -6, "281474976.710653", "4294.967293"),
            (0, 0, "281474976710653", "4294967293"),
            (6, 6, "281474976710653000000", "4294967293000000"),
        ] {
            b[4] = Some(scale);
            b[11] = Some(scale);
            let d = known(Bank::Active, &b);
            value(&d.fields["energy"], 281474976710653, signed, energy, "Wh");
            value(&d.fields["power"], 4294967293, signed, power, "W");
        }
    }
    #[test]
    fn energy_data_tmask_and_invalid_all_ff_remain_distinct_from_zero() {
        let mut b = bytes();
        for (last, status) in [
            (254, Status::TemporarilyUnavailable),
            (255, Status::Invalid),
        ] {
            b[5..11].fill(Some(255));
            b[10] = Some(last);
            b[12..16].fill(Some(255));
            b[15] = Some(last);
            let d = known(Bank::Active, &b);
            for key in ["energy", "power"] {
                assert_eq!(
                    d.fields[key].status, status,
                    "energy MASK/TMASK semantics were guessed"
                );
                assert_eq!(d.fields[key].value, None);
                assert!(d.fields[key].raw.is_some());
            }
        }
        b[5..11].fill(Some(0));
        b[12..16].fill(Some(0));
        let d = known(Bank::Active, &b);
        value(&d.fields["energy"], 0, -3, "0", "Wh");
        value(&d.fields["power"], 0, -1, "0", "W");
    }
    #[test]
    fn energy_data_scale_validation_precedes_tmask_and_retains_diagnostics() {
        for scale in [7, 127, 128, 249] {
            let mut b = bytes();
            b[4] = Some(scale);
            b[5..11].fill(Some(255));
            b[10] = Some(254);
            let d = known(Bank::Active, &b);
            assert_eq!(
                d.fields["energy"].status,
                Status::Invalid,
                "invalid scale authorized a value/TMASK"
            );
            assert_eq!(d.fields["energy"].scale, Some(scale as i8));
            assert_eq!(d.fields["energy"].value, None);
            value(&d.fields["power"], 425, -1, "42.5", "W");
        }
    }
    #[test]
    fn energy_data_missing_bytes_do_not_shift_or_default_fields() {
        for at in [4, 5, 8, 10] {
            let mut b = bytes();
            b[at] = None;
            let d = known(Bank::Active, &b);
            assert_eq!(
                d.fields["energy"].status,
                Status::Unavailable,
                "silent energy byte became a value"
            );
            assert_eq!(d.fields["energy"].value, None);
            value(&d.fields["power"], 425, -1, "42.5", "W");
        }
        let mut b = bytes();
        b[0] = Some(10);
        let d = known(Bank::Active, &b);
        value(&d.fields["energy"], 123456, -3, "123.456", "Wh");
        assert_eq!(
            d.fields["power"].status,
            Status::Unavailable,
            "header did not bound power bytes"
        );
    }
    #[test]
    fn energy_data_header_and_version_gate_the_layout() {
        for header in [None, Some(2), Some(255)] {
            let mut b = bytes();
            b[0] = header;
            let d = decode(Bank::Active, &b);
            assert_eq!(d.format_status, FormatStatus::Unavailable);
            assert!(d.fields.is_empty());
        }
        let mut b = bytes();
        b[3] = Some(2);
        let d = decode(Bank::Active, &b);
        assert_eq!(d.version, Some(2));
        assert_eq!(d.format_status, FormatStatus::Unrecognized);
        assert!(
            d.fields.is_empty(),
            "unrecognized energy version authorized fields"
        );
        b[3] = None;
        assert!(decode(Bank::Active, &b).fields.is_empty());
    }
}
