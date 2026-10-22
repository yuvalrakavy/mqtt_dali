//! Exact native scene data conversion, separate from live colour intents.
use super::{Quality, Reading, Stored};
use crate::{
    config::scene_settings::{Color, Level, Setting},
    inventory::{Location, Model},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    Unmanaged,
    Matching,
    Different,
    Unresolved,
}

pub(in crate::service) fn saved(model: &Model, bus: u8, slot: u8, address: u8) -> Option<Setting> {
    let origin = model.origin(Location::Output {
        bus,
        short_address: address,
    })?;
    model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == bus)?
        .scenes
        .iter()
        .find(|s| s.slot == slot)?
        .contents
        .iter()
        .find(|row| &row.output_origin == origin)
        .map(|row| row.setting)
}
pub(in crate::service) fn compare(saved: Option<Setting>, stored: Option<&Stored>) -> Comparison {
    let Some(saved) = saved else {
        return Comparison::Unmanaged;
    };
    match stored.and_then(|s| from_stored(s).ok()) {
        Some(actual) if actual == saved => Comparison::Matching,
        Some(_) => Comparison::Different,
        None => Comparison::Unresolved,
    }
}
fn raw(reading: &Reading, mask: u16) -> Result<Option<u16>, String> {
    match reading {
        Reading::Value { raw } if *raw < mask => Ok(Some(*raw)),
        Reading::Mask { raw } if *raw == mask => Ok(None),
        _ => Err("scene evidence is unresolved or out of range".into()),
    }
}
pub(in crate::service) fn from_stored(stored: &Stored) -> Result<Setting, String> {
    if stored.stale || stored.quality != Quality::Complete {
        return Err("complete current stored scene evidence required".into());
    }
    let level =
        raw(&stored.level, 255)?.map_or(Level::Mask {}, |v| Level::Value { value: v as u8 });
    let attr = |name: &str, mask| {
        raw(
            stored.values.get(name).ok_or("missing stored attribute")?,
            mask,
        )
    };
    let color = match stored.dt8 {
        Some(false) if stored.color_type.is_none() && stored.values.is_empty() => Color::Mask {},
        Some(true) => {
            let kind = raw(
                stored.color_type.as_ref().ok_or("missing stored type")?,
                255,
            )?;
            let (color, count) = match kind {
                None => (Color::Mask {}, 0),
                Some(0x20) => (
                    Color::Temperature {
                        value: attr("temperature", 65535)?,
                    },
                    1,
                ),
                Some(0x10) => (
                    Color::Xy {
                        x: attr("x", 65535)?,
                        y: attr("y", 65535)?,
                    },
                    2,
                ),
                Some(0x80) => {
                    let mut channels = [None; 6];
                    for (i, name) in ["red", "green", "blue", "white", "amber", "free"]
                        .iter()
                        .enumerate()
                    {
                        channels[i] = attr(name, 255)?.map(|v| v as u8);
                    }
                    (
                        Color::Rgbwaf {
                            channels,
                            control: attr("control", 255)?.map(|v| v as u8),
                        },
                        7,
                    )
                }
                _ => return Err("stored colour family is unsupported".into()),
            };
            if stored.values.len() != count {
                return Err("unexpected stored scene attributes".into());
            }
            color
        }
        _ => return Err("DT8 presence is unconfirmed".into()),
    };
    let setting = Setting { level, color };
    setting.validate()?;
    Ok(setting)
}
