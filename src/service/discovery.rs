//! Bounded addressed probes followed by durable default-name adoption.
use super::*;
use crate::atx::{
    serial::Serial,
    transactions::{BusReply, Response, TransactionError, Transactions},
    wire::{Command, DaliFrame},
};
use crate::dali::{gear, input};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    Outputs,
    Inputs,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Complete,
    Partial,
    Missing,
    Collision,
    TypeMismatch,
}
/// Captured probe evidence; this is never current fixture State/availability.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Observation {
    pub bus: u8,
    pub space: Space,
    pub address: u8,
    pub quality: Quality,
    pub status: Option<u8>,
    pub device_types: Vec<u8>,
    pub types_complete: bool,
    pub instance_count: Option<u8>,
    pub instance_types: Vec<u8>,
    pub instances_complete: bool,
    #[serde(skip)]
    pub context: RequestContext,
    #[serde(skip)]
    pub(super) stamp: output::Stamp,
    #[serde(skip)]
    pub(super) stale: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub probe: Observation,
    pub revision: u64,
    pub target: Option<String>,
    pub added_items: usize,
}
pub(super) struct Request {
    pub bus: u8,
    pub space: Space,
    pub address: u8,
    pub expected: u64,
    pub context: RequestContext,
    pub deadline: Instant,
    pub completion: Option<mqtt_bridge_kit::CommandCompletion>,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub space: Space,
    pub address: u8,
    pub context: RequestContext,
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
    bus: u8,
    space: Space,
    address: u8,
    expected: u64,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(ServiceError::Rejected("routing is not ready".into()));
    }
    if expected != snapshot.model.inventory().revision {
        return Err(ServiceError::Rejected("inventory revision changed".into()));
    }
    if cause.generation() != Some(routing::generations(&snapshot.model).instance.as_str()) {
        return Err(ServiceError::Rejected(
            "missing or retired controller generation".into(),
        ));
    }
    if bus > 3
        || address > 63
        || !snapshot
            .model
            .inventory()
            .buses
            .iter()
            .any(|b| b.bus == bus && b.enabled)
    {
        return Err(ServiceError::Rejected(
            "unknown/disabled bus or invalid short address".into(),
        ));
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        space,
        address,
        context: cause.clone(),
    })
}

