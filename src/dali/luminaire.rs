//! Read-only Part251 bank1 interpretation, independent from ATX I/O.
use serde::Serialize;
use std::collections::BTreeMap;

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
    Unknown,
    Unavailable,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Number(u32),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub status: Status,
    pub value: Option<Value>,
    pub unit: Option<&'static str>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Data {
    pub format_id: Option<u16>,
    pub format_status: FormatStatus,
    pub fields: BTreeMap<&'static str, Field>,
}

impl Field {
    fn absent(status: Status, unit: Option<&'static str>) -> Self {
        Self {
            status,
            value: None,
            unit,
        }
    }
    fn value(value: Value, unit: Option<&'static str>) -> Self {
        Self {
            status: Status::Value,
            value: Some(value),
            unit,
        }
    }
    fn text(value: impl Into<String>) -> Self {
        Self::value(Value::Text(value.into()), None)
    }
}
fn bytes(capture: &[Option<u8>], start: usize, width: usize) -> Option<Vec<u8>> {
    capture.get(start..start + width)?.iter().copied().collect()
}
fn number(
    capture: &[Option<u8>],
    start: usize,
    width: usize,
    unit: Option<&'static str>,
    range: std::ops::RangeInclusive<u32>,
) -> Field {
    let Some(raw) = bytes(capture, start, width) else {
        return Field::absent(Status::Unavailable, unit);
    };
    if raw.iter().all(|b| *b == 255) {
        return Field::absent(Status::Unknown, unit);
    }
    let value = raw
        .into_iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(byte));
    if range.contains(&value) {
        Field::value(Value::Number(value), unit)
    } else {
        Field::absent(Status::Unknown, unit)
    }
}
fn identifier(capture: &[Option<u8>], start: usize, width: usize) -> Field {
    let Some(raw) = bytes(capture, start, width) else {
        return Field::absent(Status::Unavailable, None);
    };
    if raw.iter().all(|b| *b == 255) {
        return Field::absent(Status::Unknown, None);
    }
    if width == 6 {
        let value = raw
            .into_iter()
            .fold(0u64, |value, byte| (value << 8) | u64::from(byte));
        if value > 99_999_999_999_999 {
            return Field::absent(Status::Invalid, None);
        }
        Field::text(format!("{value:014}"))
    } else {
        Field::text(
            raw.iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<String>(),
        )
    }
}
fn ascii(capture: &[Option<u8>], start: usize, width: usize) -> Field {
    let mut raw = Vec::new();
    for address in start..start + width {
        match capture.get(address).copied().flatten() {
            Some(0) => break,
            Some(byte) => raw.push(byte),
            None => return Field::absent(Status::Unavailable, None),
        }
    }
    // Part251 text is ASCII/NUL, not a numeric MASK field. Non-ASCII FF stays
    // invalid even when every byte is FF. Never skip a missing byte before NUL.
    if !raw.is_ascii() {
        return Field::absent(Status::Invalid, None);
    }
    Field::text(String::from_utf8(raw).expect("ASCII is UTF-8"))
}
/// Decode a header-bounded bank1 capture. Unknown layouts are never guessed.
pub fn decode(capture: &[Option<u8>]) -> Data {
    let mut data = Data {
        format_id: None,
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
    let Some(format) = bytes(capture, 17, 2) else {
        return data;
    };
    let format = u16::from_be_bytes([format[0], format[1]]);
    data.format_id = Some(format);
    if format != 3 {
        data.format_status = FormatStatus::Unrecognized;
        return data;
    }
    data.format_status = FormatStatus::Supported;
    data.fields = BTreeMap::from([
        ("manufacturer_gtin", identifier(capture, 3, 6)),
        ("luminaire_id", identifier(capture, 9, 8)),
        ("manufacture_year", number(capture, 19, 1, None, 0..=99)),
        ("manufacture_week", number(capture, 20, 1, None, 1..=53)),
        (
            "input_power_nominal",
            number(capture, 21, 2, Some("W"), 0..=65534),
        ),
        (
            "input_power_minimum_dim",
            number(capture, 23, 2, Some("W"), 0..=65534),
        ),
        (
            "mains_voltage_minimum",
            number(capture, 25, 2, Some("V"), 90..=480),
        ),
        (
            "mains_voltage_maximum",
            number(capture, 27, 2, Some("V"), 90..=480),
        ),
        (
            "light_output_nominal",
            number(capture, 29, 3, Some("lm"), 0..=0xfffffe),
        ),
        ("cri", number(capture, 32, 1, None, 0..=100)),
        ("cct", number(capture, 33, 2, Some("K"), 0..=65534)),
        ("light_distribution", number(capture, 35, 1, None, 0..=254)),
        ("oem_color", ascii(capture, 36, 24)),
        ("identification", ascii(capture, 60, 60)),
    ]);
    let cct = data.fields.get_mut("cct").unwrap();
    match cct.value {
        Some(Value::Number(65534)) => *cct = Field::text("part_209"),
        Some(Value::Number(value)) if value > 17000 => {
            *cct = Field::absent(Status::Invalid, Some("K"))
        }
        _ => {}
    }
    let distribution = data.fields.get_mut("light_distribution").unwrap();
    if let Some(Value::Number(value)) = distribution.value {
        *distribution = if let Some(name) = [
            "not_specified",
            "type_i",
            "type_ii",
            "type_iii",
            "type_iv",
            "type_v",
        ]
        .get(value as usize)
        {
            Field::text(*name)
        } else {
            Field::absent(Status::Invalid, None)
        };
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(bytes: &mut [Option<u8>], at: usize, values: &[u8]) {
        for (i, byte) in values.iter().enumerate() {
            bytes[at + i] = Some(*byte);
        }
    }
    fn bank() -> Vec<Option<u8>> {
        let mut b = vec![Some(0); 120];
        put(&mut b, 0, &[0x77, 0, 255]);
        put(&mut b, 3, &[0, 1, 2, 3, 4, 5]);
        put(&mut b, 9, &[0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10]);
        put(
            &mut b,
            17,
            &[
                0, 3, 24, 53, 0, 42, 0, 7, 0, 230, 1, 224, 0, 3, 232, 95, 0x0b, 0xb8, 5,
            ],
        );
        put(&mut b, 36, b"Black\0");
        put(&mut b, 60, b"Garden fixture\0");
        b
    }
    fn number(data: &Data, key: &str, expected: u32, unit: Option<&str>) {
        let f = &data.fields[key];
        assert_eq!(f.status, Status::Value, "unexpected field status: {key}");
        assert_eq!(
            f.value,
            Some(Value::Number(expected)),
            "wrong decoded value: {key}"
        );
        assert_eq!(f.unit, unit, "wrong units: {key}");
    }
    fn text(data: &Data, key: &str, expected: &str) {
        assert_eq!(data.fields[key].status, Status::Value);
        assert_eq!(
            data.fields[key].value,
            Some(Value::Text(expected.into())),
            "exact text/identifier changed: {key}"
        );
    }
    #[test]
    fn luminaire_data_known_format_keeps_exact_ids_ratings_units_and_text() {
        let d = decode(&bank());
        assert_eq!(
            d.format_status,
            FormatStatus::Supported,
            "Part251 content format was not recognized"
        );
        assert_eq!(d.format_id, Some(3));
        assert_eq!(d.fields.len(), 14);
        text(&d, "manufacturer_gtin", "00004328719365");
        text(&d, "luminaire_id", "FEDCBA9876543210");
        number(&d, "manufacture_year", 24, None);
        number(&d, "manufacture_week", 53, None);
        number(&d, "input_power_nominal", 42, Some("W"));
        number(&d, "input_power_minimum_dim", 7, Some("W"));
        number(&d, "mains_voltage_minimum", 230, Some("V"));
        number(&d, "mains_voltage_maximum", 480, Some("V"));
        number(&d, "light_output_nominal", 1000, Some("lm"));
        number(&d, "cri", 95, None);
        number(&d, "cct", 3000, Some("K"));
        text(&d, "light_distribution", "type_v");
        text(&d, "oem_color", "Black");
        text(&d, "identification", "Garden fixture");
        let j = serde_json::to_value(d).unwrap();
        assert_eq!(j["fields"]["luminaire_id"]["value"], "FEDCBA9876543210");
    }
    #[test]
    fn luminaire_data_unknowns_and_invalid_encoding_do_not_invent_numeric_values() {
        let mut b = bank();
        put(&mut b, 3, &[255; 6]);
        put(&mut b, 9, &[255; 8]);
        put(&mut b, 19, &[255, 0, 255, 255]);
        put(&mut b, 25, &[0, 89]);
        b[32] = Some(200);
        put(&mut b, 33, &[0x42, 0x69]);
        b[36] = Some(128);
        let d = decode(&b);
        for key in [
            "manufacturer_gtin",
            "luminaire_id",
            "manufacture_year",
            "manufacture_week",
            "input_power_nominal",
            "mains_voltage_minimum",
            "cri",
        ] {
            assert_eq!(
                d.fields[key].status,
                Status::Unknown,
                "unknown field became usable: {key}"
            );
            assert_eq!(d.fields[key].value, None);
        }
        for key in ["cct", "oem_color"] {
            assert_eq!(
                d.fields[key].status,
                Status::Invalid,
                "invalid field became usable: {key}"
            );
            assert_eq!(d.fields[key].value, None);
        }
        number(&d, "input_power_minimum_dim", 7, Some("W"));
    }
    #[test]
    fn luminaire_data_cct_indication_and_distribution_ff_remain_distinct() {
        let mut b = bank();
        put(&mut b, 33, &[255, 254]);
        let d = decode(&b);
        text(&d, "cct", "part_209");
        assert_eq!(
            d.fields["cct"].unit, None,
            "Part209 indication became a Kelvin temperature"
        );
        for (raw, status, value) in [
            (0, Status::Value, Some("not_specified")),
            (5, Status::Value, Some("type_v")),
            (6, Status::Invalid, None),
            (254, Status::Invalid, None),
            (255, Status::Unknown, None),
        ] {
            b[35] = Some(raw);
            let d = decode(&b);
            assert_eq!(
                d.fields["light_distribution"].status, status,
                "distribution mask/reserved classification changed: {raw}"
            );
            assert_eq!(
                d.fields["light_distribution"].value,
                value.map(|s| Value::Text(s.into()))
            );
        }
        put(&mut b, 33, &[255, 255]);
        assert_eq!(decode(&b).fields["cct"].status, Status::Unknown);
    }
    #[test]
    fn luminaire_data_holes_nul_empty_and_full_width_text_are_exact() {
        let mut b = bank();
        b[12] = None;
        b[42] = None;
        let d = decode(&b);
        assert_eq!(
            d.fields["luminaire_id"].status,
            Status::Unavailable,
            "missing serial byte was shifted/defaulted"
        );
        text(&d, "oem_color", "Black");
        b[40] = None;
        assert_eq!(
            decode(&b).fields["oem_color"].status,
            Status::Unavailable,
            "text hole before NUL was skipped"
        );
        b[36] = Some(0);
        text(&decode(&b), "oem_color", "");
        put(&mut b, 36, &[255; 24]);
        assert_eq!(
            decode(&b).fields["oem_color"].status,
            Status::Invalid,
            "numeric MASK semantics leaked into ASCII text"
        );
        put(&mut b, 36, b"123456789012345678901234");
        text(&decode(&b), "oem_color", "123456789012345678901234");
    }
    #[test]
    fn luminaire_data_header_and_format_bound_every_interpretation() {
        let mut b = bank();
        b[18] = Some(4);
        let d = decode(&b);
        assert_eq!(d.format_id, Some(4));
        assert_eq!(
            d.format_status,
            FormatStatus::Unrecognized,
            "unrecognized Part251 layout was guessed"
        );
        assert!(d.fields.is_empty(), "unknown format authorized fields");
        b[18] = None;
        let d = decode(&b);
        assert_eq!(d.format_status, FormatStatus::Unavailable);
        assert!(d.fields.is_empty());
        for last in [None, Some(4), Some(255)] {
            let mut b = bank();
            b[0] = last;
            let d = decode(&b);
            assert_eq!(
                d.format_status,
                FormatStatus::Unavailable,
                "bank header did not bound format proof"
            );
            assert!(d.fields.is_empty());
        }
        let mut b = bank();
        b.truncate(19);
        b[0] = Some(18);
        let d = decode(&b);
        assert_eq!(d.format_status, FormatStatus::Supported);
        assert_eq!(d.fields.len(), 14);
        assert_eq!(d.fields["identification"].status, Status::Unavailable);
        text(&d, "luminaire_id", "FEDCBA9876543210");
    }
}
