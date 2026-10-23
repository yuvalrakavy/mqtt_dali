//! Aggregate captured individual-address evidence for a hardware group.
//! The roster is the frozen saved inventory, not a census of every physical gear.
use crate::atx::transactions::BusReply;
use crate::dali::gear::{GroupAddress, Level, ShortAddress};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub address: ShortAddress,
    pub membership_low: Option<BusReply>,
    pub membership_high: Option<BusReply>,
    pub actual_level: Option<BusReply>,
    pub status: Option<BusReply>,
}
impl Sample {
    pub fn unvisited(address: ShortAddress) -> Self {
        Self {
            address,
            membership_low: None,
            membership_high: None,
            actual_level: None,
            status: None,
        }
    }
    pub fn membership(&self) -> Option<u16> {
        match (&self.membership_low, &self.membership_high) {
            (Some(BusReply::Value(low)), Some(BusReply::Value(high))) => {
                Some(u16::from_le_bytes([*low, *high]))
            }
            _ => None,
        }
    }
    fn level(&self) -> Option<Level> {
        match (&self.actual_level, &self.status) {
            (Some(BusReply::Value(level)), Some(BusReply::Value(_))) => Level::new(*level).ok(),
            _ => None,
        }
    }
    fn answered(&self) -> bool {
        [
            &self.membership_low,
            &self.membership_high,
            &self.actual_level,
            &self.status,
        ]
        .into_iter()
        .any(|v| matches!(v, Some(BusReply::Value(_))))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Uniform,
    Mixed,
    Empty,
    Partial,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub quality: Quality,
    pub power: Option<bool>,
    pub level: Option<Level>,
    pub power_mixed: Option<bool>,
    pub level_mixed: Option<bool>,
    /// A lower bound when membership evidence is incomplete.
    pub known_members: usize,
    pub unresolved: Vec<ShortAddress>,
}
impl Summary {
    pub fn brightness(&self) -> Option<f64> {
        self.level.map(Level::normalized)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("group roster or sample list exceeds 64 saved addresses")]
    TooMany,
    #[error("duplicate saved output address {0}")]
    DuplicateRoster(u8),
    #[error("duplicate group sample address {0}")]
    DuplicateSample(u8),
    #[error("sample address {0} is outside the frozen group roster")]
    UnexpectedSample(u8),
}
/// Missing samples remain unresolved, even when every sampled prefix agrees.
pub fn summarize(
    group: GroupAddress,
    roster: &[ShortAddress],
    samples: &[Sample],
) -> Result<Summary, Error> {
    if roster.len() > 64 || samples.len() > 64 {
        return Err(Error::TooMany);
    }
    let mut included = [false; 64];
    for address in roster {
        let seen = &mut included[usize::from(address.raw())];
        if *seen {
            return Err(Error::DuplicateRoster(address.raw()));
        }
        *seen = true;
    }
    let mut captured: [Option<&Sample>; 64] = [None; 64];
    for sample in samples {
        let index = usize::from(sample.address.raw());
        if !included[index] {
            return Err(Error::UnexpectedSample(sample.address.raw()));
        }
        if captured[index].replace(sample).is_some() {
            return Err(Error::DuplicateSample(sample.address.raw()));
        }
    }
    let mut result = Summary {
        quality: Quality::Unavailable,
        power: None,
        level: None,
        power_mixed: None,
        level_mixed: None,
        known_members: 0,
        unresolved: Vec::new(),
    };
    let mut levels = Vec::new();
    let mut answered = false;
    for address in roster {
        let Some(sample) = captured[usize::from(address.raw())] else {
            result.unresolved.push(*address);
            continue;
        };
        answered |= sample.answered();
        let Some(mask) = sample.membership() else {
            result.unresolved.push(*address);
            continue;
        };
        if mask & (1 << group.raw()) == 0 {
            continue;
        }
        result.known_members += 1;
        if let Some(level) = sample.level() {
            levels.push(level);
        } else {
            result.unresolved.push(*address);
        }
    }
    if !result.unresolved.is_empty() {
        result.quality = if answered {
            Quality::Partial
        } else {
            Quality::Unavailable
        };
        return Ok(result);
    }
    let Some(first) = levels.first().copied() else {
        result.quality = Quality::Empty;
        return Ok(result);
    };
    let power = first.raw() > 0;
    let power_mixed = levels.iter().any(|level| (level.raw() > 0) != power);
    let level_mixed = levels.iter().any(|level| *level != first);
    result.quality = if level_mixed {
        Quality::Mixed
    } else {
        Quality::Uniform
    };
    result.power = (!power_mixed).then_some(power);
    result.level = (!level_mixed).then_some(first);
    result.power_mixed = Some(power_mixed);
    result.level_mixed = Some(level_mixed);
    Ok(result)
}

#[cfg(test)]
mod tests;
