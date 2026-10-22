//! Independently authored physical DT8 model; never imports the bridge codec.
use super::scenario::{Colour, ColourType};
use serde::Serialize;

#[derive(Serialize)]
struct Temporary {
    kind: Option<ColourType>,
    xy: [u16; 2],
    temperature: u16,
    rgbwaf: [u8; 6],
}
impl Default for Temporary {
    fn default() -> Self {
        Self {
            kind: None,
            xy: [65535; 2],
            temperature: 65535,
            rgbwaf: [255; 6],
        }
    }
}

#[derive(Serialize)]
pub(super) struct State {
    actual: Colour,
    temporary: Temporary,
    temperature_out_of_range: bool,
}
impl State {
    pub fn new(profile: Colour) -> Self {
        Self {
            actual: profile,
            temporary: Temporary::default(),
            temperature_out_of_range: false,
        }
    }
    fn select(&mut self, kind: ColourType) {
        if self.temporary.kind != Some(kind) {
            self.temporary = Temporary::default();
        }
        self.temporary.kind = Some(kind);
    }
    fn activate(&mut self) {
        let staged = std::mem::take(&mut self.temporary);
        match staged.kind {
            Some(ColourType::Temperature) if staged.temperature != 65535 => {
                if let Some(tc) = &mut self.actual.temperature {
                    let clamped = staged.temperature.clamp(tc.coolest, tc.warmest);
                    self.temperature_out_of_range = clamped != staged.temperature;
                    tc.value = clamped;
                    self.actual.active = ColourType::Temperature;
                }
            }
            Some(ColourType::Xy) => {
                if let Some(xy) = &mut self.actual.xy {
                    for (actual, proposed) in xy.iter_mut().zip(staged.xy) {
                        if proposed != 65535 {
                            *actual = proposed;
                        }
                    }
                    self.actual.active = ColourType::Xy;
                    self.temperature_out_of_range = false;
                }
            }
            Some(ColourType::Rgbwaf) => {
                if let Some(channels) = &mut self.actual.rgbwaf {
                    for (actual, proposed) in channels.iter_mut().zip(staged.rgbwaf) {
                        if proposed != 255 {
                            *actual = proposed;
                        }
                    }
                    self.actual.active = ColourType::Rgbwaf;
                    self.temperature_out_of_range = false;
                }
            }
            _ => {}
        }
    }
    /// Outer None is an unsupported opcode; inner None is a non-query command.
    pub fn command(&mut self, opcode: u8, dtr: &mut [u8; 3]) -> Option<Option<u8>> {
        let word = u16::from_le_bytes([dtr[0], dtr[1]]);
        let reply = match opcode {
            0xe0 | 0xe1 => {
                if self.actual.xy.is_some() {
                    self.select(ColourType::Xy);
                    self.temporary.xy[usize::from(opcode - 0xe0)] = word;
                }
                None
            }
            0xe7 => {
                if self.actual.temperature.is_some() && word != 0 {
                    self.select(ColourType::Temperature);
                    self.temporary.temperature = word;
                }
                None
            }
            0xeb | 0xec => {
                if self.actual.rgbwaf.is_some() {
                    self.select(ColourType::Rgbwaf);
                    let offset = if opcode == 0xeb { 0 } else { 3 };
                    self.temporary.rgbwaf[offset..offset + 3].copy_from_slice(dtr);
                }
                None
            }
            0xe2 => {
                self.activate();
                None
            }
            0xf7 => Some(0), // Manual activation, no auto-calibration model.
            0xf8 => Some(self.status()),
            0xf9 => Some(
                u8::from(self.actual.xy.is_some())
                    | (u8::from(self.actual.temperature.is_some()) << 1)
                    | ((self.actual.rgbwaf.as_ref().map_or(0, Vec::len) as u8) << 5),
            ),
            0xfa => Some(self.value(dtr)),
            0xfb => Some(if self.actual.rgbwaf.is_some() {
                0x80
            } else {
                255
            }),
            0xff => Some(1), // Emulated profile's raw extended version.
            _ => return None,
        };
        Some(reply)
    }
    fn status(&self) -> u8 {
        let kind = match self.actual.active {
            ColourType::Xy => 0x10,
            ColourType::Temperature => 0x20,
            ColourType::Rgbwaf => 0x80,
        };
        kind | (u8::from(self.temperature_out_of_range) << 1)
    }
    fn value(&self, dtr: &mut [u8; 3]) -> u8 {
        let selector = dtr[0];
        let word = match selector {
            0 | 1 if self.actual.active == ColourType::Xy => {
                self.actual.xy.map(|xy| xy[usize::from(selector)])
            }
            2 if self.actual.active == ColourType::Temperature => {
                self.actual.temperature.as_ref().map(|tc| tc.value)
            }
            128..=131 => self.actual.temperature.as_ref().map(|tc| match selector {
                128 => tc.coolest,
                129 => tc.physical_coolest,
                130 => tc.warmest,
                _ => tc.physical_warmest,
            }),
            9..=15 => {
                return if self.actual.active != ColourType::Rgbwaf {
                    255
                } else if selector == 15 {
                    0x80
                } else {
                    self.actual
                        .rgbwaf
                        .as_ref()
                        .and_then(|channels| channels.get(usize::from(selector - 9)))
                        .copied()
                        .unwrap_or(255)
                };
            }
            _ => None,
        }
        .unwrap_or(65535);
        let [high, low] = word.to_be_bytes();
        dtr[1] = high;
        dtr[0] = low;
        high
    }
}
