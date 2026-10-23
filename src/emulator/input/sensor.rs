//! Independent sensor configuration profiles, without production codec imports.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    Absolute {
        report: u8,
        deadtime: u8,
    },
    Occupancy {
        report: u8,
        deadtime: u8,
        hold: u8,
    },
    Light {
        report: u8,
        deadtime: u8,
        hysteresis: u8,
        hysteresis_min: u8,
    },
}
impl Profile {
    pub fn validate(&self, kind: u8) -> Result<(), String> {
        if !matches!(
            (kind, self),
            (2, Self::Absolute { .. })
                | (3, Self::Occupancy { hold: 0..=254, .. })
                | (
                    4,
                    Self::Light {
                        hysteresis: 0..=25,
                        ..
                    }
                )
        ) {
            return Err("invalid sensor parameter family/range".into());
        }
        Ok(())
    }
    /// Deliberate emulator defaults. Occupancy models a supported hold timer;
    /// this is not a claim that all occupancy devices support hold or these resets.
    pub fn new(kind: u8) -> Option<Self> {
        Some(match kind {
            2 => Self::Absolute {
                report: 0,
                deadtime: 2,
            },
            3 => Self::Occupancy {
                report: 20,
                deadtime: 2,
                hold: 90,
            },
            4 => Self::Light {
                report: 30,
                deadtime: 30,
                hysteresis: 5,
                hysteresis_min: 0,
            },
            _ => return None,
        })
    }
    /// Outer None means unrecognized; inner None is a handled write/no reply.
    /// Repeat admission belongs to the existing bus owner. Invalid writes are
    /// ignored by this limited profile, without claiming universal clamp rules.
    pub fn command(&mut self, opcode: u8, dtr0: u8) -> Option<Option<u8>> {
        match self {
            Self::Absolute { report, deadtime } => match opcode {
                0x10 => *report = dtr0,
                0x11 => *deadtime = dtr0,
                0x1d => return Some(Some(*deadtime)),
                0x1e => return Some(Some(*report)),
                _ => return None,
            },
            Self::Occupancy {
                report,
                deadtime,
                hold,
            } => match opcode {
                0x21 => {
                    if dtr0 != 255 {
                        *hold = dtr0;
                    }
                }
                0x22 => *report = dtr0,
                0x23 => *deadtime = dtr0,
                0x2c => return Some(Some(*deadtime)),
                0x2d => return Some(Some(*hold)),
                0x2e => return Some(Some(*report)),
                _ => return None,
            },
            Self::Light {
                report,
                deadtime,
                hysteresis,
                hysteresis_min,
            } => match opcode {
                0x30 => *report = dtr0,
                0x31 => {
                    if dtr0 <= 25 {
                        *hysteresis = dtr0;
                    }
                }
                0x32 => *deadtime = dtr0,
                0x33 => *hysteresis_min = dtr0,
                0x3c => return Some(Some(*hysteresis_min)),
                0x3d => return Some(Some(*deadtime)),
                0x3e => return Some(Some(*report)),
                0x3f => return Some(Some(*hysteresis)),
                _ => return None,
            },
        }
        Some(None)
    }
}
