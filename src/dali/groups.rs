//! Pure Part 102 group membership frames. FF is a valid membership byte.
use super::gear::{Destination, GroupAddress, ShortAddress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mask(pub u16);
impl Mask {
    pub fn contains(self, group: GroupAddress) -> bool {
        self.0 & (1 << group.raw()) != 0
    }
    pub fn with(self, group: GroupAddress, member: bool) -> Self {
        let bit = 1 << group.raw();
        Self(if member { self.0 | bit } else { self.0 & !bit })
    }
    /// At most sixteen settings, removals before additions.
    pub fn changes(self, desired: Self) -> Vec<(GroupAddress, bool)> {
        [false, true]
            .into_iter()
            .flat_map(|member| {
                (0..16).filter_map(move |slot| {
                    let group = GroupAddress::new(slot).unwrap();
                    (self.contains(group) != desired.contains(group)
                        && desired.contains(group) == member)
                        .then_some((group, member))
                })
            })
            .collect()
    }
}
pub fn query(address: ShortAddress, high: bool) -> u16 {
    Destination::Short(address).command(if high { 0xc1 } else { 0xc0 })
}
/// This setting must be sent twice by the transaction transport.
pub fn setting(address: ShortAddress, group: GroupAddress, member: bool) -> u16 {
    Destination::Short(address).command((if member { 0x60 } else { 0x70 }) + group.raw())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_boundaries_and_mask_differences() {
        let a = ShortAddress::new(0).unwrap();
        let z = ShortAddress::new(63).unwrap();
        let first = GroupAddress::new(0).unwrap();
        let last = GroupAddress::new(15).unwrap();
        assert_eq!((query(a, false), query(z, true)), (0x01c0, 0x7fc1));
        assert_eq!(
            (setting(a, first, true), setting(z, last, false)),
            (0x0160, 0x7f7f)
        );
        assert!(ShortAddress::new(64).is_err());
        assert!(GroupAddress::new(16).is_err());
        assert_eq!(Mask(0xffff).with(last, false), Mask(0x7fff));
        let changes = Mask(0x8000).changes(Mask(1));
        assert_eq!(changes, vec![(last, false), (first, true)]);
        assert_eq!(Mask(0).changes(Mask(0xffff)).len(), 16);
        assert!(Mask(0xffff).changes(Mask(0xffff)).is_empty());
    }
}
