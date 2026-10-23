//! Aggregate current native color samples; never query a DALI group for replies.
use super::*;
use crate::config::ColorRepresentation;

fn common_capabilities(rows: &[PropertyMap]) -> Value {
    if rows.is_empty() {
        return Value::Null;
    }
    let Some(caps) = rows
        .iter()
        .map(|row| row.get("ColorCapabilities")?.as_object())
        .collect::<Option<Vec<_>>>()
    else {
        return Value::Null;
    };
    let flags = caps
        .iter()
        .map(|cap| cap.get("dt8").and_then(Value::as_bool))
        .collect::<Vec<_>>();
    let dt8 = if flags.contains(&Some(false)) {
        Some(false)
    } else if flags.iter().all(|v| *v == Some(true)) {
        Some(true)
    } else {
        None
    };
    let formats = caps
        .iter()
        .map(|cap| cap.get("supported_representations")?.as_array())
        .collect::<Option<Vec<_>>>()
        .map(|sets| {
            sets[0]
                .iter()
                .filter(|value| sets.iter().all(|set| set.contains(value)))
                .cloned()
                .collect::<Vec<_>>()
        });
    let channels = caps
        .iter()
        .map(|cap| cap.get("rgbwaf_assignments")?.as_array())
        .collect::<Option<Vec<_>>>()
        .map(|sets| {
            ["red", "green", "blue", "white", "amber", "free"]
                .into_iter()
                .filter(|name| {
                    sets.iter()
                        .all(|set| set.iter().any(|v| v.as_str() == Some(name)))
                })
                .collect::<Vec<_>>()
        });
    let limit = |key: &str, upper: bool| {
        caps.iter()
            .map(|cap| cap.get("temperature_limits")?.get(key)?.as_f64())
            .collect::<Option<Vec<_>>>()
            .map(|values| {
                values
                    .into_iter()
                    .reduce(|a, b| if upper { a.min(b) } else { a.max(b) })
                    .unwrap()
            })
    };
    let limits = match (
        limit("coolest_kelvin", true),
        limit("warmest_kelvin", false),
    ) {
        (Some(coolest), Some(warmest)) if warmest <= coolest => {
            serde_json::json!({"coolest_kelvin":coolest,"warmest_kelvin":warmest})
        }
        _ => Value::Null,
    };
    serde_json::json!({"dt8":dt8,"supported_representations":formats,"common_rgbwaf_assignments":channels,"temperature_limits":limits})
}

