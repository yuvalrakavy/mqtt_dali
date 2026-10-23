//! Optional timing evidence and patches; shares its parent's physical owner.
use super::*;
use crate::dali::input::button::{Timer, Value};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub short: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub double: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub repeat: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub stuck: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Values {
    pub short: Option<u8>,
    pub short_min: Option<u8>,
    pub double: Option<u8>,
    pub double_min: Option<u8>,
    pub repeat: Option<u8>,
    pub stuck: Option<u8>,
    pub quality: &'static str,
}
impl Values {
    fn complete(&self) -> bool {
        self.short.zip(self.short_min).is_some_and(|(v, m)| {
            Value::new(Timer::Short, v)
                .and_then(|v| v.check_minimum(m))
                .is_ok()
        }) && self.double.zip(self.double_min).is_some_and(|(v, m)| {
            Value::new(Timer::Double, v)
                .and_then(|v| v.check_minimum(m))
                .is_ok()
        }) && self
            .repeat
            .is_some_and(|v| Value::new(Timer::Repeat, v).is_ok())
            && self
                .stuck
                .is_some_and(|v| Value::new(Timer::Stuck, v).is_ok())
    }
}
impl Patch {
    pub(super) fn plans(&self) -> Result<Vec<Value>, ServiceError> {
        let values = [
            (Timer::Short, self.short),
            (Timer::Double, self.double),
            (Timer::Repeat, self.repeat),
            (Timer::Stuck, self.stuck),
        ]
        .into_iter()
        .filter_map(|(t, v)| v.map(|v| Value::new(t, v)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(reject)?;
        if values.is_empty() {
            return Err(reject("change at least one button timer"));
        }
        Ok(values)
    }
    pub(super) fn preflight(&self, actual: Option<&Values>) -> Result<(), &'static str> {
        let v = actual
            .filter(|v| v.quality == "complete" && v.complete())
            .ok_or("complete button timing evidence required before programming")?;
        for value in self.plans().map_err(|_| "invalid button timing patch")? {
            let minimum = match value.timer() {
                Timer::Short => v.short_min,
                Timer::Double => v.double_min,
                Timer::Repeat | Timer::Stuck => continue,
            }
            .ok_or("button timer minimum is unavailable")?;
            value
                .check_minimum(minimum)
                .map_err(|_| "requested button timing violates the device minimum")?;
        }
        Ok(())
    }
    pub(super) fn matches(&self, actual: &Values) -> bool {
        actual.quality == "complete"
            && actual.complete()
            && self.short.is_none_or(|v| actual.short == Some(v))
            && self.double.is_none_or(|v| actual.double == Some(v))
            && self.repeat.is_none_or(|v| actual.repeat == Some(v))
            && self.stuck.is_none_or(|v| actual.stuck == Some(v))
    }
}
pub(super) async fn probe(bus: &Bus<'_>, route: &Route) -> Result<Values, Error> {
    let address = route.input.address;
    let instance = route.input.instance;
    let mut bytes = [None; 6];
    for (index, frame) in [
        Timer::Short.query(address, instance),
        Timer::Short.minimum_query(address, instance).unwrap(),
        Timer::Double.query(address, instance),
        Timer::Double.minimum_query(address, instance).unwrap(),
        Timer::Repeat.query(address, instance),
        Timer::Stuck.query(address, instance),
    ]
    .into_iter()
    .enumerate()
    {
        bytes[index] = byte(bus.query(DaliFrame::Bits24(frame)).await?);
    }
    let [short, short_min, double, double_min, repeat, stuck] = bytes;
    let mut values = Values {
        short,
        short_min,
        double,
        double_min,
        repeat,
        stuck,
        quality: "partial",
    };
    if values.complete() {
        values.quality = "complete";
    }
    Ok(values)
}
