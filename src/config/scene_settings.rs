//! Private desired scene data; these raw native values are never live output intents.
use crate::dali::{colour, gear, scenes};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub output_origin: crate::inventory::Origin,
    pub setting: Setting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawSetting")]
pub struct Setting {
    pub level: Level,
    pub color: Color,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSetting {
    level: Level,
    color: Color,
}
impl TryFrom<RawSetting> for Setting {
    type Error = String;
    fn try_from(raw: RawSetting) -> Result<Self, String> {
        let setting = Self {
            level: raw.level,
            color: raw.color,
        };
        setting.validate()?;
        Ok(setting)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Level {
    Value { value: u8 },
    Mask {},
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Color {
    Mask {},
    Temperature {
        #[serde(deserialize_with = "required_nullable")]
        value: Option<u16>,
    },
    Xy {
        #[serde(deserialize_with = "required_nullable")]
        x: Option<u16>,
        #[serde(deserialize_with = "required_nullable")]
        y: Option<u16>,
    },
    Rgbwaf {
        channels: [Option<u8>; 6],
        #[serde(deserialize_with = "required_nullable")]
        control: Option<u8>,
    },
}
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
impl Setting {
    pub fn ignored() -> Self {
        Self {
            level: Level::Mask {},
            color: Color::Mask {},
        }
    }
    pub fn validate(self) -> Result<(), String> {
        self.native_level()?;
        self.native_color()?;
        Ok(())
    }
    pub fn native_level(self) -> Result<scenes::Level, String> {
        match self.level {
            Level::Mask {} => Ok(scenes::Level::Mask),
            Level::Value { value } => gear::Level::new(value)
                .map(scenes::Level::Known)
                .map_err(|e| e.to_string()),
        }
    }
    pub fn native_color(self) -> Result<Option<scenes::Colour>, String> {
        let result = match self.color {
            Color::Mask {} => return Ok(None),
            Color::Temperature { value } => scenes::Colour::Temperature(
                value
                    .map(colour::Temperature::new)
                    .transpose()
                    .map_err(|e| e.to_string())?,
            ),
            Color::Xy { x, y } => scenes::Colour::Xy {
                x: x.map(colour::Coordinate::new)
                    .transpose()
                    .map_err(|e| e.to_string())?,
                y: y.map(colour::Coordinate::new)
                    .transpose()
                    .map_err(|e| e.to_string())?,
            },
            Color::Rgbwaf { channels, control } => {
                let mut checked = [None; 6];
                for (index, value) in channels.into_iter().enumerate() {
                    checked[index] = value
                        .map(colour::Channel::new)
                        .transpose()
                        .map_err(|e| e.to_string())?;
                }
                scenes::Colour::Rgbwaf {
                    channels: checked,
                    control: control
                        .map(scenes::RgbwafControl::new)
                        .transpose()
                        .map_err(|e| e.to_string())?,
                }
            }
        };
        Ok(Some(result))
    }
}
