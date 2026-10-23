//! Basic Part332 feature-addressed feedback. Single ordinary24-bit frames,
//! not repeated input settings and not a DT8 enable sequence. Callers must
//! separately prove feature/capability and read actual state after a write.
use super::{features::Scope, Address};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Activate,
    Stop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Capability,
    Active,
}
fn frame(address: Address, scope: Scope, opcode: u8) -> u32 {
    let selector = match scope {
        Scope::Device => 0xfc,
        Scope::Instance(i) => 0x20 | i.raw(),
    };
    address.command(selector, opcode)
}
impl Action {
    pub fn frame(self, address: Address, scope: Scope) -> u32 {
        frame(
            address,
            scope,
            match self {
                Self::Activate => 0x10,
                Self::Stop => 0x11,
            },
        )
    }
}
impl Query {
    pub fn frame(self, address: Address, scope: Scope) -> u32 {
        frame(
            address,
            scope,
            match self {
                Self::Capability => 0x4f,
                Self::Active => 0x4e,
            },
        )
    }
}
/// Preserve unfamiliar bits without inventing their meaning or using them as
/// mutation authority. Flags below are the six base IEC2017 capability bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities(u8);
impl Capabilities {
    pub fn from_raw(raw: u8) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u8 {
        self.0
    }
    pub fn visible(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn brightness(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn colour(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn audible(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn volume(self) -> bool {
        self.0 & 16 != 0
    }
    pub fn pitch(self) -> bool {
        self.0 & 32 != 0
    }
    pub fn unknown_bits(self) -> u8 {
        self.0 & 0xc0
    }
}
