//! Independent read-only memory profile. No bridge codec imports.
use serde::{Deserialize, Serialize};
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
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        let mut banks = std::collections::HashSet::new();
        if self.banks.len() > 16
            || self.banks.iter().any(|bank| {
                !banks.insert(bank.bank) || bank.bytes.is_empty() || bank.bytes.len() > 255
            })
        {
            return Err(
                "memory profile needs at most 16 unique banks of 1..255 nullable bytes".into(),
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
