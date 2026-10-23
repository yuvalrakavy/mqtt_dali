//! Independent memory profile and bounded snapshot latch. No bridge codec imports.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(default = "version")]
    pub version: u8,
    pub banks: Vec<Bank>,
}
fn version() -> u8 {
    8
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bank {
    pub bank: u8,
    pub bytes: Vec<Option<u8>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub latch: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        let mut banks = std::collections::HashSet::new();
        if self.banks.len() > 16
            || self.banks.iter().any(|bank| {
                !banks.insert(bank.bank)
                    || bank.bytes.is_empty()
                    || bank.bytes.len() > 255
                    || bank.latch && (!(202..=204).contains(&bank.bank) || bank.bytes.len() > 16)
            })
        {
            return Err(
                "memory profile needs at most16 unique banks of1..255 nullable bytes; latching requires output banks202..204 with at most16 bytes".into(),
            );
        }
        Ok(())
    }
    pub fn read(&self, bank: u8, offset: &mut u8) -> Option<u8> {
        let bank = self.banks.iter().find(|entry| entry.bank == bank)?;
        let byte = bank.bytes.get(usize::from(*offset)).copied().flatten();
        *offset = offset.saturating_add(1);
        byte
    }
}

/// Scratch state, separate from installation data and scenario capture.
#[derive(Default)]
pub struct State {
    write_enabled: bool,
    frozen: BTreeMap<u8, Vec<Option<u8>>>,
}
impl State {
    pub fn observe(&mut self, frame: u16) {
        let [address, opcode] = frame.to_be_bytes();
        let addressed = (address <= 0x9f || address >= 0xfc) && address & 1 == 1;
        let related = matches!(address, 0xa3 | 0xc3 | 0xc5 | 0xc7 | 0xc9)
            || addressed && matches!(opcode, 0x98 | 0x9c | 0x9d);
        if !related {
            self.write_enabled = false;
        }
    }
    pub fn enable(&mut self) {
        self.write_enabled = true;
    }
    pub fn write(&mut self, profile: &Profile, bank: u8, offset: &mut u8, value: u8) {
        if !self.write_enabled || *offset != 2 {
            return;
        }
        let Some(entry) = profile
            .banks
            .iter()
            .find(|entry| entry.bank == bank && entry.latch)
        else {
            return;
        };
        match value {
            0xaa => {
                self.frozen.insert(bank, entry.bytes.clone());
            }
            0xff => {
                self.frozen.remove(&bank);
            }
            _ => return,
        }
        *offset = 3;
    }
    pub fn read(&self, profile: &Profile, bank: u8, offset: &mut u8) -> Option<u8> {
        let entry = profile.banks.iter().find(|entry| entry.bank == bank)?;
        if !entry.latch {
            return profile.read(bank, offset);
        }
        let frozen = self.frozen.get(&bank);
        let byte = if *offset == 2 {
            Some(if frozen.is_some() { 0xaa } else { 0xff })
        } else {
            frozen
                .unwrap_or(&entry.bytes)
                .get(usize::from(*offset))
                .copied()
                .flatten()
        };
        *offset = (*offset).saturating_add(1);
        byte
    }
}
