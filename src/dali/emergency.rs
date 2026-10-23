//! Common Part202 queries and checked native plans; no transport or timers.
use crate::dali::gear::{settings::Step, Destination, ShortAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Battery,
    Timing(u8),
    DurationResult,
    EmergencyHours,
    TotalHours,
    Level,
    Minimum,
    Maximum,
    RatedDuration,
    Mode,
    Features,
    Failures,
    Status,
    ExtendedVersion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Known(u32),
    Bitmap(u8),
    Unknown,
    Invalid,
}
impl Query {
    pub fn plan(self, address: ShortAddress) -> Result<Vec<Step>, &'static str> {
        let mut plan = Vec::new();
        let opcode = match self {
            Self::Battery => 0xf1,
            Self::Timing(selector) if selector <= 7 => {
                plan.push(Step {
                    frame: 0xa300 | u16::from(selector),
                    repeat: false,
                });
                0xf2
            }
            Self::Timing(_) => return Err("emergency timing selector must be0..7"),
            Self::DurationResult => 0xf3,
            Self::EmergencyHours => 0xf4,
            Self::TotalHours => 0xf5,
            Self::Level => 0xf6,
            Self::Minimum => 0xf7,
            Self::Maximum => 0xf8,
            Self::RatedDuration => 0xf9,
            Self::Mode => 0xfa,
            Self::Features => 0xfb,
            Self::Failures => 0xfc,
            Self::Status => 0xfd,
            Self::ExtendedVersion => 0xff,
        };
        plan.extend([
            Step {
                frame: 0xc101,
                repeat: false,
            },
            Step {
                frame: Destination::Short(address).command(opcode),
                repeat: false,
            },
        ]);
        Ok(plan)
    }
    pub fn decode(self, raw: u8) -> Value {
        let value = u32::from(raw);
        match self {
            Self::Mode | Self::Features | Self::Failures | Self::Status => Value::Bitmap(raw),
            Self::Battery if raw == 255 => Value::Unknown,
            Self::Battery => Value::Known(value),
            Self::Level | Self::Minimum | Self::Maximum => match raw {
                0 => Value::Invalid,
                255 => Value::Unknown,
                _ => Value::Known(value),
            },
            Self::DurationResult | Self::RatedDuration => Value::Known(value * 120),
            Self::EmergencyHours => Value::Known(value),
            Self::TotalHours => Value::Known(value * 4),
            Self::Timing(0..=4) => Value::Known(value),
            Self::Timing(5) if raw <= 97 => Value::Known(value),
            Self::Timing(6) => Value::Known(if raw == 0 { 15 } else { value * 1440 }),
            Self::Timing(7) => Value::Known(value * 30),
            Self::Timing(_) => Value::Invalid,
            Self::ExtendedVersion => Value::Known(value),
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::DurationResult | Self::RatedDuration | Self::Timing(7) => "seconds",
            Self::EmergencyHours | Self::TotalHours => "hours",
            // A high/low component is raw; combine its atomic pair first.
            Self::Timing(0..=3) => "raw",
            Self::Timing(4) => "days",
            Self::Timing(5) => "weeks",
            Self::Timing(6) => "minutes",
            _ => "raw",
        }
    }
    pub fn saturated(self, raw: u8) -> bool {
        raw == 255
            && matches!(
                self,
                Self::DurationResult
                    | Self::EmergencyHours
                    | Self::TotalHours
                    | Self::RatedDuration
            )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    EmergencyLevel,
    ProlongSeconds,
    ExecutionTimeoutMinutes,
    FunctionIntervalDays,
    DurationIntervalWeeks,
    InitialDelayMinutes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    key: SettingKey,
    raw: u16,
}
impl Setting {
    pub fn new(key: SettingKey, value: u32) -> Result<Self, &'static str> {
        let raw = match key {
            SettingKey::EmergencyLevel if (1..=254).contains(&value) => value,
            SettingKey::ProlongSeconds if value <= 7650 && value.is_multiple_of(30) => value / 30,
            SettingKey::ExecutionTimeoutMinutes if value == 15 => 0,
            SettingKey::ExecutionTimeoutMinutes
                if (1440..=367200).contains(&value) && value.is_multiple_of(1440) =>
            {
                value / 1440
            }
            SettingKey::FunctionIntervalDays if value <= 255 => value,
            SettingKey::DurationIntervalWeeks if value <= 97 => value,
            SettingKey::InitialDelayMinutes if value <= 983010 && value.is_multiple_of(15) => {
                value / 15
            }
            _ => return Err("invalid emergency setting range or native unit multiple"),
        };
        Ok(Self {
            key,
            raw: raw as u16,
        })
    }
    pub fn plan(self, destination: Destination) -> Vec<Step> {
        let setter = |raw: u8, opcode| {
            [
                Step {
                    frame: 0xa300 | u16::from(raw),
                    repeat: false,
                },
                Step {
                    frame: 0xc101,
                    repeat: false,
                },
                Step {
                    frame: destination.command(opcode),
                    repeat: true,
                },
            ]
        };
        if self.key == SettingKey::InitialDelayMinutes {
            let [high, low] = self.raw.to_be_bytes();
            return setter(high, 0xea)
                .into_iter()
                .chain(setter(low, 0xeb))
                .collect();
        }
        let opcode = match self.key {
            SettingKey::EmergencyLevel => 0xe9,
            SettingKey::ProlongSeconds => 0xef,
            SettingKey::ExecutionTimeoutMinutes => 0xee,
            SettingKey::FunctionIntervalDays => 0xec,
            SettingKey::DurationIntervalWeeks => 0xed,
            SettingKey::InitialDelayMinutes => unreachable!(),
        };
        setter(self.raw as u8, opcode).into()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Test {
    Function,
    Duration,
}
pub fn control(test: Option<Test>, address: ShortAddress) -> Vec<Step> {
    vec![
        Step {
            frame: 0xc101,
            repeat: false,
        },
        Step {
            frame: Destination::Short(address).command(match test {
                Some(Test::Function) => 0xe3,
                Some(Test::Duration) => 0xe4,
                None => 0xe5,
            }),
            repeat: true,
        },
    ]
}
/// Initial QUERY DEVICE TYPE plus a bounded, ordered NEXT prefix for FF.
pub fn supports(evidence: &[Option<u8>], wanted: u8) -> Option<bool> {
    match *evidence.first()? {
        Some(0..=253) => return Some(evidence[0] == Some(wanted)),
        Some(255) => {}
        _ => return None,
    }
    let mut previous = None;
    for (index, reply) in evidence.iter().skip(1).take(9).enumerate() {
        let raw = (*reply)?;
        if raw == 254 {
            return (index >= 2).then_some(false);
        }
        if raw == 255 || previous.is_some_and(|old| raw <= old) {
            return None;
        }
        if raw == wanted {
            return Some(true);
        }
        if raw > wanted {
            return Some(false);
        }
        previous = Some(raw);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn address() -> ShortAddress {
        ShortAddress::new(3).unwrap()
    }
    fn steps(plan: Vec<Step>) -> Vec<(u16, bool)> {
        plan.into_iter().map(|s| (s.frame, s.repeat)).collect()
    }
    #[test]
    fn emergency_queries_use_dt1_and_atomic_timing_selector() {
        let queries = [
            Query::Battery,
            Query::DurationResult,
            Query::EmergencyHours,
            Query::TotalHours,
            Query::Level,
            Query::Minimum,
            Query::Maximum,
            Query::RatedDuration,
            Query::Mode,
            Query::Features,
            Query::Failures,
            Query::Status,
        ];
        for (query, opcode) in queries.into_iter().zip([
            0xf1, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd,
        ]) {
            assert_eq!(
                steps(query.plan(address()).unwrap()),
                vec![(0xc101, false), (0x0700 | opcode, false)]
            );
        }
        for selector in 0..=7 {
            assert_eq!(
                steps(Query::Timing(selector).plan(address()).unwrap()),
                vec![
                    (0xa300 | u16::from(selector), false),
                    (0xc101, false),
                    (0x07f2, false)
                ]
            );
        }
        assert!(Query::Timing(8).plan(address()).is_err());
    }
    #[test]
    fn emergency_ff_is_query_specific_and_counters_saturate() {
        for query in [Query::Mode, Query::Features, Query::Failures, Query::Status] {
            assert_eq!(query.decode(255), Value::Bitmap(255));
        }
        assert_eq!(Query::Battery.decode(255), Value::Unknown);
        assert_eq!(Query::Battery.decode(0), Value::Known(0));
        assert_eq!(Query::Battery.decode(254), Value::Known(254));
        for query in [Query::Level, Query::Minimum, Query::Maximum] {
            assert_eq!(query.decode(255), Value::Unknown);
            assert_eq!(query.decode(0), Value::Invalid);
            assert_eq!(query.decode(254), Value::Known(254));
        }
        for (query, unit, value) in [
            (Query::DurationResult, "seconds", 30600),
            (Query::EmergencyHours, "hours", 255),
            (Query::TotalHours, "hours", 1020),
            (Query::RatedDuration, "seconds", 30600),
        ] {
            assert_eq!(query.decode(255), Value::Known(value));
            assert!(query.saturated(255));
            assert!(!query.saturated(254));
            assert_eq!(query.unit(), unit);
        }
        assert_eq!(Query::Timing(4).decode(255), Value::Known(255));
        assert_eq!(Query::Timing(5).decode(98), Value::Invalid);
        assert_eq!(Query::Timing(6).decode(0), Value::Known(15));
        assert_eq!(Query::Timing(6).decode(255), Value::Known(367200));
        assert_eq!(Query::Timing(7).decode(255), Value::Known(7650));
    }
    #[test]
    fn emergency_setting_plans_repeat_one_enable_and_stage_delay() {
        let destination = Destination::Short(address());
        for (key, public, raw, opcode) in [
            (SettingKey::EmergencyLevel, 123, 123, 0xe9),
            (SettingKey::ProlongSeconds, 7650, 255, 0xef),
            (SettingKey::ExecutionTimeoutMinutes, 15, 0, 0xee),
            (SettingKey::ExecutionTimeoutMinutes, 367200, 255, 0xee),
            (SettingKey::FunctionIntervalDays, 255, 255, 0xec),
            (SettingKey::DurationIntervalWeeks, 97, 97, 0xed),
        ] {
            assert_eq!(
                steps(Setting::new(key, public).unwrap().plan(destination)),
                vec![
                    (0xa300 | raw, false),
                    (0xc101, false),
                    (0x0700 | opcode, true)
                ]
            );
        }
        assert_eq!(
            steps(
                Setting::new(SettingKey::InitialDelayMinutes, 0x1234 * 15)
                    .unwrap()
                    .plan(destination)
            ),
            vec![
                (0xa312, false),
                (0xc101, false),
                (0x07ea, true),
                (0xa334, false),
                (0xc101, false),
                (0x07eb, true)
            ]
        );
    }
    #[test]
    fn emergency_public_units_refuse_invalid_values_without_truncation() {
        for (key, invalid) in [
            (SettingKey::EmergencyLevel, vec![0, 255, u32::MAX]),
            (SettingKey::ProlongSeconds, vec![1, 7651, u32::MAX]),
            (
                SettingKey::ExecutionTimeoutMinutes,
                vec![0, 16, 1439, 1441, 367201],
            ),
            (SettingKey::FunctionIntervalDays, vec![256, u32::MAX]),
            (SettingKey::DurationIntervalWeeks, vec![98, 255, u32::MAX]),
            (SettingKey::InitialDelayMinutes, vec![1, 983025, u32::MAX]),
        ] {
            for value in invalid {
                assert!(Setting::new(key, value).is_err(), "{key:?} {value}");
            }
        }
    }
    #[test]
    fn emergency_tests_are_prompt_native_requests_not_result_resets() {
        for (test, frame) in [
            (Some(Test::Function), 0x07e3),
            (Some(Test::Duration), 0x07e4),
            (None, 0x07e5),
        ] {
            assert_eq!(
                steps(control(test, address())),
                vec![(0xc101, false), (frame, true)]
            );
        }
    }
    #[test]
    fn emergency_and_relay_type_evidence_never_guess_from_missing_or_bad_prefixes() {
        for wanted in [1, 7] {
            assert_eq!(supports(&[Some(wanted)], wanted), Some(true));
            assert_eq!(supports(&[Some(8)], wanted), Some(false));
            for evidence in [
                vec![],
                vec![None],
                vec![Some(254)],
                vec![Some(255), None],
                vec![Some(255), Some(0), Some(0)],
                vec![Some(255), Some(0), Some(255)],
            ] {
                assert_eq!(supports(&evidence, wanted), None, "{evidence:?}");
            }
            assert_eq!(supports(&[Some(255), Some(0), Some(254)], wanted), None);
        }
        assert_eq!(supports(&[Some(255), Some(1), Some(7)], 1), Some(true));
        assert_eq!(supports(&[Some(255), Some(1), Some(7)], 7), Some(true));
        assert_eq!(supports(&[Some(255), Some(0), Some(2)], 1), Some(false));
        assert_eq!(supports(&[Some(255), Some(0), Some(2)], 7), None);
        assert_eq!(
            supports(&[Some(255), Some(0), Some(2), Some(254)], 7),
            Some(false)
        );
        assert_eq!(supports(&[Some(255), Some(7), Some(1)], 1), Some(false));
    }
}
