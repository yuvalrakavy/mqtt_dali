//! Part 102 special frames. No transport, identity or persistence effects here.
use super::gear::ShortAddress;

pub const TERMINATE: u16 = 0xa100;
pub const INITIALISE_UNADDRESSED: u16 = 0xa5ff;
pub const RANDOMISE: u16 = 0xa700;
pub const COMPARE: u16 = 0xa900;
pub const WITHDRAW: u16 = 0xab00;
pub const QUERY_SHORT_ADDRESS: u16 = 0xbb00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchAddress(u32);
impl SearchAddress {
    pub fn new(value: u32) -> Option<Self> {
        (value <= 0xffffff).then_some(Self(value))
    }
    pub fn raw(self) -> u32 {
        self.0
    }
    pub fn frames(self) -> [u16; 3] {
        [
            0xb100 | ((self.0 >> 16) as u16 & 255),
            0xb300 | ((self.0 >> 8) as u16 & 255),
            0xb500 | (self.0 as u16 & 255),
        ]
    }
}
pub fn program(address: ShortAddress) -> u16 {
    0xb700 | u16::from((address.raw() << 1) | 1)
}
pub fn verify(address: ShortAddress) -> u16 {
    0xb900 | u16::from((address.raw() << 1) | 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commissioning_special_frames_are_literal_and_width_checked() {
        assert_eq!(
            [
                TERMINATE,
                INITIALISE_UNADDRESSED,
                RANDOMISE,
                COMPARE,
                WITHDRAW,
                QUERY_SHORT_ADDRESS
            ],
            [0xa100, 0xa5ff, 0xa700, 0xa900, 0xab00, 0xbb00]
        );
        for (value, expected) in [
            (0, [0xb100, 0xb300, 0xb500]),
            (0x123456, [0xb112, 0xb334, 0xb556]),
            (0xffffff, [0xb1ff, 0xb3ff, 0xb5ff]),
        ] {
            assert_eq!(SearchAddress::new(value).unwrap().frames(), expected);
        }
        assert!(SearchAddress::new(0x1000000).is_none());
        assert_eq!(program(ShortAddress::new(0).unwrap()), 0xb701);
        assert_eq!(program(ShortAddress::new(63).unwrap()), 0xb77f);
        assert_eq!(verify(ShortAddress::new(0).unwrap()), 0xb901);
        assert_eq!(verify(ShortAddress::new(63).unwrap()), 0xb97f);
    }
}
