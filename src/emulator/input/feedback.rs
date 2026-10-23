//! Independent, opt-in visible on/off profile. No production codec imports.
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct State {
    pub instance: Option<u8>,
    capability: u8,
    active: bool,
}
impl State {
    pub fn new(instance: Option<u8>) -> Self {
        Self {
            instance,
            capability: 1,
            active: false,
        }
    }
    /// Activate/Stop need one ordinary24-bit send. Inactive has no backward byte;
    /// callers need independent current presence and capability evidence.
    pub fn command(&mut self, opcode: u8) -> Option<u8> {
        match opcode {
            0x10 => self.active = true,
            0x11 => self.active = false,
            0x4f => return Some(self.capability),
            0x4e => return self.active.then_some(255),
            _ => {}
        }
        None
    }
}
