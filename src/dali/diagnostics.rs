//! Explicit Part253 capture interpretation, independent from transport.
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    Driver,
    LightSource,
    Maintenance,
}
impl Bank {
    pub fn new(raw: u8) -> Result<Self, &'static str> {
        match raw {
            205 => Ok(Self::Driver),
            206 => Ok(Self::LightSource),
            207 => Ok(Self::Maintenance),
            _ => Err("diagnostic bank must be205,206 or207"),
        }
    }
    pub fn number(self) -> u8 {
        match self {
            Self::Driver => 205,
            Self::LightSource => 206,
            Self::Maintenance => 207,
        }
    }
    pub fn last(self) -> u8 {
        match self {
            Self::Driver => 0x1c,
            Self::LightSource => 0x20,
            Self::Maintenance => 7,
        }
    }
    pub fn snapshot(self) -> bool {
        self != Self::Maintenance
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
    ReadProtected,
    Unknown,
    Unavailable,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Number(String),
    Boolean(bool),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub status: Status,
    pub raw: Option<u64>,
    pub value: Option<Value>,
    pub unit: Option<&'static str>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Data {
    pub version: Option<u8>,
    pub format_status: FormatStatus,
    pub fields: BTreeMap<&'static str, Field>,
}
#[derive(Clone, Copy)]
enum Kind {
    Number,
    Boolean,
    Temperature,
}
#[derive(Clone, Copy)]
struct Definition {
    key: &'static str,
    start: usize,
    width: usize,
    unit: Option<&'static str>,
    scale: i8,
    maximum: u64,
    mask: Status,
    kind: Kind,
}
const fn number(
    key: &'static str,
    start: usize,
    width: usize,
    unit: Option<&'static str>,
    scale: i8,
    mask: Status,
) -> Definition {
    Definition {
        key,
        start,
        width,
        unit,
        scale,
        maximum: (1_u64 << (width * 8)) - 3,
        mask,
        kind: Kind::Number,
    }
}
const fn limited(mut definition: Definition, maximum: u64) -> Definition {
    definition.maximum = maximum;
    definition
}
const fn flag(key: &'static str, start: usize, mask: Status) -> Definition {
    let mut definition = limited(number(key, start, 1, None, 0, mask), 1);
    definition.kind = Kind::Boolean;
    definition
}
const fn temperature(key: &'static str, start: usize, mask: Status) -> Definition {
    let mut definition = number(key, start, 1, Some("°C"), 0, mask);
    definition.kind = Kind::Temperature;
    definition
}
const DRIVER: [Definition; 19] = [
    number("operating_time", 4, 4, Some("s"), 0, Status::Invalid),
    number("start_counter", 8, 3, Some("count"), 0, Status::Invalid),
    number(
        "supply_voltage",
        11,
        2,
        Some("Vrms"),
        -1,
        Status::ReadProtected,
    ),
    number(
        "supply_frequency",
        13,
        1,
        Some("Hz"),
        0,
        Status::ReadProtected,
    ),
    limited(
        number("power_factor", 14, 1, None, -2, Status::ReadProtected),
        100,
    ),
    flag("overall_failure", 15, Status::Invalid),
    number(
        "overall_failure_counter",
        16,
        1,
        Some("count"),
        0,
        Status::Invalid,
    ),
    flag("undervoltage", 17, Status::ReadProtected),
    number(
        "undervoltage_counter",
        18,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("overvoltage", 19, Status::ReadProtected),
    number(
        "overvoltage_counter",
        20,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("power_limit", 21, Status::ReadProtected),
    number(
        "power_limit_counter",
        22,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("thermal_derating", 23, Status::ReadProtected),
    number(
        "thermal_derating_counter",
        24,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("thermal_shutdown", 25, Status::ReadProtected),
    number(
        "thermal_shutdown_counter",
        26,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    temperature("temperature", 27, Status::Invalid),
    limited(
        number(
            "output_current_percent",
            28,
            1,
            Some("%"),
            0,
            Status::Invalid,
        ),
        100,
    ),
];
const SOURCE: [Definition; 17] = [
    number(
        "resettable_start_counter",
        4,
        3,
        Some("count"),
        0,
        Status::Invalid,
    ),
    number("start_counter", 7, 3, Some("count"), 0, Status::Invalid),
    number("resettable_on_time", 10, 4, Some("s"), 0, Status::Invalid),
    number("on_time", 14, 4, Some("s"), 0, Status::Invalid),
    number("voltage", 18, 2, Some("V"), -1, Status::Invalid),
    number("current", 20, 2, Some("A"), -3, Status::Invalid),
    flag("overall_failure", 22, Status::Invalid),
    number(
        "overall_failure_counter",
        23,
        1,
        Some("count"),
        0,
        Status::Invalid,
    ),
    flag("short_circuit", 24, Status::ReadProtected),
    number(
        "short_circuit_counter",
        25,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("open_circuit", 26, Status::ReadProtected),
    number(
        "open_circuit_counter",
        27,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("thermal_derating", 28, Status::ReadProtected),
    number(
        "thermal_derating_counter",
        29,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    flag("thermal_shutdown", 30, Status::ReadProtected),
    number(
        "thermal_shutdown_counter",
        31,
        1,
        Some("count"),
        0,
        Status::ReadProtected,
    ),
    temperature("temperature", 32, Status::ReadProtected),
];
const MAINTENANCE: [Definition; 3] = [
    number("rated_life", 4, 1, Some("h"), 3, Status::Unknown),
    temperature("reference_temperature", 5, Status::Unknown),
    number("rated_starts", 6, 2, Some("count"), 2, Status::Unknown),
];
fn decimal(raw: u64, scale: i8) -> String {
    if scale >= 0 {
        return (raw * 10_u64.pow(scale as u32)).to_string();
    }
    let digits = usize::from(scale.unsigned_abs());
    let divisor = 10_u64.pow(digits as u32);
    let fraction = raw % divisor;
    if fraction == 0 {
        return (raw / divisor).to_string();
    }
    let fraction = format!("{fraction:0digits$}");
    format!("{}.{}", raw / divisor, fraction.trim_end_matches('0'))
}
fn field(definition: &Definition, capture: &[Option<u8>]) -> Field {
    let raw = capture
        .get(definition.start..definition.start + definition.width)
        .and_then(|bytes| bytes.iter().copied().collect::<Option<Vec<_>>>())
        .map(|bytes| {
            bytes
                .into_iter()
                .fold(0_u64, |raw, byte| (raw << 8) | u64::from(byte))
        });
    let maximum = (1_u64 << (definition.width * 8)) - 1;
    let status = match raw {
        None => Status::Unavailable,
        Some(raw) if raw == maximum - 1 => Status::TemporarilyUnavailable,
        Some(raw) if raw == maximum => definition.mask,
        Some(raw) if raw > definition.maximum => Status::Invalid,
        Some(_) => Status::Value,
    };
    let value = (status == Status::Value).then(|| match definition.kind {
        Kind::Boolean => Value::Boolean(raw.unwrap() == 1),
        Kind::Temperature => Value::Number((raw.unwrap() as i64 - 60).to_string()),
        Kind::Number => Value::Number(decimal(raw.unwrap(), definition.scale)),
    });
    Field {
        status,
        raw,
        value,
        unit: definition.unit,
    }
}
/// Interpret only the observed, header-bounded version1 standard layout.
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
    let definitions = match bank {
        Bank::Driver => DRIVER.as_slice(),
        Bank::LightSource => SOURCE.as_slice(),
        Bank::Maintenance => MAINTENANCE.as_slice(),
    };
    data.fields = definitions
        .iter()
        .map(|definition| (definition.key, field(definition, capture)))
        .collect();
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture(bank: Bank) -> Vec<Option<u8>> {
        let mut data = vec![Some(0); usize::from(bank.last()) + 1];
        data[0] = Some(bank.last());
        data[2] = Some(255);
        data[3] = Some(1);
        data
    }
    fn set(data: &mut [Option<u8>], start: usize, width: usize, raw: u64) {
        for offset in 0..width {
            data[start + offset] = Some((raw >> ((width - 1 - offset) * 8)) as u8);
        }
    }
    fn known(bank: Bank, data: &[Option<u8>]) -> Data {
        let result = decode(bank, data);
        assert_eq!(
            result.format_status,
            FormatStatus::Supported,
            "Part253 supported layout was not decoded"
        );
        assert_eq!(result.version, Some(1));
        result
    }
    fn value(field: &Field, raw: u64, text: &str, unit: Option<&str>) {
        assert_eq!(field.status, Status::Value);
        assert_eq!(field.raw, Some(raw));
        assert_eq!(
            field.value,
            Some(Value::Number(text.into())),
            "Part253 numeric conversion changed"
        );
        assert_eq!(field.unit, unit);
    }
    #[test]
    fn diagnostics_driver_all_nineteen_fields_use_literal_offsets_and_exact_units() {
        let b = [
            28, 0, 255, 1, 0x12, 0x34, 0x56, 0x78, 0x01, 0x02, 0x03, 0x08, 0xfc, 50, 99, 1, 3, 0,
            4, 1, 5, 0, 6, 1, 7, 0, 8, 35, 100,
        ]
        .map(Some);
        let d = known(Bank::Driver, &b);
        assert_eq!(d.fields.len(), 19);
        for (key, raw, text, unit) in [
            ("operating_time", 305419896, "305419896", Some("s")),
            ("start_counter", 66051, "66051", Some("count")),
            ("supply_voltage", 2300, "230", Some("Vrms")),
            ("supply_frequency", 50, "50", Some("Hz")),
            ("power_factor", 99, "0.99", None),
            ("overall_failure_counter", 3, "3", Some("count")),
            ("undervoltage_counter", 4, "4", Some("count")),
            ("overvoltage_counter", 5, "5", Some("count")),
            ("power_limit_counter", 6, "6", Some("count")),
            ("thermal_derating_counter", 7, "7", Some("count")),
            ("thermal_shutdown_counter", 8, "8", Some("count")),
            ("temperature", 35, "-25", Some("°C")),
            ("output_current_percent", 100, "100", Some("%")),
        ] {
            value(&d.fields[key], raw, text, unit);
        }
        for (key, flag) in [
            ("overall_failure", true),
            ("undervoltage", false),
            ("overvoltage", true),
            ("power_limit", false),
            ("thermal_derating", true),
            ("thermal_shutdown", false),
        ] {
            let f = &d.fields[key];
            assert_eq!(f.status, Status::Value);
            assert_eq!(f.value, Some(Value::Boolean(flag)));
            assert_eq!(f.raw, Some(u64::from(flag)));
            assert_eq!(f.unit, None);
        }
        let json = serde_json::to_value(d).unwrap();
        assert!(json["fields"]["overall_failure"]["value"].is_boolean());
        assert!(json["fields"]["operating_time"]["value"].is_string());
    }
    #[test]
    fn diagnostics_light_source_all_seventeen_fields_use_literal_offsets_and_scales() {
        let b = [
            32, 0, 255, 1, 1, 2, 3, 2, 3, 4, 0, 0, 14, 16, 0x12, 0x34, 0x56, 0x78, 0x01, 0xe1,
            0x04, 0xd2, 0, 1, 1, 2, 0, 3, 1, 4, 0, 5, 85,
        ]
        .map(Some);
        let d = known(Bank::LightSource, &b);
        assert_eq!(d.fields.len(), 17);
        for (key, raw, text, unit) in [
            ("resettable_start_counter", 66051, "66051", Some("count")),
            ("start_counter", 131844, "131844", Some("count")),
            ("resettable_on_time", 3600, "3600", Some("s")),
            ("on_time", 305419896, "305419896", Some("s")),
            ("voltage", 481, "48.1", Some("V")),
            ("current", 1234, "1.234", Some("A")),
            ("overall_failure_counter", 1, "1", Some("count")),
            ("short_circuit_counter", 2, "2", Some("count")),
            ("open_circuit_counter", 3, "3", Some("count")),
            ("thermal_derating_counter", 4, "4", Some("count")),
            ("thermal_shutdown_counter", 5, "5", Some("count")),
            ("temperature", 85, "25", Some("°C")),
        ] {
            value(&d.fields[key], raw, text, unit);
        }
        for (key, flag) in [
            ("overall_failure", false),
            ("short_circuit", true),
            ("open_circuit", false),
            ("thermal_derating", true),
            ("thermal_shutdown", false),
        ] {
            assert_eq!(d.fields[key].value, Some(Value::Boolean(flag)));
            assert_eq!(d.fields[key].unit, None);
        }
    }
    #[test]
    fn diagnostics_maintenance_static_ratings_are_exact_and_temperature_is_unsigned_offset() {
        let mut b = [7, 0, 255, 1, 50, 0, 0xff, 0xfd].map(Some);
        let d = known(Bank::Maintenance, &b);
        assert_eq!(d.fields.len(), 3);
        value(&d.fields["rated_life"], 50, "50000", Some("h"));
        value(&d.fields["reference_temperature"], 0, "-60", Some("°C"));
        value(&d.fields["rated_starts"], 65533, "6553300", Some("count"));
        for (raw, text) in [(60, "0"), (128, "68"), (253, "193")] {
            b[5] = Some(raw);
            value(
                &known(Bank::Maintenance, &b).fields["reference_temperature"],
                u64::from(raw),
                text,
                Some("°C"),
            );
        }
        for (raw, bank, last, snapshot) in [
            (205, Bank::Driver, 28, true),
            (206, Bank::LightSource, 32, true),
            (207, Bank::Maintenance, 7, false),
        ] {
            let actual = Bank::new(raw).unwrap();
            assert_eq!(actual, bank);
            assert_eq!(actual.number(), raw);
            assert_eq!(actual.last(), last);
            assert_eq!(actual.snapshot(), snapshot);
        }
        for invalid in [0, 1, 202, 204, 208, 255] {
            assert!(Bank::new(invalid).is_err());
        }
    }
    #[test]
    fn diagnostics_reserved_values_obey_each_fields_mask_support_and_keep_raw() {
        // Independent literal offset/width/mask table covers every standard field.
        let driver = [
            ("operating_time", 4, 4, false),
            ("start_counter", 8, 3, false),
            ("supply_voltage", 11, 2, true),
            ("supply_frequency", 13, 1, true),
            ("power_factor", 14, 1, true),
            ("overall_failure", 15, 1, false),
            ("overall_failure_counter", 16, 1, false),
            ("undervoltage", 17, 1, true),
            ("undervoltage_counter", 18, 1, true),
            ("overvoltage", 19, 1, true),
            ("overvoltage_counter", 20, 1, true),
            ("power_limit", 21, 1, true),
            ("power_limit_counter", 22, 1, true),
            ("thermal_derating", 23, 1, true),
            ("thermal_derating_counter", 24, 1, true),
            ("thermal_shutdown", 25, 1, true),
            ("thermal_shutdown_counter", 26, 1, true),
            ("temperature", 27, 1, false),
            ("output_current_percent", 28, 1, false),
        ];
        let source = [
            ("resettable_start_counter", 4, 3, false),
            ("start_counter", 7, 3, false),
            ("resettable_on_time", 10, 4, false),
            ("on_time", 14, 4, false),
            ("voltage", 18, 2, false),
            ("current", 20, 2, false),
            ("overall_failure", 22, 1, false),
            ("overall_failure_counter", 23, 1, false),
            ("short_circuit", 24, 1, true),
            ("short_circuit_counter", 25, 1, true),
            ("open_circuit", 26, 1, true),
            ("open_circuit_counter", 27, 1, true),
            ("thermal_derating", 28, 1, true),
            ("thermal_derating_counter", 29, 1, true),
            ("thermal_shutdown", 30, 1, true),
            ("thermal_shutdown_counter", 31, 1, true),
            ("temperature", 32, 1, true),
        ];
        let maintenance = [
            ("rated_life", 4, 1, true),
            ("reference_temperature", 5, 1, true),
            ("rated_starts", 6, 2, true),
        ];
        for (bank, fields) in [
            (Bank::Driver, driver.as_slice()),
            (Bank::LightSource, source.as_slice()),
            (Bank::Maintenance, maintenance.as_slice()),
        ] {
            for &(key, start, width, mask) in fields {
                let max = (1_u64 << (width * 8)) - 1;
                let mut b = capture(bank);
                for (raw, status) in [
                    (max - 1, Status::TemporarilyUnavailable),
                    (
                        max,
                        if !mask {
                            Status::Invalid
                        } else if bank == Bank::Maintenance {
                            Status::Unknown
                        } else {
                            Status::ReadProtected
                        },
                    ),
                ] {
                    set(&mut b, start, width, raw);
                    let f = known(bank, &b).fields.remove(key).unwrap();
                    assert_eq!(
                        f.status, status,
                        "Part253 MASK/TMASK meaning changed for {key}"
                    );
                    assert_eq!(f.raw, Some(raw));
                    assert_eq!(f.value, None);
                }
                b[start] = None;
                let f = known(bank, &b).fields.remove(key).unwrap();
                assert_eq!(f.status, Status::Unavailable);
                assert_eq!(f.raw, None);
                assert_eq!(f.value, None);
            }
        }
    }
    #[test]
    fn diagnostics_boolean_and_percentage_ranges_do_not_turn_invalid_into_true() {
        let mut b = capture(Bank::Driver);
        for offset in [15, 17, 19, 21, 23, 25] {
            b[offset] = Some(2);
        }
        b[14] = Some(101);
        b[28] = Some(101);
        let d = known(Bank::Driver, &b);
        for key in [
            "overall_failure",
            "undervoltage",
            "overvoltage",
            "power_limit",
            "thermal_derating",
            "thermal_shutdown",
            "power_factor",
            "output_current_percent",
        ] {
            assert_eq!(
                d.fields[key].status,
                Status::Invalid,
                "Part253 invalid boolean/percentage accepted: {key}"
            );
            assert_eq!(d.fields[key].value, None);
        }
        b[14] = Some(100);
        b[28] = Some(0);
        let d = known(Bank::Driver, &b);
        value(&d.fields["power_factor"], 100, "1", None);
        value(&d.fields["output_current_percent"], 0, "0", Some("%"));
        set(&mut b, 4, 4, 0xffff_fffd);
        set(&mut b, 8, 3, 0xff_fffd);
        set(&mut b, 11, 2, 0xfffd);
        let d = known(Bank::Driver, &b);
        value(
            &d.fields["operating_time"],
            4294967293,
            "4294967293",
            Some("s"),
        );
        value(
            &d.fields["start_counter"],
            16777213,
            "16777213",
            Some("count"),
        );
        value(&d.fields["supply_voltage"], 65533, "6553.3", Some("Vrms"));
    }
    #[test]
    fn diagnostics_header_version_holes_and_truncation_never_guess_a_layout() {
        for bank in [Bank::Driver, Bank::LightSource, Bank::Maintenance] {
            for data in [
                vec![],
                vec![None],
                vec![Some(255)],
                vec![Some(2), Some(0), Some(255), Some(1)],
            ] {
                let d = decode(bank, &data);
                assert_eq!(d.format_status, FormatStatus::Unavailable);
                assert_eq!(d.version, None);
                assert!(d.fields.is_empty());
            }
            let mut b = capture(bank);
            b[3] = Some(2);
            let d = decode(bank, &b);
            assert_eq!(d.version, Some(2));
            assert_eq!(d.format_status, FormatStatus::Unrecognized);
            assert!(d.fields.is_empty());
            b[3] = None;
            assert_eq!(decode(bank, &b).format_status, FormatStatus::Unavailable);
            b[3] = Some(1);
            b[0] = Some(3);
            let d = known(bank, &b);
            assert!(
                d.fields.values().all(|f| f.status == Status::Unavailable
                    && f.raw.is_none()
                    && f.value.is_none()),
                "Part253 read beyond header"
            );
            b[0] = Some(bank.last());
            b.truncate(4);
            assert!(known(bank, &b)
                .fields
                .values()
                .all(|f| f.status == Status::Unavailable));
        }
        let mut b = capture(Bank::LightSource);
        set(&mut b, 18, 2, 481);
        set(&mut b, 20, 2, 1234);
        b[19] = None;
        let d = known(Bank::LightSource, &b);
        assert_eq!(d.fields["voltage"].status, Status::Unavailable);
        value(&d.fields["current"], 1234, "1.234", Some("A"));
    }
}
