//! Part 103 random-address commands; short-address parameters are raw 0..63.
use super::Address;
use crate::dali::commissioning::SearchAddress;

pub const TERMINATE: u32 = 0xc10000;
pub const INITIALISE_UNADDRESSED: u32 = 0xc1017f;
pub const RANDOMISE: u32 = 0xc10200;
pub const COMPARE: u32 = 0xc10300;
pub const WITHDRAW: u32 = 0xc10400;
pub const QUERY_SHORT_ADDRESS: u32 = 0xc10a00;

pub fn search(address: SearchAddress) -> [u32; 3] {
    let raw = address.raw();
    [
        0xc10500 | ((raw >> 16) & 255),
        0xc10600 | ((raw >> 8) & 255),
        0xc10700 | (raw & 255),
    ]
}
pub fn program(address: Address) -> u32 {
    0xc10800 | u32::from(address.raw())
}
pub fn verify(address: Address) -> u32 {
    0xc10900 | u32::from(address.raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_commissioning_literal_boundaries_use_raw_short_addresses() {
        assert_eq!(
            [
                TERMINATE,
                INITIALISE_UNADDRESSED,
                RANDOMISE,
                COMPARE,
                WITHDRAW,
                QUERY_SHORT_ADDRESS
            ],
            [0xc10000, 0xc1017f, 0xc10200, 0xc10300, 0xc10400, 0xc10a00]
        );
        assert_eq!(
            (
                program(Address::new(0).unwrap()),
                verify(Address::new(63).unwrap())
            ),
            (0xc10800, 0xc1093f)
        );
        assert_eq!(
            search(SearchAddress::new(0).unwrap()),
            [0xc10500, 0xc10600, 0xc10700]
        );
        assert_eq!(
            search(SearchAddress::new(0xffffff).unwrap()),
            [0xc105ff, 0xc106ff, 0xc107ff]
        );
        assert!(Address::new(64).is_err());
        assert!(SearchAddress::new(0x1000000).is_none());
    }
}
