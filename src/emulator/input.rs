//! Independent physical value/latch model; no bridge input codec dependencies.
use super::scenario::Input;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(super) struct State {
    #[serde(flatten)]
    pub config: Input,
    #[serde(skip)]
    latches: BTreeMap<u8, (Vec<u8>, usize)>,
}
impl State {
    pub fn new(config: Input) -> Self {
        Self {
            config,
            latches: BTreeMap::new(),
        }
    }
    pub fn query(&mut self, instance: u8, opcode: u8) -> Option<u8> {
        if instance == 0xfe {
            return match opcode {
                0x30 => Some(0),
                0x35 => Some(self.config.instances.len() as u8),
                _ => None,
            };
        }
        let kind = *self.config.instances.get(usize::from(instance))?;
        if opcode == 0x80 {
            return Some(kind);
        }
        let value = self.config.values.iter().find(|v| v.instance == instance)?;
        match opcode {
            0x81 => Some(value.resolution),
            0x8c => {
                let bytes = value.bytes.clone();
                let first = bytes.first().copied();
                self.latches.insert(instance, (bytes, 1));
                first
            }
            0x8d => {
                let (bytes, cursor) = self.latches.get_mut(&instance)?;
                let byte = bytes.get(*cursor).copied();
                if byte.is_some() {
                    *cursor += 1;
                }
                byte
            }
            _ => None,
        }
    }
    pub fn set_value(&mut self, instance: u8, bytes: Vec<u8>) -> Result<(), String> {
        let value = self
            .config
            .values
            .iter_mut()
            .find(|v| v.instance == instance)
            .ok_or("input value profile does not exist")?;
        if bytes.len() != usize::from(value.resolution).div_ceil(8) {
            return Err("input value byte count does not match resolution".into());
        }
        value.bytes = bytes;
        Ok(())
    }
}
