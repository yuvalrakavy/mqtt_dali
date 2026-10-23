//! Optional, freshly proved DT7 settings within the existing output operation.
use super::*;
use crate::{
    atx::wire::DaliFrame,
    dali::gear::{switching as frames, Query, ShortAddress},
    service::{
        commissioning::{Bus, Error},
        scenes::Reading,
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub upward_on: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub upward_off: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub downward_on: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub downward_off: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub features_raw: Option<u8>,
    pub adjustable_thresholds: Option<bool>,
    pub physical_selection: Option<bool>,
    pub switch_status_raw: Option<u8>,
    pub gear_type_raw: Option<u8>,
    pub extended_version_raw: Option<u8>,
    pub upward_on: Option<u8>,
    pub upward_off: Option<u8>,
    pub downward_on: Option<u8>,
    pub downward_off: Option<u8>,
    pub quality: &'static str,
}
impl Patch {
    pub(super) fn validate(&self) -> Result<(), ServiceError> {
        if self == &Self::default() {
            return Err(reject("change at least one relay threshold"));
        }
        for raw in [self.upward_on, self.upward_off].into_iter().flatten() {
            frames::UpThreshold::new(raw).map_err(reject)?;
        }
        Ok(())
    }
    fn current<'a>(&self, values: &'a super::Values) -> Option<&'a Values> {
        values.switching.as_ref().filter(|v| {
            values.switching_support == Some(true) && v.adjustable_thresholds == Some(true)
        })
    }
    pub(super) fn plans(
        &self,
        values: &super::Values,
    ) -> Result<Vec<frames::Setting>, ServiceError> {
        self.validate()?;
        let current = self
            .current(values)
            .ok_or_else(|| reject("fresh adjustable DT7 support required"))?;
        let mut plans = Vec::new();
        for (requested, readable, up, on) in [
            (self.upward_on, current.upward_on, true, true),
            (self.upward_off, current.upward_off, true, false),
            (self.downward_on, current.downward_on, false, true),
            (self.downward_off, current.downward_off, false, false),
        ] {
            let Some(raw) = requested else {
                continue;
            };
            if readable.is_none() {
                return Err(reject("fresh requested relay threshold required"));
            }
            plans.push(match (up, on) {
                (true, true) => {
                    frames::Setting::UpOn(frames::UpThreshold::new(raw).map_err(reject)?)
                }
                (true, false) => {
                    frames::Setting::UpOff(frames::UpThreshold::new(raw).map_err(reject)?)
                }
                (false, true) => frames::Setting::DownOn(frames::Threshold::from_raw(raw)),
                (false, false) => frames::Setting::DownOff(frames::Threshold::from_raw(raw)),
            });
        }
        Ok(plans)
    }
    pub(super) fn matches(&self, values: &super::Values) -> bool {
        self.current(values).is_some_and(|v| {
            self.upward_on.is_none_or(|x| v.upward_on == Some(x))
                && self.upward_off.is_none_or(|x| v.upward_off == Some(x))
                && self.downward_on.is_none_or(|x| v.downward_on == Some(x))
                && self.downward_off.is_none_or(|x| v.downward_off == Some(x))
        })
    }
}
fn byte(reading: &Reading) -> Option<u8> {
    if let Reading::Value { raw } = reading {
        u8::try_from(*raw).ok()
    } else {
        None
    }
}
const TYPE_KEYS: [&str; 9] = [
    "device_type_0",
    "device_type_1",
    "device_type_2",
    "device_type_3",
    "device_type_4",
    "device_type_5",
    "device_type_6",
    "device_type_7",
    "device_type_8",
];
pub(super) async fn probe(
    bus: &Bus<'_>,
    address: ShortAddress,
    readings: &mut BTreeMap<&'static str, Reading>,
) -> Result<(Option<bool>, Option<Values>), Error> {
    let first = Reading::byte(
        bus.query(DaliFrame::Bits16(address.query(Query::DeviceType)))
            .await?,
        false,
    );
    let mut support = match byte(&first) {
        Some(7) => Some(true),
        Some(255) | None => None,
        Some(_) => Some(false),
    };
    if byte(&first) == Some(255) {
        let mut previous = None;
        for (n, key) in TYPE_KEYS.into_iter().enumerate() {
            let next = Reading::byte(
                bus.query(DaliFrame::Bits16(address.query(Query::NextDeviceType)))
                    .await?,
                false,
            );
            let raw = byte(&next);
            readings.insert(key, next);
            match raw {
                Some(254) => {
                    support = (n >= 2).then_some(false);
                    break;
                }
                Some(v) if v < 254 && previous.is_none_or(|p| v > p) => {
                    if v >= 7 {
                        support = Some(v == 7);
                        break;
                    }
                    previous = Some(v);
                }
                _ => break,
            }
        }
    }
    readings.insert("device_type", first);
    if support != Some(true) {
        return Ok((support, None));
    }
    for (name, query) in [
        ("switching_features", frames::Query::Features),
        ("switching_status", frames::Query::SwitchStatus),
        ("switching_upward_on", frames::Query::UpOn),
        ("switching_upward_off", frames::Query::UpOff),
        ("switching_downward_on", frames::Query::DownOn),
        ("switching_downward_off", frames::Query::DownOff),
        ("switching_gear_type", frames::Query::GearType),
        ("switching_version", frames::Query::ExtendedVersion),
    ] {
        let [enable, query] = query.plan(address);
        bus.command(DaliFrame::Bits16(enable), false).await?;
        readings.insert(
            name,
            Reading::byte(bus.query(DaliFrame::Bits16(query)).await?, false),
        );
    }
    let features_raw = byte(&readings["switching_features"]);
    let features = features_raw.and_then(|raw| {
        match frames::Query::Features.decode(crate::atx::transactions::BusReply::Value(raw)) {
            frames::Reply::Value(frames::Value::Features(f)) => Some(f),
            _ => None,
        }
    });
    let mut values = Values {
        features_raw,
        adjustable_thresholds: features.map(|f| f.adjustable_thresholds()),
        physical_selection: features.map(|f| f.physical_selection()),
        switch_status_raw: byte(&readings["switching_status"]),
        gear_type_raw: byte(&readings["switching_gear_type"]),
        extended_version_raw: byte(&readings["switching_version"]),
        upward_on: byte(&readings["switching_upward_on"]).filter(|v| *v != 0),
        upward_off: byte(&readings["switching_upward_off"]).filter(|v| *v != 0),
        downward_on: byte(&readings["switching_downward_on"]),
        downward_off: byte(&readings["switching_downward_off"]),
        quality: "partial",
    };
    if [
        values.features_raw,
        values.switch_status_raw,
        values.gear_type_raw,
        values.extended_version_raw,
        values.upward_on,
        values.upward_off,
        values.downward_on,
        values.downward_off,
    ]
    .into_iter()
    .all(|v| v.is_some())
    {
        values.quality = "complete";
    }
    Ok((support, Some(values)))
}
