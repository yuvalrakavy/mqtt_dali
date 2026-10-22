//! Typed DT8 evidence from one owned output operation.
use super::{frame, Route, Stamp};
use crate::{
    atx::{
        serial::Serial,
        transactions::{BusReply, TransactionError, Transactions},
    },
    dali::{
        colour::{
            self as codec, AssignedColour, ByteValue, ChannelIndex, ColourType, Features, Intent,
            Query, Reply, Status, Step, ValueQuery, Width, WordValue,
        },
        gear,
    },
};
use std::collections::BTreeMap;
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Current,
    Partial,
    Unsupported,
    Unavailable,
    Stale,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Byte(ByteValue),
    Word(WordValue),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub dt8: Option<bool>,
    pub device_types: Vec<Reply<u8>>,
    pub type_prefix: Vec<u8>,
    pub gear_features: Option<Reply<u8>>,
    pub features: Option<Reply<Features>>,
    pub status: Option<Reply<Status>>,
    pub control: Option<Reply<u8>>,
    pub assignments: Vec<Reply<AssignedColour>>,
    pub quality: Quality,
    pub reason: Option<String>,
    pub values: BTreeMap<ValueQuery, Reply<Value>>,
    pub raw_values: BTreeMap<ValueQuery, Vec<Reply<u8>>>,
}
impl Default for Observation {
    fn default() -> Self {
        Self {
            dt8: None,
            device_types: vec![],
            type_prefix: vec![],
            gear_features: None,
            features: None,
            status: None,
            control: None,
            assignments: vec![],
            quality: Quality::Unavailable,
            reason: None,
            values: BTreeMap::new(),
            raw_values: BTreeMap::new(),
        }
    }
}
const CHANNELS: [ValueQuery; 6] = [
    ValueQuery::Red,
    ValueQuery::Green,
    ValueQuery::Blue,
    ValueQuery::White,
    ValueQuery::Amber,
    ValueQuery::Free,
];
impl Observation {
    pub fn active(&self) -> Option<ColourType> {
        if matches!(
            self.quality,
            Quality::Stale | Quality::Unavailable | Quality::Unsupported
        ) {
            return None;
        }
        self.reported_active()
    }
    fn reported_active(&self) -> Option<ColourType> {
        match self.status {
            Some(Reply::Value(status)) => status.active(),
            _ => None,
        }
    }
    pub fn value(&self, query: ValueQuery) -> Option<Value> {
        if self.quality != Quality::Current {
            return None;
        }
        match self.values.get(&query) {
            Some(Reply::Value(value)) => Some(*value),
            _ => None,
        }
    }
    fn feature_value(&self) -> Option<Features> {
        match self.features {
            Some(Reply::Value(features)) if features.counts_valid() => Some(features),
            _ => None,
        }
    }
    fn supported(&self, kind: ColourType) -> bool {
        self.feature_value().is_some_and(|f| match kind {
            ColourType::Temperature => f.temperature(),
            ColourType::Xy => f.xy(),
            ColourType::Rgbwaf => f.rgbwaf_channels() > 0,
            ColourType::PrimaryN => false,
        })
    }
    fn required(&self, kind: ColourType) -> Vec<ValueQuery> {
        match kind {
            ColourType::Temperature => vec![ValueQuery::Temperature],
            ColourType::Xy => vec![ValueQuery::X, ValueQuery::Y],
            ColourType::Rgbwaf => self
                .assignments
                .iter()
                .filter_map(|r| match r {
                    Reply::Value(a) => a.slot().map(|slot| CHANNELS[slot]),
                    _ => None,
                })
                .collect(),
            ColourType::PrimaryN => vec![],
        }
    }
    fn usable(&self, query: ValueQuery) -> bool {
        matches!(
            self.values.get(&query),
            Some(Reply::Value(
                Value::Byte(ByteValue::Known(_)) | Value::Word(WordValue::Known(_))
            ))
        ) && !(query == ValueQuery::Temperature
            && self.values.get(&query) == Some(&Reply::Value(Value::Word(WordValue::Known(0)))))
    }
    fn update_quality(&mut self) {
        let current = self.reported_active().is_some_and(|kind| {
            let required = self.required(kind);
            self.supported(kind)
                && !required.is_empty()
                && required.iter().all(|q| self.usable(*q))
                && (kind != ColourType::Rgbwaf
                    || (self.assignments.len()
                        == self
                            .feature_value()
                            .map_or(0, |f| usize::from(f.rgbwaf_channels()))
                        && self
                            .assignments
                            .iter()
                            .all(|r| matches!(r, Reply::Value(a) if a.slot().is_some()))))
        });
        self.quality = if current {
            Quality::Current
        } else {
            Quality::Partial
        };
        self.reason =
            (!current).then(|| "active colour evidence is incomplete or unsupported".into());
    }
    pub(super) fn preflight(&self, intent: Intent) -> Result<(), String> {
        if self.dt8 != Some(true) || !self.supported(intent.kind()) {
            return Err("requested colour representation is unsupported or unknown".into());
        }
        if !matches!(self.gear_features, Some(Reply::Value(_))) {
            return Err("colour writes require confirmed gear features".into());
        }
        if let Some(channels) = intent.channels() {
            if !matches!(self.control, Some(Reply::Value(raw)) if raw >> 6 == 2) {
                return Err("RGBWAF control is not confirmed normalized mode".into());
            }
            for (slot, value) in channels.iter().enumerate() {
                if value.is_some()
                    && !self
                        .assignments
                        .iter()
                        .any(|r| matches!(r, Reply::Value(a) if a.slot() == Some(slot)))
                {
                    return Err(
                        "requested RGBWAF colour has no confirmed physical channel assignment"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
    pub(super) fn confirms(&self, intent: Intent) -> bool {
        self.quality == Quality::Current
            && self.active() == Some(intent.kind())
            && intent.channels().is_none_or(|channels| {
                channels
                    .iter()
                    .enumerate()
                    .all(|(slot, v)| v.is_none() || self.usable(CHANNELS[slot]))
            })
    }
}

/// All steps run inside the existing owned output helper and its deadline.
async fn send(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    value: u16,
) -> Result<(), TransactionError> {
    match frame(link, route.bus, value, deadline, stamp).await? {
        BusReply::NoResponse => Ok(()),
        _ => Err(TransactionError::StreamLoss),
    }
}
async fn plan(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    steps: Vec<Step>,
) -> Result<Vec<BusReply>, TransactionError> {
    let mut replies = vec![];
    for step in steps {
        match step {
            Step::Send(value) => send(link, route, stamp, deadline, value).await?,
            Step::Query(value) => {
                let reply = frame(link, route.bus, value, deadline, stamp).await?;
                replies.push(reply.clone());
                if !matches!(reply, BusReply::Value(_)) {
                    break;
                }
            }
        }
    }
    Ok(replies)
}
async fn query(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    query: Query,
) -> Result<Reply<u8>, TransactionError> {
    Ok(codec::byte_reply(
        plan(link, route, stamp, deadline, query.plan(route.address))
            .await?
            .remove(0),
    ))
}
pub(in crate::service) async fn discover(
    sample: &mut Observation,
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
) -> Result<(), TransactionError> {
    let first = codec::byte_reply(
        frame(
            link,
            route.bus,
            route.address.query(gear::Query::DeviceType),
            deadline,
            stamp,
        )
        .await?,
    );
    sample.device_types.push(first);
    sample.dt8 = match first {
        Reply::Value(8) => Some(true),
        Reply::Value(255) => {
            let mut previous = None;
            let mut found = None;
            for _ in 0..9 {
                let next = codec::byte_reply(
                    frame(
                        link,
                        route.bus,
                        route.address.query(gear::Query::NextDeviceType),
                        deadline,
                        stamp,
                    )
                    .await?,
                );
                sample.device_types.push(next);
                match next {
                    Reply::Value(value) if value != 255 && previous.is_none_or(|p| value > p) => {
                        if value == 8 {
                            sample.type_prefix.push(value);
                            found = Some(true);
                            break;
                        }
                        if value > 8 {
                            found = Some(false);
                            break;
                        }
                        sample.type_prefix.push(value);
                        previous = Some(value);
                    }
                    _ => break,
                }
            }
            found
        }
        Reply::Value(_) => Some(false),
        _ => None,
    };
    if sample.dt8 != Some(true) {
        sample.quality = if sample.dt8 == Some(false) {
            Quality::Unsupported
        } else {
            Quality::Unavailable
        };
        sample.reason = Some("DT8 presence is absent or unconfirmed".into());
        return Ok(());
    }
    sample.gear_features = Some(query(link, route, stamp, deadline, Query::GearFeatures).await?);
    sample.features = Some(
        query(link, route, stamp, deadline, Query::TypeFeatures)
            .await?
            .map(Features::decode),
    );
    if let Some(features) = sample.feature_value() {
        if features.rgbwaf_channels() > 0 {
            sample.control = Some(query(link, route, stamp, deadline, Query::RgbwafControl).await?);
            for index in 0..features.rgbwaf_channels() {
                let replies = plan(
                    link,
                    route,
                    stamp,
                    deadline,
                    ChannelIndex::new(index)
                        .expect("validated feature count")
                        .query(route.address)
                        .to_vec(),
                )
                .await?;
                sample
                    .assignments
                    .push(codec::byte_reply(replies[0].clone()).map(AssignedColour::decode));
            }
        }
    }
    read_values(sample, link, route, stamp, deadline).await
}
pub(super) async fn read_values(
    sample: &mut Observation,
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
) -> Result<(), TransactionError> {
    sample.values.clear();
    sample.raw_values.clear();
    // Mark incomplete before awaiting, so a transport failure cannot retain a
    // pre-write Current classification over a partially replaced sample.
    sample.quality = Quality::Partial;
    sample.status = Some(
        query(link, route, stamp, deadline, Query::Status)
            .await?
            .map(Status::decode),
    );
    let mut required = sample
        .reported_active()
        .filter(|kind| sample.supported(*kind))
        .map_or_else(Vec::new, |kind| sample.required(kind));
    if sample.feature_value().is_some_and(Features::temperature) {
        required.extend([
            ValueQuery::Coolest,
            ValueQuery::PhysicalCoolest,
            ValueQuery::Warmest,
            ValueQuery::PhysicalWarmest,
        ]);
    }
    for value in required {
        let replies = plan(link, route, stamp, deadline, value.plan(route.address)).await?;
        let raw: Vec<_> = replies.iter().cloned().map(codec::byte_reply).collect();
        let decoded = match value.width() {
            Width::Byte => codec::byte_reply(replies.last().unwrap().clone())
                .map(|v| Value::Byte(ByteValue::decode(v))),
            Width::Word if replies.len() == 3 => {
                codec::word_reply(replies[1].clone(), replies[2].clone()).map(Value::Word)
            }
            Width::Word => codec::byte_reply(replies.last().unwrap().clone())
                .map(|_| unreachable!("incomplete plan ends on failed query")),
        };
        sample.raw_values.insert(value, raw);
        sample.values.insert(value, decoded);
    }
    sample.update_quality();
    Ok(())
}
pub(super) async fn apply(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: Stamp,
    deadline: Instant,
    intent: Intent,
) -> Result<(), TransactionError> {
    for value in intent.frames(gear::Destination::Short(route.address)) {
        send(link, route, stamp, deadline, value).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
