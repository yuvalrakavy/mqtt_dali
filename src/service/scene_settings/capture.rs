//! Capture-only control level, kept separate from public actual output State.
use super::*;
use crate::dali::gear::{self, LevelValue};

/// Interpret already correlated type replies; DT8 absence alone cannot prove
/// DT7 absence. A decisive ordered prefix need not enumerate subsequent types.
fn switching(types: &[Reply<u8>]) -> Option<bool> {
    let (first, following) = types.split_first()?;
    match first {
        Reply::Value(7) => Some(true),
        Reply::Value(255) => {
            let mut previous = None;
            for (n, next) in following.iter().take(9).enumerate() {
                match next {
                    Reply::Value(254) => return (n >= 2).then_some(false),
                    Reply::Value(v) if *v < 254 && previous.is_none_or(|p| *v > p) => {
                        if *v >= 7 {
                            return Some(*v == 7);
                        }
                        previous = Some(*v);
                    }
                    _ => return None,
                }
            }
            None
        }
        Reply::Value(_) => Some(false),
        _ => None,
    }
}

pub(super) async fn level(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    actual: &output::Observation,
) -> Result<Option<gear::Level>, TransactionError> {
    if actual.quality != output::Quality::Current {
        return Ok(None);
    }
    let Some(caps) = &actual.colour else {
        return Ok(None);
    };
    match switching(&caps.device_types) {
        Some(false) => Ok(match actual.level() {
            Some(LevelValue::Known(level)) => Some(level),
            _ => None,
        }),
        Some(true) => {
            for step in codec::capture_level(route.address) {
                match step {
                    codec::Step::Query(frame) => {
                        return Ok(
                            match output::frame(link, route.bus, frame, deadline, stamp).await? {
                                BusReply::Value(raw) => gear::Level::new(raw).ok(),
                                _ => None,
                            },
                        );
                    }
                    command => super::execute::step(link, route, stamp, deadline, command).await?,
                }
            }
            unreachable!("capture plan ends with its DTR0 query")
        }
        None => Ok(None),
    }
}