pub(super) async fn execute(
    link: Transactions<Serial>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let mut probe = Observation {
        bus: route.bus,
        space: route.space,
        address: route.address,
        quality: Quality::Missing,
        status: None,
        device_types: Vec::new(),
        types_complete: false,
        instance_count: None,
        instance_types: Vec::new(),
        instances_complete: false,
        context: route.context.clone(),
        stamp,
        stale: false,
    };
    let result: Result<(), TransactionError> = async {
        let status = match route.space {
            Space::Outputs => DaliFrame::Bits16(
                gear::ShortAddress::new(route.address)
                    .unwrap()
                    .query(gear::Query::Status),
            ),
            Space::Inputs => DaliFrame::Bits24(
                input::Address::new(route.address)
                    .unwrap()
                    .query(input::DeviceQuery::Status),
            ),
        };
        match frame(&link, &route, stamp, deadline, status).await? {
            BusReply::Value(status) => probe.status = Some(status),
            BusReply::NoResponse => return Ok(()),
            _ => {
                probe.quality = Quality::Collision;
                return Ok(());
            }
        }
        probe.quality = Quality::Partial;
        match route.space {
            Space::Outputs => {
                let address = gear::ShortAddress::new(route.address).unwrap();
                match frame(
                    &link,
                    &route,
                    stamp,
                    deadline,
                    DaliFrame::Bits16(address.query(gear::Query::DeviceType)),
                )
                .await?
                {
                    BusReply::Value(254) => probe.types_complete = true,
                    BusReply::Value(255) => {
                        let mut previous = None;
                        for _ in 0..255 {
                            match frame(
                                &link,
                                &route,
                                stamp,
                                deadline,
                                DaliFrame::Bits16(address.query(gear::Query::NextDeviceType)),
                            )
                            .await?
                            {
                                BusReply::Value(254) => {
                                    probe.types_complete = probe.device_types.len() >= 2;
                                    break;
                                }
                                BusReply::Value(value)
                                    if value < 254 && previous.is_none_or(|p| value > p) =>
                                {
                                    probe.device_types.push(value);
                                    previous = Some(value);
                                }
                                _ => break,
                            }
                        }
                    }
                    BusReply::Value(value) => {
                        probe.device_types.push(value);
                        probe.types_complete = true;
                    }
                    _ => {}
                }
                if probe.types_complete {
                    probe.quality = Quality::Complete;
                }
            }
            Space::Inputs => {
                let address = input::Address::new(route.address).unwrap();
                if let BusReply::Value(count) = frame(
                    &link,
                    &route,
                    stamp,
                    deadline,
                    DaliFrame::Bits24(address.query(input::DeviceQuery::NumberOfInstances)),
                )
                .await?
                {
                    probe.instance_count = Some(count);
                    if count > 32 {
                        return Ok(());
                    }
                    for number in 0..count {
                        let query = address.query_instance(
                            input::Instance::new(number).unwrap(),
                            input::InstanceQuery::Type,
                        );
                        match frame(&link, &route, stamp, deadline, DaliFrame::Bits24(query))
                            .await?
                        {
                            BusReply::Value(kind) if kind <= 31 => probe.instance_types.push(kind),
                            _ => return Ok(()),
                        }
                    }
                    probe.instances_complete = true;
                    probe.quality = Quality::Complete;
                }
            }
        }
        Ok(())
    }
    .await;
    let recovery = matches!(result, Err(TransactionError::StreamLoss));
    let error = result.err().map(|e| ServiceError::Rejected(e.to_string()));
    Outcome {
        observation: probe,
        error,
        recovery,
    }
}
async fn frame(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    frame: DaliFrame,
) -> Result<BusReply, TransactionError> {
    match link
        .execute_until(
            Command::Send {
                bus: route.bus,
                frame,
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

/// Propose additions only. Existing lifetimes and missing items are preserved.
pub(super) fn additions(model: &Model, probe: &mut Observation) -> Result<Vec<Edit>, ServiceError> {
    let bus = model
        .inventory()
        .buses
        .iter()
        .find(|b| b.bus == probe.bus)
        .ok_or_else(|| ServiceError::Rejected("unknown bus".into()))?;
    if probe.stale || probe.status.is_none() {
        return Ok(Vec::new());
    }
    match probe.space {
        Space::Outputs => {
            if bus.outputs.iter().any(|o| o.short_address == probe.address) {
                return Ok(Vec::new());
            }
            let name = unused_name("Light", bus.outputs.iter().map(|o| o.name.as_str()));
            Ok(vec![Edit::AddOutput {
                bus: probe.bus,
                output: crate::config::Output {
                    short_address: probe.address,
                    name,
                    description: None,
                    color_representation: None,
                },
            }])
        }
        Space::Inputs if probe.instances_complete => {
            let existing = bus.inputs.iter().find(|i| i.short_address == probe.address);
            if let Some(existing) = existing {
                if existing.instances.iter().any(|i| {
                    probe
                        .instance_types
                        .get(usize::from(i.number))
                        .is_some_and(|kind| *kind != i.instance_type)
                }) {
                    probe.quality = Quality::TypeMismatch;
                    return Ok(Vec::new());
                }
                let mut names: BTreeSet<String> =
                    existing.instances.iter().map(|i| i.name.clone()).collect();
                let mut edits = Vec::new();
                for (number, kind) in probe.instance_types.iter().enumerate() {
                    if existing
                        .instances
                        .iter()
                        .any(|i| usize::from(i.number) == number)
                    {
                        continue;
                    }
                    let name = unused_name("Input", names.iter().map(String::as_str));
                    names.insert(name.clone());
                    edits.push(Edit::AddInputInstance {
                        bus: probe.bus,
                        address: probe.address,
                        instance: crate::config::InputInstance {
                            number: number as u8,
                            name,
                            description: None,
                            instance_type: *kind,
                        },
                    });
                }
                Ok(edits)
            } else {
                let name = unused_name("Input", bus.inputs.iter().map(|i| i.name.as_str()));
                let instances = probe
                    .instance_types
                    .iter()
                    .enumerate()
                    .map(|(number, kind)| crate::config::InputInstance {
                        number: number as u8,
                        name: format!("Input{}", number + 1),
                        description: None,
                        instance_type: *kind,
                    })
                    .collect();
                Ok(vec![Edit::AddInput {
                    bus: probe.bus,
                    input: crate::config::Input {
                        short_address: probe.address,
                        name,
                        description: None,
                        instances,
                    },
                }])
            }
        }
        _ => Ok(Vec::new()),
    }
}
pub(super) fn unused_name<'a>(prefix: &str, names: impl Iterator<Item = &'a str>) -> String {
    let names: BTreeSet<_> = names.collect();
    (1u32..)
        .map(|n| format!("{prefix}{n}"))
        .find(|name| !names.contains(name.as_str()))
        .expect("bounded inventory has an unused default name")
}
pub(super) fn report(model: &Model, probe: Observation, added_items: usize) -> Report {
    let location = match probe.space {
        Space::Outputs => crate::inventory::Location::Output {
            bus: probe.bus,
            short_address: probe.address,
        },
        Space::Inputs => crate::inventory::Location::Input {
            bus: probe.bus,
            short_address: probe.address,
        },
    };
    let target = model
        .projections()
        .into_iter()
        .find(|p| p.location == location)
        .map(|p| p.target);
    Report {
        probe,
        revision: model.inventory().revision,
        target,
        added_items,
    }
}

impl ServiceHandle {
    /// Discover one addressed endpoint and durably adopt verified new inventory.
    pub async fn discover_address(
        &self,
        bus: u8,
        space: Space,
        address: u8,
        expected_revision: u64,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.discover(
            bus,
            space,
            address,
            expected_revision,
            context,
            deadline,
            None,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn discover(
        &self,
        bus: u8,
        space: Space,
        address: u8,
        expected: u64,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Report, ServiceError> {
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        if self.permit.is_closed() {
            return Err(ServiceError::Closed);
        }
        let permit = self
            .permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| ServiceError::Busy)?;
        let snapshot = self
            .snapshot()
            .ok_or_else(|| ServiceError::Rejected("service is starting".into()))?;
        resolve(&snapshot, bus, space, address, expected, &context)?;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Discovery(Request {
                bus,
                space,
                address,
                expected,
                context,
                deadline,
                completion,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "discovery caller deadline; inspect inventory".into(),
            ));
        }
        result
            .map_err(|_| {
                ServiceError::Uncertain("discovery caller deadline; inspect inventory".into())
            })?
            .map_err(|_| ServiceError::Closed)?
    }
}