pub(super) fn project(
    values: &mut PropertyMap,
    representation: Option<ColorRepresentation>,
    observation: Option<&group_output::Observation>,
    stale: bool,
) {
    project_captured(
        values,
        representation,
        observation.map(|o| o.report().members.as_slice()),
        observation.and_then(group_output::Observation::color_observations),
        stale,
    );
}
fn project_captured(
    values: &mut PropertyMap,
    representation: Option<ColorRepresentation>,
    members: Option<&[group_output::Member]>,
    colors: Option<&std::collections::BTreeMap<u8, output::colour::Observation>>,
    stale: bool,
) {
    let observed = members.is_some();
    let members = members.filter(|_| !stale);
    let complete =
        members.is_some_and(|members| members.iter().all(|member| member.member.is_some()));
    let actual = members
        .map(|members| {
            members
                .iter()
                .filter(|member| member.member == Some(true))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let colors = colors.filter(|_| !stale);
    let mut rows = vec![];
    if let Some(colors) = colors {
        for member in &actual {
            let mut fields = PropertyMap::new();
            super::colour::project_sample(
                &mut fields,
                representation,
                colors.get(&member.address),
                false,
            );
            rows.push(fields);
        }
    }
    let quality = if representation.is_none() {
        "unconfigured"
    } else if !observed {
        "unobserved"
    } else if stale {
        "stale"
    } else if !complete {
        "partial"
    } else if actual.is_empty() {
        "empty"
    } else if colors.is_none() {
        "unobserved"
    } else if rows.iter().any(|row| row["ColorQuality"] == "unsupported") {
        "unsupported"
    } else if rows.iter().all(|row| row["ColorQuality"] == "current") {
        "current"
    } else if rows
        .iter()
        .any(|row| row["ColorQuality"] == "unsupported_mode")
    {
        "unsupported_mode"
    } else if rows
        .iter()
        .any(|row| row["ColorQuality"] == "representation_mismatch")
    {
        "representation_mismatch"
    } else if rows.iter().all(|row| row["ColorQuality"] == "unavailable") {
        "unavailable"
    } else {
        "partial"
    };
    values.insert(
        "ColorRepresentation".into(),
        serde_json::to_value(representation).expect("typed color representation"),
    );
    values.insert("ColorQuality".into(), Value::from(quality));
    values.insert("ColorAvailable".into(), Value::Bool(quality == "current"));
    values.insert(
        "ColorCapabilities".into(),
        if complete {
            common_capabilities(&rows)
        } else {
            Value::Null
        },
    );
    let active = rows
        .iter()
        .map(|row| {
            row.get("ActiveColorRepresentation")
                .filter(|value| value.is_string())
        })
        .collect::<Option<Vec<_>>>();
    let (active, mixed) = match active.filter(|rows| complete && !rows.is_empty()) {
        Some(rows) => {
            let mixed = rows.iter().any(|value| *value != rows[0]);
            (
                if mixed { Value::Null } else { rows[0].clone() },
                Value::Bool(mixed),
            )
        }
        _ => (Value::Null, Value::Null),
    };
    values.insert("ActiveColorRepresentation".into(), active);
    values.insert("ActiveColorRepresentationMixed".into(), mixed);
    for (field, _) in super::colour::FIELDS {
        let samples = (quality == "current")
            .then(|| {
                rows.iter()
                    .map(|row| row.get(field).filter(|value| value.is_number()))
                    .collect::<Option<Vec<_>>>()
            })
            .flatten();
        let (value, mixed) = match samples.filter(|samples| !samples.is_empty()) {
            Some(samples) => {
                let mixed = samples.iter().any(|value| *value != samples[0]);
                (
                    if mixed {
                        Value::Null
                    } else {
                        samples[0].clone()
                    },
                    Value::Bool(mixed),
                )
            }
            _ => (Value::Null, Value::Null),
        };
        values.insert(field.into(), value);
        values.insert(format!("{field}Mixed"), mixed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dali::colour::{self as codec, Reply, ValueQuery, WordValue};
    use output::colour::{Observation, Quality, Value as Sample};
    #[test]
    fn group_color_aggregate_clears_unknowns_and_is_independent_of_base_mask() {
        let member = |address| group_output::Member {
            target: format!("L{address}"),
            target_generation: "epoch.1".into(),
            address,
            expected_member: true,
            member: Some(true),
            membership_low: None,
            membership_high: None,
            actual_level: Some(scenes::Reading::Mask { raw: 255 }),
            status: None,
        };
        let mut members = vec![member(0), member(1)];
        let sample = |temperature| Observation {
            dt8: Some(true),
            features: Some(Reply::Value(codec::Features::decode(2))),
            status: Some(Reply::Value(codec::Status::decode(0x20))),
            quality: Quality::Current,
            values: std::collections::BTreeMap::from([(
                ValueQuery::Temperature,
                Reply::Value(Sample::Word(WordValue::Known(temperature))),
            )]),
            ..Observation::default()
        };
        let mut colors = std::collections::BTreeMap::from([(0, sample(250)), (1, sample(300))]);
        let mut values = PropertyMap::from_iter([("Available".into(), Value::Bool(false))]);
        let representation = Some(ColorRepresentation::Temperature);
        project_captured(
            &mut values,
            representation,
            Some(&members),
            Some(&colors),
            false,
        );
        assert_eq!(
            values["ColorAvailable"], true,
            "base MASK suppressed complete group color"
        );
        assert_eq!(values["Available"], false);
        assert_eq!(values["ColorTemperature"], Value::Null);
        assert_eq!(
            values["ColorTemperatureMixed"], true,
            "mixed group color invented a uniform value"
        );
        colors.get_mut(&1).unwrap().values = colors[&0].values.clone();
        project_captured(
            &mut values,
            representation,
            Some(&members),
            Some(&colors),
            false,
        );
        assert_eq!(values["ColorTemperature"].as_f64(), Some(4000.0));
        assert_eq!(values["ColorTemperatureMixed"], false);
        for (membership, stale, quality) in [
            (Some(true), true, "stale"),
            (None, false, "partial"),
            (Some(false), false, "empty"),
        ] {
            for member in &mut members {
                member.member = membership;
            }
            project_captured(
                &mut values,
                representation,
                Some(&members),
                Some(&colors),
                stale,
            );
            assert_eq!(values["ColorQuality"], quality);
            for field in [
                "ColorTemperature",
                "ColorTemperatureMixed",
                "ActiveColorRepresentation",
                "ActiveColorRepresentationMixed",
            ] {
                assert_eq!(
                    values[field],
                    Value::Null,
                    "unknown group color retained merged evidence: {field}"
                );
            }
            assert_eq!(values["ColorAvailable"], false);
        }
        project_captured(&mut values, representation, None, None, false);
        assert_eq!(values["ColorQuality"], "unobserved");
        project_captured(&mut values, None, None, None, false);
        assert_eq!(values["ColorQuality"], "unconfigured");
    }
}
