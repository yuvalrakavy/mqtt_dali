//! Stored configuration evidence and momentary native recall under one UART owner.
use super::*;
use crate::{
    atx::{
        serial::Serial,
        transactions::{BusReply, Response, TransactionError, Transactions},
        wire::{Command, DaliFrame},
    },
    dali::{
        colour::{self as colour_codec, Reply, Width},
        gear::{Destination, GroupAddress, ShortAddress},
        scenes as codec,
    },
};
use serde::{Deserialize, Serialize};
pub(super) mod read;
pub(super) mod settings_value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Read {
        slot: codec::Slot,
    },
    Recall {
        group: Option<GroupAddress>,
        expected_revision: u64,
    },
}
impl Action {
    pub(super) fn writing(self) -> bool {
        matches!(self, Self::Recall { .. })
    }
    fn name(self) -> &'static str {
        if self.writing() {
            "recall"
        } else {
            "read"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reading {
    Value { raw: u16 },
    Mask { raw: u16 },
    NoResponse {},
    ReceiveCollision { code: Option<u8> },
    TransmitCollision { code: Option<u8> },
}
impl Reading {
    fn byte(reply: BusReply, masked: bool) -> Self {
        match reply {
            BusReply::Value(raw) if masked && raw == 255 => Self::Mask { raw: 255 },
            BusReply::Value(raw) => Self::Value {
                raw: u16::from(raw),
            },
            BusReply::NoResponse => Self::NoResponse {},
            BusReply::ReceiveCollision(code) => Self::ReceiveCollision { code },
            BusReply::TransmitCollision(code) => Self::TransmitCollision { code },
        }
    }
    pub(super) fn answered(&self) -> bool {
        matches!(self, Self::Value { .. } | Self::Mask { .. })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Complete,
    Partial,
    Unsupported,
    Stale,
}
impl<'de> Deserialize<'de> for Quality {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match String::deserialize(d)?.as_str() {
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "unsupported" => Ok(Self::Unsupported),
            "stale" => Ok(Self::Stale),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["complete", "partial", "unsupported", "stale"],
            )),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    pub level: Reading,
    pub dt8: Option<bool>,
    pub color_type: Option<Reading>,
    pub values: std::collections::BTreeMap<String, Reading>,
    pub quality: Quality,
    pub stale: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub revision: u64,
    pub bus: u8,
    pub slot: u8,
    pub address: Option<u8>,
    pub group: Option<u8>,
    pub stored: Option<Stored>,
    pub saved: Option<crate::config::scene_settings::Setting>,
    pub comparison: Option<settings_value::Comparison>,
}
pub(super) struct Request {
    pub target: String,
    pub bus: u8,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub address: Option<ShortAddress>,
    pub slot: codec::Slot,
    pub action: Action,
    pub context: RequestContext,
}
pub(super) struct Observation {
    pub stamp: output::Stamp,
    pub stale: bool,
    pub stored: Option<Stored>,
}
pub(super) struct Outcome {
    pub observation: Observation,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) struct Completion {
    pub request: Request,
    pub observation: Option<Observation>,
    pub error: Option<ServiceError>,
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: Action,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if let Action::Read { slot } = action {
        let r = output::resolve(snapshot, target, cause, false)?;
        return Ok(Route {
            controller: r.controller,
            bus: r.bus,
            address: Some(r.address),
            slot,
            action,
            context: r.context,
        });
    }
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    let Action::Recall {
        expected_revision, ..
    } = action
    else {
        unreachable!()
    };
    if expected_revision != snapshot.model.inventory().revision {
        return Err(ServiceError::Rejected("inventory revision changed".into()));
    }
    let p = snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .ok_or_else(|| ServiceError::Rejected("unknown scene".into()))?;
    let crate::inventory::Location::Scene { bus, slot } = p.location else {
        return Err(ServiceError::Rejected("named scene required".into()));
    };
    if !snapshot
        .model
        .inventory()
        .buses
        .iter()
        .any(|b| b.bus == bus && b.enabled)
    {
        return Err(ServiceError::Rejected("bus is disabled".into()));
    }
    let table = routing::generations(&snapshot.model);
    if cause.generation() != Some(table.targets[target].as_str()) {
        return Err(ServiceError::Rejected(
            "missing or retired target generation".into(),
        ));
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        address: None,
        slot: codec::Slot::new(slot).map_err(|e| ServiceError::Rejected(e.to_string()))?,
        action,
        context: cause.clone(),
    })
}
pub(super) fn admit(handle: &ServiceHandle, route: &Route) -> Result<(), ServiceError> {
    if route.action.writing() && identification::blocks_bus(handle, route.bus) {
        return Err(ServiceError::Rejected(
            "stop identification before recalling a scene on this bus".into(),
        ));
    }
    Ok(())
}
pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    let Request { reply, _permit, .. } = request;
    drop(_permit);
    let _ = reply.send(result);
}
pub(super) fn invalidate_actual(snapshot: &mut Snapshot, bus: u8) {
    for sample in snapshot.outputs.values_mut().filter(|s| s.bus == bus) {
        sample.invalidate("native scene recall requires fresh actual readback");
    }
}
pub(super) fn accept(
    snapshot: &Snapshot,
    completion: &mut Completion,
    hat: &hat::Hat,
) -> Result<Report, ServiceError> {
    let q = &completion.request;
    if q.reply.is_closed() || Instant::now() >= q.deadline {
        return Err(if q.action.writing() {
            ServiceError::Uncertain("scene recall caller ended; inspect hardware".into())
        } else {
            ServiceError::Expired
        });
    }
    let r = resolve(snapshot, &q.target, q.action, &q.context)?;
    if let Some(error) = completion.error.take() {
        return Err(error);
    }
    let mut observed = completion
        .observation
        .take()
        .ok_or_else(|| ServiceError::Rejected("missing scene result".into()))?;
    observed.stale |= !hat.evidence_current(r.bus, observed.stamp);
    if let Some(stored) = &mut observed.stored {
        stored.stale |= observed.stale;
        if stored.stale {
            stored.quality = Quality::Stale;
            stored.reason = Some("scene evidence changed before acceptance".into());
        }
    }
    if q.action.writing() && observed.stale {
        return Err(ServiceError::Uncertain(
            "scene recall evidence changed before acceptance".into(),
        ));
    }
    let group = match q.action {
        Action::Recall { group, .. } => group.map(GroupAddress::raw),
        _ => None,
    };
    let saved = r.address.and_then(|address| {
        settings_value::saved(&snapshot.model, r.bus, r.slot.raw(), address.raw())
    });
    let comparison =
        (!q.action.writing()).then(|| settings_value::compare(saved, observed.stored.as_ref()));
    Ok(Report {
        operation: q.action.name(),
        outcome: if q.action.writing() {
            "sent"
        } else {
            "observed"
        },
        target: q.target.clone(),
        target_generation: r.context.generation().unwrap().into(),
        revision: snapshot.model.inventory().revision,
        bus: r.bus,
        slot: r.slot.raw(),
        address: r.address.map(ShortAddress::raw),
        group,
        stored: observed.stored,
        saved,
        comparison,
    })
}
pub(super) async fn execute(
    link: Transactions<Serial>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let mut observation = Observation {
        stamp,
        stale: false,
        stored: None,
    };
    let mut refusal = None;
    let result: Result<(), TransactionError> = async {
        match route.action {
            Action::Read { .. } => {
                observation.stored = Some(read::stored(&link, &route, stamp, deadline).await?)
            }
            Action::Recall { group, .. } => {
                let destination = group.map_or(Destination::Broadcast, Destination::Group);
                let reply = frame(
                    &link,
                    &route,
                    stamp,
                    deadline,
                    route.slot.recall(destination),
                )
                .await?;
                if !matches!(reply, BusReply::NoResponse) {
                    refusal = Some(ServiceError::Uncertain(
                        "scene recall collided or returned data; inspect hardware".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    .await;
    Outcome {
        observation,
        recovery: matches!(result, Err(TransactionError::StreamLoss)),
        error: refusal.or_else(|| {
            result.err().map(|e| {
                if route.action.writing() {
                    ServiceError::Uncertain(e.to_string())
                } else {
                    ServiceError::Rejected(e.to_string())
                }
            })
        }),
    }
}
pub(super) async fn frame(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    value: u16,
) -> Result<BusReply, TransactionError> {
    match link
        .execute_until(
            Command::Send {
                bus: route.bus,
                frame: DaliFrame::Bits16(value),
                repeat: false,
            },
            deadline,
            Some((stamp.actor, stamp.bus)),
        )
        .await?
    {
        Response::Dali(mut replies) if replies.len() == 1 => Ok(replies.remove(0)),
        _ => Err(TransactionError::StreamLoss),
    }
}
impl ServiceHandle {
    pub async fn read_scene(
        &self,
        target: &str,
        slot: codec::Slot,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.scene(target, Action::Read { slot }, context, deadline)
            .await
    }
    /// Native recall is momentary; success confirms sending, not resulting light values.
    pub async fn recall_scene(
        &self,
        target: &str,
        group: Option<GroupAddress>,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.scene(
            target,
            Action::Recall {
                group,
                expected_revision,
            },
            context,
            deadline,
        )
        .await
    }
    pub(super) async fn scene(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        let permit = self.permit.clone().try_acquire_owned().map_err(|_| {
            if self.permit.is_closed() {
                ServiceError::Closed
            } else {
                ServiceError::Busy
            }
        })?;
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            action,
            &context,
        )?;
        admit(self, &route)?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Scene(Request {
                target: target.into(),
                bus: route.bus,
                action,
                context,
                deadline,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(if action.writing() {
                ServiceError::Uncertain("scene caller deadline; inspect hardware".into())
            } else {
                ServiceError::Expired
            });
        }
        result
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?
    }
}
