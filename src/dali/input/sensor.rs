//! Checked Part 302–304 event parameters. Pure plans, not capability evidence.
//! Owners must establish type/support and verify actual readback. Hold a whole
//! plan under one bus permit; its final step repeats as ordinary24-bit sends.
use super::{settings::Step, Address, Instance, InstanceType};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parameter {
    Report,
    Deadtime,
    Hold,
    Hysteresis,
    HysteresisMinimum,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("input type {0:?} has no supported sensor parameter family")]
    Kind(InstanceType),
    #[error("sensor parameter {parameter:?} does not apply to {kind:?}")]
    Parameter {
        kind: InstanceType,
        parameter: Parameter,
    },
    #[error("sensor parameter {parameter:?} raw value {raw} is out of range")]
    Range { parameter: Parameter, raw: u8 },
}
pub fn parameters(kind: InstanceType) -> Result<&'static [Parameter], Error> {
    use Parameter::*;
    Ok(match kind {
        InstanceType::Absolute => &[Report, Deadtime],
        InstanceType::Occupancy => &[Report, Deadtime, Hold],
        InstanceType::Light => &[Report, Deadtime, Hysteresis, HysteresisMinimum],
        _ => return Err(Error::Kind(kind)),
    })
}
impl Parameter {
    fn opcodes(self, kind: InstanceType) -> Result<(u8, u8), Error> {
        use InstanceType::*;
        Ok(match (kind, self) {
            (Absolute, Self::Report) => (0x10, 0x1e),
            (Absolute, Self::Deadtime) => (0x11, 0x1d),
            (Occupancy, Self::Hold) => (0x21, 0x2d),
            (Occupancy, Self::Report) => (0x22, 0x2e),
            (Occupancy, Self::Deadtime) => (0x23, 0x2c),
            (Light, Self::Report) => (0x30, 0x3e),
            (Light, Self::Hysteresis) => (0x31, 0x3f),
            (Light, Self::Deadtime) => (0x32, 0x3d),
            (Light, Self::HysteresisMinimum) => (0x33, 0x3c),
            _ => {
                return Err(Error::Parameter {
                    kind,
                    parameter: self,
                })
            }
        })
    }
    pub fn query(
        self,
        kind: InstanceType,
        address: Address,
        instance: Instance,
    ) -> Result<u32, Error> {
        Ok(address.command(instance.raw(), self.opcodes(kind)?.1))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value {
    parameter: Parameter,
    raw: u8,
    opcode: u8,
}
impl Value {
    pub fn new(kind: InstanceType, parameter: Parameter, raw: u8) -> Result<Self, Error> {
        let opcode = parameter.opcodes(kind)?.0;
        let valid = match parameter {
            Parameter::Hold => raw <= 254,
            Parameter::Hysteresis => raw <= 25,
            _ => true,
        };
        if !valid {
            return Err(Error::Range { parameter, raw });
        }
        Ok(Self {
            parameter,
            raw,
            opcode,
        })
    }
    pub fn parameter(self) -> Parameter {
        self.parameter
    }
    pub fn raw(self) -> u8 {
        self.raw
    }
    /// Timer settings only. Report/deadtime0 disable; hold0 means one second.
    /// Hysteresis has sensor/percentage units, never a duration or calibrated lux.
    pub fn duration(self) -> Option<Duration> {
        let millis = match self.parameter {
            Parameter::Report => u64::from(self.raw) * 1000,
            Parameter::Deadtime => u64::from(self.raw) * 50,
            Parameter::Hold => {
                if self.raw == 0 {
                    1000
                } else {
                    u64::from(self.raw) * 10_000
                }
            }
            Parameter::Hysteresis | Parameter::HysteresisMinimum => return None,
        };
        Some(Duration::from_millis(millis))
    }
    pub fn plan(self, address: Address, instance: Instance) -> [Step; 2] {
        [
            Step {
                frame: 0xc13000 | u32::from(self.raw),
                repeat: false,
            },
            Step {
                frame: address.command(instance.raw(), self.opcode),
                repeat: true,
            },
        ]
    }
}
#[cfg(test)]
mod tests;
