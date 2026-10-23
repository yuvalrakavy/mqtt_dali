//! Selected public colour family; physical evidence remains owned by output.
use super::*;
use crate::{config::ColorRepresentation, dali::colour as codec};
use codec::{ByteValue, ColourType, Reply, ValueQuery, WordValue};
use output::colour::{Observation, Quality, Value as SampleValue};

pub(super) const FIELDS: [(&str, ValueQuery); 9] = [
    ("ColorTemperature", ValueQuery::Temperature),
    ("ColorX", ValueQuery::X),
    ("ColorY", ValueQuery::Y),
    ("ColorRed", ValueQuery::Red),
    ("ColorGreen", ValueQuery::Green),
    ("ColorBlue", ValueQuery::Blue),
    ("ColorWhite", ValueQuery::White),
    ("ColorAmber", ValueQuery::Amber),
    ("ColorFree", ValueQuery::Free),
];
const CHANNELS: [&str; 6] = ["red", "green", "blue", "white", "amber", "free"];

fn kind(representation: ColorRepresentation) -> ColourType {
    match representation {
        ColorRepresentation::Temperature => ColourType::Temperature,
        ColorRepresentation::Xy => ColourType::Xy,
        ColorRepresentation::Rgbwaf => ColourType::Rgbwaf,
    }
}
fn name(kind: ColourType) -> &'static str {
    match kind {
        ColourType::Temperature => "temperature",
        ColourType::Xy => "xy",
        ColourType::Rgbwaf => "rgbwaf",
        ColourType::PrimaryN => "primary_n",
    }
}
fn belongs(query: ValueQuery, representation: ColorRepresentation) -> bool {
    match representation {
        ColorRepresentation::Temperature => query == ValueQuery::Temperature,
        ColorRepresentation::Xy => matches!(query, ValueQuery::X | ValueQuery::Y),
        ColorRepresentation::Rgbwaf => matches!(
            query,
            ValueQuery::Red
                | ValueQuery::Green
                | ValueQuery::Blue
                | ValueQuery::White
                | ValueQuery::Amber
                | ValueQuery::Free
        ),
    }
}
pub(super) fn is_property(name: &str) -> bool {
    FIELDS.iter().any(|(field, _)| *field == name)
}
pub(super) fn configured(snapshot: &Snapshot, target: &str) -> Option<ColorRepresentation> {
    snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .and_then(|p| p.color_representation)
}
fn rejected(error: impl ToString) -> OpError {
    OpError::Rejected(error.to_string())
}
fn number(values: &PropertyMap, field: &str) -> Result<Option<f64>, OpError> {
    values
        .get(field)
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| rejected(format!("{field} must be a JSON number")))
        })
        .transpose()
}
pub(super) fn intent(
    values: &PropertyMap,
    representation: Option<ColorRepresentation>,
) -> Result<Option<codec::Intent>, OpError> {
    if !FIELDS.iter().any(|(field, _)| values.contains_key(*field)) {
        return Ok(None);
    }
    let representation =
        representation.ok_or_else(|| rejected("no public color representation is configured"))?;
    for (field, query) in FIELDS {
        if values.contains_key(field) && !belongs(query, representation) {
            return Err(rejected(format!(
                "{field} does not belong to the configured color representation"
            )));
        }
    }
    let colour = match representation {
        ColorRepresentation::Temperature => codec::Intent::temperature(
            codec::Temperature::from_kelvin(
                number(values, "ColorTemperature")?.expect("validated family has a supplied field"),
            )
            .map_err(rejected)?,
        ),
        ColorRepresentation::Xy => codec::Intent::xy_parts(
            number(values, "ColorX")?
                .map(codec::Coordinate::from_normalized)
                .transpose()
                .map_err(rejected)?,
            number(values, "ColorY")?
                .map(codec::Coordinate::from_normalized)
                .transpose()
                .map_err(rejected)?,
        )
        .map_err(rejected)?,
        ColorRepresentation::Rgbwaf => {
            let mut channels = [None; 6];
            for (slot, (field, _)) in FIELDS[3..].iter().enumerate() {
                channels[slot] = number(values, field)?
                    .map(codec::Channel::from_normalized)
                    .transpose()
                    .map_err(rejected)?;
            }
            codec::Intent::rgbwaf(channels).map_err(rejected)?
        }
    };
    Ok(Some(colour))
}

pub(super) fn project(
    values: &mut PropertyMap,
    representation: Option<ColorRepresentation>,
    observed: Option<&output::Observation>,
    live: bool,
) {
    let sample = observed.and_then(|o| o.colour.as_ref());
    let stale = !live
        || observed.is_some_and(|o| o.quality == output::Quality::Stale)
        || sample.is_some_and(|s| s.quality == Quality::Stale);
    project_sample(values, representation, sample, stale);
}

