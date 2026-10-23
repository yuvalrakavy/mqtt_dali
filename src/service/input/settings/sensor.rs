//! Optional sensor evidence under the existing input-settings physical owner.
use super::*;
use crate::dali::input::sensor::{self as frames, Parameter, Value};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub report: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub deadtime: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub hold: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub hysteresis: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub hysteresis_min: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    #[serde(skip)]
    kind: u8,
    pub report: Option<u8>,
    pub deadtime: Option<u8>,
    /// Raw FF is known unsupported; null is unknown/inapplicable.
    pub hold: Option<u8>,
    pub hold_supported: Option<bool>,
    pub hysteresis: Option<u8>,
    pub hysteresis_min: Option<u8>,
    pub quality: &'static str,
}
impl Values {
    fn complete(&self) -> bool {
        if self.report.is_none() || self.deadtime.is_none() {
            return false;
        }
        match self.kind {
            2 => {
                self.hold.is_none()
                    && self.hold_supported.is_none()
                    && self.hysteresis.is_none()
                    && self.hysteresis_min.is_none()
            }
            3 => {
                self.hold
                    .is_some_and(|v| self.hold_supported == Some(v != 255))
                    && self.hysteresis.is_none()
                    && self.hysteresis_min.is_none()
            }
            4 => {
                self.hold.is_none()
                    && self.hold_supported.is_none()
                    && self.hysteresis.is_some_and(|v| v <= 25)
                    && self.hysteresis_min.is_some()
            }
            _ => false,
        }
    }
}
impl Patch {
    pub(super) fn plans(&self, kind: u8) -> Result<Vec<Value>, ServiceError> {
        let kind = InstanceType::from_raw(kind);
        frames::parameters(kind).map_err(reject)?;
        let values = [
            (Parameter::Report, self.report),
            (Parameter::Deadtime, self.deadtime),
            (Parameter::Hold, self.hold),
            (Parameter::Hysteresis, self.hysteresis),
            (Parameter::HysteresisMinimum, self.hysteresis_min),
        ]
        .into_iter()
        .filter_map(|(p, v)| v.map(|v| Value::new(kind, p, v)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(reject)?;
        if values.is_empty() {
            return Err(reject("change at least one sensor setting"));
        }
        Ok(values)
    }
    pub(super) fn preflight(&self, actual: Option<&Values>) -> Result<(), &'static str> {
        let v = actual
            .filter(|v| v.quality == "complete" && v.complete())
            .ok_or("complete sensor setting evidence required before programming")?;
        if self.hold.is_some() && v.hold_supported != Some(true) {
            return Err("this sensor does not support a hold timer");
        }
        Ok(())
    }
    pub(super) fn matches(&self, v: &Values) -> bool {
        v.quality == "complete"
            && v.complete()
            && self.report.is_none_or(|x| v.report == Some(x))
            && self.deadtime.is_none_or(|x| v.deadtime == Some(x))
            && self
                .hold
                .is_none_or(|x| v.hold_supported == Some(true) && v.hold == Some(x))
            && self.hysteresis.is_none_or(|x| v.hysteresis == Some(x))
            && self
                .hysteresis_min
                .is_none_or(|x| v.hysteresis_min == Some(x))
    }
}
pub(super) async fn probe(bus: &Bus<'_>, route: &Route) -> Result<Values, Error> {
    let kind = route.input.kind;
    let native = InstanceType::from_raw(kind);
    let mut v = Values {
        kind,
        report: None,
        deadtime: None,
        hold: None,
        hold_supported: None,
        hysteresis: None,
        hysteresis_min: None,
        quality: "partial",
    };
    for p in frames::parameters(native).map_err(|_| Error::Evidence("unsupported sensor family"))? {
        let frame = p
            .query(native, route.input.address, route.input.instance)
            .map_err(|_| Error::Evidence("unsupported sensor parameter query"))?;
        let raw = byte(bus.query(DaliFrame::Bits24(frame)).await?);
        match p {
            Parameter::Report => v.report = raw,
            Parameter::Deadtime => v.deadtime = raw,
            Parameter::Hold => {
                v.hold = raw;
                v.hold_supported = raw.map(|v| v != 255);
            }
            Parameter::Hysteresis => v.hysteresis = raw,
            Parameter::HysteresisMinimum => v.hysteresis_min = raw,
        }
    }
    if v.complete() {
        v.quality = "complete";
    }
    Ok(v)
}
