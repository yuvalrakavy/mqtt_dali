//! Part103 feature query frames and bounded decoding of captured byte replies.
//! This performs no I/O. Silence/collision is not an empty feature list. The
//! owner must bound collection and keep each adjacent same-scope enumeration
//! under one bus permit, with separate presence/type/authority proofs.
use super::{Address, Instance};

pub const MAX_FEATURES: usize = 65;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Device,
    Instance(Instance),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Type,
    Next,
}
impl Scope {
    pub fn query(self, address: Address, query: Query) -> u32 {
        address.command(
            match self {
                Self::Device => 0xfe,
                Self::Instance(i) => i.raw(),
            },
            match query {
                Query::Type => 0x8e,
                Query::Next => 0x8f,
            },
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeatureType(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("feature type {0} is outside32..96")]
    Type(u8),
    #[error("invalid or incomplete feature enumeration: {0}")]
    Sequence(&'static str),
}
impl FeatureType {
    pub const FEEDBACK: Self = Self(32);
    pub fn new(raw: u8) -> Result<Self, Error> {
        if !(32..=96).contains(&raw) {
            return Err(Error::Type(raw));
        }
        Ok(Self(raw))
    }
    pub fn raw(self) -> u8 {
        self.0
    }
}
/// Decode a captured QUERY FEATURE TYPE byte and its immediate NEXT replies.
/// FE means none/end; FF means multiple. Never collect beyond65 features plus
/// the terminator, or interleave queries to another address/selector.
pub fn decode(first: u8, continuation: &[u8]) -> Result<Vec<FeatureType>, Error> {
    if first != 255 {
        if !continuation.is_empty() {
            return Err(Error::Sequence("unexpected continuation"));
        }
        return if first == 254 {
            Ok(Vec::new())
        } else {
            Ok(vec![FeatureType::new(first)?])
        };
    }
    if !(3..=MAX_FEATURES + 1).contains(&continuation.len()) || continuation.last() != Some(&254) {
        return Err(Error::Sequence(
            "multiple features need a bounded terminated list",
        ));
    }
    let values = continuation[..continuation.len() - 1]
        .iter()
        .copied()
        .map(FeatureType::new)
        .collect::<Result<Vec<_>, _>>()?;
    if values.windows(2).any(|v| v[0] >= v[1]) {
        return Err(Error::Sequence("features must be strictly ascending"));
    }
    Ok(values)
}
#[cfg(test)]
mod tests;
