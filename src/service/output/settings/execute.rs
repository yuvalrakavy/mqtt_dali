use super::*;
use crate::{
    atx::{
        connection::Connection,
        transactions::{BusReply, TransactionError, Transactions},
        wire::DaliFrame,
    },
    dali::gear::{Destination, Query, QueryReply, QueryValue},
    service::{
        commissioning::{Bus, Error},
        discovery::Space,
        scenes::Reading,
    },
};
use std::collections::BTreeMap;

fn byte(r: &Reading) -> Option<u8> {
    if let Reading::Value { raw } = r {
        u8::try_from(*raw).ok()
    } else {
        None
    }
}
fn limit(r: &Reading) -> Option<u8> {
    byte(r).filter(|v| (1..=254).contains(v))
}
struct Probe {
    values: Option<Values>,
    readings: BTreeMap<&'static str, Reading>,
    flags: Option<StatusFlags>,
}
async fn probe(bus: &Bus<'_>, r: &Route) -> Result<Probe, Error> {
    let mut readings = BTreeMap::new();
    for (name, query, masked) in [
        ("status", Query::Status, false),
        ("version", Query::Version, false),
        ("actual_level", Query::ActualLevel, true),
        ("physical_minimum", Query::PhysicalMinimum, true),
        ("minimum_level", Query::MinimumLevel, true),
        ("maximum_level", Query::MaximumLevel, true),
        ("power_on_level", Query::PowerOnLevel, false),
        ("system_failure_level", Query::SystemFailureLevel, false),
        ("fade_time_rate", Query::FadeTimeRate, false),
    ] {
        let reply = bus
            .query(DaliFrame::Bits16(r.output.address.query(query)))
            .await?;
        readings.insert(name, Reading::byte(reply, masked));
    }
    let (switching_support, switching) =
        switching::probe(bus, r.output.address, &mut readings).await?;
    readings.insert(
        "closing_status",
        Reading::byte(
            bus.query(DaliFrame::Bits16(r.output.address.query(Query::Status)))
                .await?,
            false,
        ),
    );
    let present =
        byte(&readings["status"]).is_some() && byte(&readings["closing_status"]).is_some();
    let flags = byte(&readings["closing_status"]).map(|raw| {
        let QueryReply::Value(QueryValue::Status(s)) = Query::Status.decode(BusReply::Value(raw))
        else {
            unreachable!()
        };
        StatusFlags {
            gear_failure: s.gear_failure(),
            lamp_failure: s.lamp_failure(),
            lamp_on: s.lamp_on(),
            limit_error: s.limit_error(),
            fade_running: s.fade_running(),
            reset_state: s.reset_state(),
            missing_short_address: s.missing_short_address(),
            power_failure: s.power_failure(),
        }
    });
    let values = present.then(|| {
        let fade_raw = byte(&readings["fade_time_rate"]);
        let mut v = Values {
            physical_minimum: limit(&readings["physical_minimum"]),
            minimum_level: limit(&readings["minimum_level"]),
            maximum_level: limit(&readings["maximum_level"]),
            power_on_level: byte(&readings["power_on_level"]),
            system_failure_level: byte(&readings["system_failure_level"]),
            fade_raw,
            fade_time: fade_raw.map(|raw| raw >> 4),
            fade_rate: fade_raw.map(|raw| raw & 15).filter(|v| *v != 0),
            switching_support,
            switching,
            quality: "partial",
        };
        if v.limit_floor()
            .zip(v.minimum_level)
            .zip(v.maximum_level)
            .is_some_and(|((physical, min), max)| physical <= min && min <= max)
            && v.power_on_level.is_some()
            && v.system_failure_level.is_some()
            && v.fade_raw.is_some()
            && v.fade_rate.is_some()
        {
            v.quality = "complete";
        }
        v
    });
    Ok(Probe {
        values,
        readings,
        flags,
    })
}
pub(in crate::service) async fn run(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = Bus {
        link: &link,
        bus: route.output.bus,
        space: Space::Outputs,
        stamp,
        deadline,
    };
    let mut report = report(&target, &route);
    let mut attempted = false;
    let result: Result<(), Error> = async {
        let first = probe(&bus, &route).await?;
        report.settings = first.values;
        report.readings = Some(first.readings);
        report.status_flags = first.flags;
        report.outcome = match report.settings.as_ref() {
            Some(v) if v.quality == "complete" => "current",
            Some(_) => "partial",
            None => "unavailable",
        };
        let Action::Configure { settings, .. } = &route.action else {
            return Ok(());
        };
        let (plans, switching_plans) = match report
            .settings
            .as_ref()
            .ok_or_else(|| reject("fresh fixture presence required"))
            .and_then(|before| settings.plans(before))
        {
            Ok(plans) => plans,
            Err(e) => {
                report.outcome = "rejected";
                report.reason = Some(e.to_string());
                return Ok(());
            }
        };
        report.before = report.settings.take();
        report.readings = None;
        report.status_flags = None;
        report.outcome = "unconfirmed";
        for setting in plans {
            for step in setting.plan(Destination::Short(route.output.address)) {
                attempted = true;
                bus.command(DaliFrame::Bits16(step.frame), step.repeat)
                    .await?;
            }
        }
        for setting in switching_plans {
            for step in setting.plan(Destination::Short(route.output.address)) {
                attempted = true;
                bus.command(DaliFrame::Bits16(step.frame), step.repeat)
                    .await?;
            }
        }
        let after = probe(&bus, &route).await?;
        report.settings = after.values;
        report.readings = Some(after.readings);
        report.status_flags = after.flags;
        if report
            .settings
            .as_ref()
            .is_some_and(|v| settings.matches(v))
        {
            report.outcome = "configured";
        } else {
            report.reason =
                Some("output settings were not confirmed by matching actual readback".into());
        }
        Ok(())
    }
    .await;
    let recovery = matches!(
        &result,
        Err(Error::Transaction(TransactionError::StreamLoss))
    );
    let mut error = None;
    if let Err(e) = result {
        let reason = match e {
            Error::Transaction(e) => e.to_string(),
            Error::Evidence(e) => e.into(),
        };
        report.reason = Some(reason.clone());
        report.settings = None;
        report.readings = None;
        report.status_flags = None;
        if attempted {
            report.outcome = "unconfirmed";
        } else {
            error = Some(reject(reason));
        }
    }
    Outcome {
        observation: Observation {
            report: Box::new(report),
            stamp,
            stale: false,
        },
        error,
        recovery,
    }
}