pub(super) fn project_sample(
    values: &mut PropertyMap,
    representation: Option<ColorRepresentation>,
    sample: Option<&Observation>,
    stale: bool,
) {
    let quality = match (representation, sample) {
        (None, _) => "unconfigured",
        (_, None) => "unobserved",
        (_, Some(_)) if stale => "stale",
        (_, Some(s)) => match s.quality {
            Quality::Stale => "stale",
            Quality::Unavailable => "unavailable",
            Quality::Unsupported => "unsupported",
            Quality::Partial => "partial",
            Quality::Current if s.active() != representation.map(kind) => "representation_mismatch",
            Quality::Current
                if representation == Some(ColorRepresentation::Rgbwaf)
                    && !matches!(s.control, Some(Reply::Value(raw)) if raw >> 6 == 2) =>
            {
                "unsupported_mode"
            }
            Quality::Current => "current",
        },
    };
    let fresh = sample.filter(|_| !stale);
    values.insert(
        "ColorRepresentation".into(),
        representation
            .map(|r| Value::from(name(kind(r))))
            .unwrap_or(Value::Null),
    );
    values.insert("ColorAvailable".into(), Value::Bool(quality == "current"));
    values.insert("ColorQuality".into(), Value::from(quality));
    values.insert(
        "ActiveColorRepresentation".into(),
        fresh
            .and_then(Observation::active)
            .map(|k| Value::from(name(k)))
            .unwrap_or(Value::Null),
    );
    values.insert(
        "ColorCapabilities".into(),
        fresh.map(capabilities).unwrap_or(Value::Null),
    );
    for (field, query) in FIELDS {
        let actual = if quality == "current" && representation.is_some_and(|r| belongs(query, r)) {
            fresh
                .and_then(|s| s.value(query))
                .and_then(|value| match (query, value) {
                    (ValueQuery::Temperature, SampleValue::Word(WordValue::Known(raw)))
                        if raw > 0 =>
                    {
                        Some(1_000_000.0 / f64::from(raw))
                    }
                    (ValueQuery::X | ValueQuery::Y, SampleValue::Word(WordValue::Known(raw))) => {
                        Some(f64::from(raw) / 65536.0)
                    }
                    (_, SampleValue::Byte(ByteValue::Known(raw))) => Some(f64::from(raw) / 254.0),
                    _ => None,
                })
        } else {
            None
        };
        values.insert(field.into(), actual.map(Value::from).unwrap_or(Value::Null));
    }
}
fn capabilities(sample: &Observation) -> Value {
    let features = match sample.features {
        Some(Reply::Value(f)) if f.counts_valid() => Some(f),
        _ => None,
    };
    let formats = match sample.dt8 {
        Some(false) => Some(vec![]),
        Some(true) => features.map(|f| {
            let mut formats = vec![];
            if f.temperature() {
                formats.push("temperature");
            }
            if f.xy() {
                formats.push("xy");
            }
            if f.rgbwaf_channels() > 0 {
                formats.push("rgbwaf");
            }
            if f.primary_count() > 0 {
                formats.push("primary_n");
            }
            formats
        }),
        None => None,
    };
    let assignments = features.map(|_| {
        sample
            .assignments
            .iter()
            .map(|reply| match reply {
                Reply::Value(assignment) => assignment.slot().map(|slot| CHANNELS[slot]),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    let limits = features.filter(|f| f.temperature()).map(|_| {
        let kelvin = |query| match sample.values.get(&query) {
            Some(Reply::Value(SampleValue::Word(WordValue::Known(raw)))) if *raw > 0 => Some(1_000_000.0 / f64::from(*raw)),
            _ => None,
        };
        serde_json::json!({"coolest_kelvin":kelvin(ValueQuery::Coolest),"warmest_kelvin":kelvin(ValueQuery::Warmest),"physical_coolest_kelvin":kelvin(ValueQuery::PhysicalCoolest),"physical_warmest_kelvin":kelvin(ValueQuery::PhysicalWarmest)})
    });
    serde_json::json!({
        "dt8":sample.dt8, "supported_representations":formats,
        "primary_count":features.map(|f| f.primary_count()),
        "rgbwaf_channel_count":features.map(|f| f.rgbwaf_channels()),
        "rgbwaf_assignments":assignments,
        "rgbwaf_control":match sample.control { Some(Reply::Value(raw)) => Some(raw), _ => None },
        "automatic_activation":match sample.gear_features { Some(Reply::Value(raw)) => Some(raw & 1 != 0), _ => None },
        "temperature_limits":limits,
    })
}
