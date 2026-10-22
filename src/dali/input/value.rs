//! Exact full-resolution Part 103 input values, independently of event data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("input resolution must be 1..255 bits")]
    Resolution,
    #[error("input value needs {expected} bytes, received {actual}")]
    Length { expected: usize, actual: usize },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution(u8);
impl Resolution {
    pub fn new(bits: u8) -> Result<Self, ValueError> {
        (bits != 0)
            .then_some(Self(bits))
            .ok_or(ValueError::Resolution)
    }
    pub fn bits(self) -> u8 {
        self.0
    }
    pub fn byte_count(self) -> usize {
        usize::from(self.0).div_ceil(8)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValue {
    resolution: Resolution,
    bytes: Vec<u8>,
}
impl InputValue {
    pub fn decode(resolution: Resolution, wire: &[u8]) -> Result<Self, ValueError> {
        let expected = resolution.byte_count();
        if wire.len() != expected {
            return Err(ValueError::Length {
                expected,
                actual: wire.len(),
            });
        }
        // Each response is most-significant first. The last byte repeats the
        // value's high bits into the unused low positions; remove that padding.
        let shift = (8 - resolution.bits() % 8) % 8;
        let mut carry = 0;
        let bytes = wire
            .iter()
            .map(|byte| {
                let result = if shift == 0 {
                    *byte
                } else {
                    (*byte >> shift) | carry
                };
                carry = if shift == 0 { 0 } else { *byte << (8 - shift) };
                result
            })
            .collect();
        Ok(Self { resolution, bytes })
    }
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn hex(&self) -> String {
        self.bytes.iter().map(|b| format!("{b:02X}")).collect()
    }
    pub fn as_u64(&self) -> Option<u64> {
        self.bytes.iter().try_fold(0u64, |value, b| {
            value.checked_mul(256)?.checked_add(u64::from(*b))
        })
    }
    /// Approximate 0..1 fraction; use bytes/hex when exact precision is required.
    pub fn normalized(&self) -> f64 {
        let numerator = self
            .bytes
            .iter()
            .fold(0.0, |value, b| value * 256.0 + f64::from(*b));
        numerator / (2f64.powi(i32::from(self.resolution.bits())) - 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_input_values_keep_literal_resolution_padding_and_all_255_bits() {
        for (bits, wire, expected) in [
            (1, vec![255], "01".to_string()),
            (1, vec![0], "00".to_string()),
            (2, vec![0x55], "01".to_string()),
            (2, vec![0xaa], "02".to_string()),
            (2, vec![255], "03".to_string()),
            (10, vec![0xab, 0xc0], "02AF".to_string()),
            (16, vec![0xab, 0xcd], "ABCD".to_string()),
            (64, vec![255; 8], "FFFFFFFFFFFFFFFF".to_string()),
            (255, vec![255; 32], format!("7F{}", "FF".repeat(31))),
        ] {
            let v = InputValue::decode(Resolution::new(bits).unwrap(), &wire).unwrap();
            assert_eq!(
                v.hex(),
                expected,
                "full input value lost significant bits or kept padding"
            );
            assert_eq!(v.bytes().len(), wire.len());
            assert!((0.0..=1.0).contains(&v.normalized()));
        }
        let huge = InputValue::decode(Resolution::new(255).unwrap(), &[255; 32]).unwrap();
        assert_eq!(huge.as_u64(), None, "wide input was truncated into u64");
        assert_eq!(huge.normalized(), 1.0);
    }
    #[test]
    fn input_value_requires_exact_complete_byte_count_and_nonzero_resolution() {
        assert!(Resolution::new(0).is_err());
        for bits in 1..=255 {
            let resolution = Resolution::new(bits).unwrap();
            let bytes = vec![0; resolution.byte_count()];
            assert!(InputValue::decode(resolution, &bytes).is_ok());
            assert!(
                InputValue::decode(resolution, &bytes[..bytes.len() - 1]).is_err(),
                "partial value became current"
            );
            let mut extra = bytes;
            extra.push(0);
            assert!(
                InputValue::decode(resolution, &extra).is_err(),
                "extra bytes were silently discarded"
            );
        }
    }
}
