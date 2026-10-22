//! Normal short-address writes. Hold each DTR/write plan under one bus permit.
//! None as destination means broadcast; None as new address means MASK (clear).
use super::{gear, input};

/// DTR0 followed by SetShortAddress. Send the second frame twice.
pub fn output(
    destination: Option<gear::ShortAddress>,
    address: Option<gear::ShortAddress>,
) -> [u16; 2] {
    [
        0xa300 | u16::from(address.map_or(255, |a| (a.raw() << 1) | 1)),
        destination
            .map_or(gear::Destination::Broadcast, gear::Destination::Short)
            .command(0x80),
    ]
}
/// Part 103 uses a raw 0..63 address in DTR0. Send the second frame twice.
pub fn input(destination: Option<input::Address>, address: Option<input::Address>) -> [u32; 2] {
    [
        0xc13000 | u32::from(address.map_or(255, input::Address::raw)),
        (u32::from(destination.map_or(255, |a| (a.raw() << 1) | 1)) << 16) | 0xfe14,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_clear_and_short_address_boundaries() {
        assert_eq!(output(None, None), [0xa3ff, 0xff80]);
        assert_eq!(input(None, None), [0xc130ff, 0xfffe14]);
        assert_eq!(
            output(Some(gear::ShortAddress::new(0).unwrap()), None),
            [0xa3ff, 0x0180]
        );
        assert_eq!(
            output(Some(gear::ShortAddress::new(63).unwrap()), None),
            [0xa3ff, 0x7f80]
        );
        assert_eq!(
            input(Some(input::Address::new(0).unwrap()), None),
            [0xc130ff, 0x01fe14]
        );
        assert_eq!(
            input(Some(input::Address::new(63).unwrap()), None),
            [0xc130ff, 0x7ffe14]
        );
        assert_eq!(
            output(
                Some(gear::ShortAddress::new(0).unwrap()),
                Some(gear::ShortAddress::new(63).unwrap())
            ),
            [0xa37f, 0x0180]
        );
        assert_eq!(
            output(
                Some(gear::ShortAddress::new(63).unwrap()),
                Some(gear::ShortAddress::new(0).unwrap())
            ),
            [0xa301, 0x7f80]
        );
        assert_eq!(
            input(
                Some(input::Address::new(0).unwrap()),
                Some(input::Address::new(63).unwrap())
            ),
            [0xc1303f, 0x01fe14]
        );
        assert_eq!(
            input(
                Some(input::Address::new(63).unwrap()),
                Some(input::Address::new(0).unwrap())
            ),
            [0xc13000, 0x7ffe14]
        );
        assert!(gear::ShortAddress::new(64).is_err());
        assert!(input::Address::new(64).is_err());
    }
}
