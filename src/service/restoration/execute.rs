//! One retained native plan. Never changes bridge inventory or saved identity.
use super::*;
use crate::{
    atx::{
        serial::Serial,
        transactions::{BusReply, SessionState, TransactionError, Transactions},
        wire::DaliFrame,
    },
    dali::{commissioning as gear_frames, input as device},
};
use commissioning::{Bus, Error, Selection};

async fn absent(bus: &Bus<'_>, address: u8) -> Result<(), Error> {
    if bus.query(bus.status(address)).await? != BusReply::NoResponse {
        return Err(Error::Evidence(
            "saved destination is occupied or absence is uncertain",
        ));
    }
    Ok(())
}
async fn present(bus: &Bus<'_>, address: u8) -> Result<(), Error> {
    if !matches!(bus.query(bus.status(address)).await?, BusReply::Value(_)) {
        return Err(Error::Evidence(
            "identified candidate presence is uncertain",
        ));
    }
    Ok(())
}
async fn input_compatible(bus: &Bus<'_>, route: &Route, source: Option<u8>) -> Result<(), Error> {
    if route.space != discovery::Space::Inputs {
        return Ok(());
    }
    let count = match source {
        Some(a) => device::Address::new(a)
            .unwrap()
            .query(device::DeviceQuery::NumberOfInstances),
        None => device::query_unaddressed(device::DeviceQuery::NumberOfInstances),
    };
    let BusReply::Value(count) = bus.query(DaliFrame::Bits24(count)).await? else {
        return Err(Error::Evidence(
            "complete input candidate topology required",
        ));
    };
    if count > 32 {
        return Err(Error::Evidence("invalid input candidate instance count"));
    }
    let mut types = Vec::new();
    for number in 0..count {
        let instance = device::Instance::new(number).unwrap();
        let frame = match source {
            Some(a) => device::Address::new(a)
                .unwrap()
                .query_instance(instance, device::InstanceQuery::Type),
            None => device::query_unaddressed_instance(instance, device::InstanceQuery::Type),
        };
        let BusReply::Value(kind) = bus.query(DaliFrame::Bits24(frame)).await? else {
            return Err(Error::Evidence(
                "complete input candidate topology required",
            ));
        };
        types.push(kind);
    }
    if route
        .instances
        .iter()
        .any(|(number, kind)| types.get(usize::from(*number)) != Some(kind))
    {
        return Err(Error::Evidence(
            "input candidate does not match saved instance types",
        ));
    }
    Ok(())
}
async fn run(bus: &Bus<'_>, route: &Route, entered: &mut bool) -> Result<Step, Error> {
    absent(bus, route.address).await?;
    match route.plan {
        Plan::Unique => {
            let random = match bus.only_unaddressed(entered).await? {
                Selection::None => return Ok(Step::NoCandidate),
                Selection::Multiple => return Ok(Step::Multiple),
                Selection::Found(random) => random,
            };
            input_compatible(bus, route, None).await?;
            let selected = commissioning::Candidate {
                address: route.address,
                random,
                stamp: bus.stamp,
            };
            bus.program(&selected, entered).await?;
            Ok(Step::Restored)
        }
        Plan::Identified => {
            let source = route
                .candidate
                .as_ref()
                .expect("identified candidate")
                .address;
            present(bus, source).await?;
            input_compatible(bus, route, Some(source)).await?;
            Ok(Step::ReadyToRebind)
        }
    }
}
pub(in crate::service) async fn execute(
    link: Transactions<Serial>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let bus = Bus {
        link: &link,
        bus: route.bus,
        space: route.space,
        stamp,
        deadline,
    };
    let mut entered = false;
    let mut result = run(&bus, &route, &mut entered).await;
    let mut recovery = matches!(
        &result,
        Err(Error::Transaction(TransactionError::StreamLoss))
    );
    if entered && link.status() == SessionState::Ready && Instant::now() < deadline {
        let cleanup = bus
            .command(
                bus.frame(gear_frames::TERMINATE, device::commissioning::TERMINATE),
                false,
            )
            .await;
        recovery |= matches!(
            &cleanup,
            Err(Error::Transaction(TransactionError::StreamLoss))
        );
        if result.is_ok() {
            if let Err(error) = cleanup {
                result = Err(error);
            }
        }
    }
    let (step, error) = match result {
        Ok(step) => (Some(step), None),
        Err(Error::Transaction(error)) => (None, Some(ServiceError::Uncertain(error.to_string()))),
        Err(Error::Evidence(reason)) => (None, Some(ServiceError::Rejected(reason.into()))),
    };
    Outcome {
        observation: Observation {
            stamp,
            stale: false,
            step,
        },
        error,
        recovery,
    }
}
