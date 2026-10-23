use super::*;

pub(in crate::service) async fn run(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Outcome {
    let mut sample = Observation {
        stamp,
        stale: false,
        programmed: matches!(route.plan, Plan::Program { .. }),
        stored: None,
        desired: None,
        candidate: None,
        reason: None,
    };
    let mut refusal = None;
    let result: Result<(), TransactionError> = async {
        let output_route = route.output_route();
        let mut capabilities = output::colour::Observation::default();
        if matches!(route.action, Action::Capture { .. }) {
            let actual = output::execute(
                link.clone(),
                output_route.clone(),
                stamp,
                None,
                true,
                deadline,
            )
            .await;
            if let Some(error) = actual.error {
                refusal = Some(error);
                return Ok(());
            }
            let level = capture::level(&link, &route, stamp, deadline, &actual.observation).await?;
            sample.candidate = level.and_then(|level| candidate(&actual.observation, level));
            if sample.candidate.is_none() {
                sample.reason =
                    Some("capture level or active colour is incomplete or unsupported; no capture candidate".into());
            }
            capabilities = actual.observation.colour.unwrap_or_default();
        } else {
            output::colour::discover(&mut capabilities, &link, &output_route, stamp, deadline)
                .await?;
        }
        let read_route = route.read_route();
        let stored =
            scenes::read::with_capabilities(&link, &read_route, stamp, deadline, &capabilities)
                .await?;
        if let Some(reviewed) = route.action.reviewed() {
            if reviewed != &stored || settings_value::from_stored(&stored).is_err() {
                refusal = Some(ServiceError::Rejected(
                    "stored scene changed or is unresolved; review again".into(),
                ));
                return Ok(());
            }
        }
        let desired = match &route.plan {
            Plan::Program { desired, .. } => Some(*desired),
            Plan::Probe => match &route.action {
                Action::Set { setting, .. } => Some(*setting),
                Action::Adopt { .. } => settings_value::from_stored(&stored).ok(),
                Action::Repair { .. } | Action::Capture { .. } => None,
            },
        };
        sample.stored = Some(stored);
        if let Some(desired) = desired {
            if !sample.stored.as_ref().unwrap().level.answered() {
                refusal = Some(ServiceError::Rejected(
                    "fixture presence is unconfirmed".into(),
                ));
                return Ok(());
            }
            if let Err(error) = preflight(desired, &capabilities) {
                refusal = Some(ServiceError::Rejected(error));
                return Ok(());
            }
            sample.desired = Some(desired);
            if matches!(route.plan, Plan::Program { .. }) {
                // Preparation evidence cannot stand in for a post-write report.
                sample.stored = None;
                program(
                    &link,
                    &route,
                    stamp,
                    deadline,
                    desired,
                    capabilities.dt8 == Some(true),
                )
                .await?;
                sample.stored = Some(
                    scenes::read::with_capabilities(
                        &link,
                        &read_route,
                        stamp,
                        deadline,
                        &capabilities,
                    )
                    .await?,
                );
            }
        }
        Ok(())
    }
    .await;
    Outcome {
        observation: sample,
        recovery: matches!(result, Err(TransactionError::StreamLoss)),
        error: refusal.or_else(|| result.err().map(|e| ServiceError::Rejected(e.to_string()))),
    }
}

pub(in crate::service) fn preflight(
    setting: Setting,
    caps: &output::colour::Observation,
) -> Result<(), String> {
    setting.validate()?;
    if setting == Setting::ignored() {
        return Ok(());
    }
    if caps.dt8.is_none() {
        return Err("DT8 presence must be confirmed before storing a participating row".into());
    }
    if matches!(setting.color, Color::Mask {}) {
        return Ok(());
    }
    let Some(Reply::Value(features)) = caps
        .features
        .filter(|f| matches!(f,Reply::Value(v) if v.counts_valid()))
    else {
        return Err("colour features are unconfirmed".into());
    };
    if caps.dt8 != Some(true) || !matches!(caps.gear_features, Some(Reply::Value(_))) {
        return Err("DT8 gear features must be confirmed".into());
    }
    match setting.color {
        Color::Temperature { .. } if features.temperature() => Ok(()),
        Color::Xy { .. } if features.xy() => Ok(()),
        Color::Rgbwaf { channels, control } if features.rgbwaf_channels() > 0 => {
            if !matches!(caps.control,Some(Reply::Value(raw)) if raw >> 6 == 2)
                || control.is_some_and(|raw| raw >> 6 != 2)
            {
                return Err("scene RGBWAF requires confirmed normalized mode".into());
            }
            if caps.assignments.len() != usize::from(features.rgbwaf_channels())
                || caps
                    .assignments
                    .iter()
                    .any(|r| !matches!(r,Reply::Value(a) if a.slot().is_some()))
            {
                return Err("RGBWAF assignments are unconfirmed".into());
            }
            for (slot, value) in channels.iter().enumerate() {
                if value.is_some()
                    && !caps
                        .assignments
                        .iter()
                        .any(|r| matches!(r,Reply::Value(a) if a.slot()==Some(slot)))
                {
                    return Err("requested scene channel has no physical assignment".into());
                }
            }
            Ok(())
        }
        _ => Err("requested scene colour is unsupported".into()),
    }
}
fn candidate(actual: &output::Observation, level: crate::dali::gear::Level) -> Option<Setting> {
    if actual.quality != output::Quality::Current {
        return None;
    }
    let caps = actual.colour.as_ref()?;
    let color = if caps.dt8 == Some(false) {
        Color::Mask {}
    } else {
        if caps.dt8 != Some(true) || caps.quality != output::colour::Quality::Current {
            return None;
        }
        use colour::{ByteValue, ColourType, ValueQuery as Q, WordValue};
        use output::colour::Value;
        let word = |q| match caps.value(q) {
            Some(Value::Word(WordValue::Known(v))) => Some(v),
            _ => None,
        };
        match caps.active()? {
            ColourType::Temperature => Color::Temperature {
                value: Some(word(Q::Temperature)?),
            },
            ColourType::Xy => Color::Xy {
                x: Some(word(Q::X)?),
                y: Some(word(Q::Y)?),
            },
            ColourType::Rgbwaf => {
                let mut channels = [None; 6];
                for (slot, q) in [Q::Red, Q::Green, Q::Blue, Q::White, Q::Amber, Q::Free]
                    .into_iter()
                    .enumerate()
                {
                    if caps
                        .assignments
                        .iter()
                        .any(|r| matches!(r,Reply::Value(a) if a.slot()==Some(slot)))
                    {
                        let Some(Value::Byte(ByteValue::Known(value))) = caps.value(q) else {
                            return None;
                        };
                        channels[slot] = Some(value);
                    }
                }
                let Some(Reply::Value(control)) = caps.control else {
                    return None;
                };
                Color::Rgbwaf {
                    channels,
                    control: Some(control),
                }
            }
            _ => return None,
        }
    };
    let candidate = Setting {
        level: Level::Value { value: level.raw() },
        color,
    };
    preflight(candidate, caps).ok()?;
    Some(candidate)
}
async fn program(
    link: &Transactions<Connection>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    setting: Setting,
    dt8: bool,
) -> Result<(), TransactionError> {
    step(
        link,
        route,
        stamp,
        deadline,
        route.slot.remove(route.address),
    )
    .await?;
    if setting == Setting::ignored() {
        return Ok(());
    }
    if dt8 {
        let read_route = route.read_route();
        // QuerySceneLevel selects the cleared destination report. Type query
        // confirms it before copying MASK, independent of pending actual colour.
        let masked =
            scenes::read::value(link, &read_route, stamp, deadline, codec::ReportValue::Type)
                .await?;
        if masked != (scenes::Reading::Mask { raw: 255 }) {
            return Err(TransactionError::StreamLoss);
        }
        for value in codec::copy_report_to_temporary(route.address) {
            step(link, route, stamp, deadline, codec::Step::Send(value)).await?;
        }
    }
    if let Some(color) = setting.native_color().expect("validated scene setting") {
        for value in color.staged_frames(route.address) {
            step(link, route, stamp, deadline, codec::Step::Send(value)).await?;
        }
    }
    for value in route.slot.store(
        route.address,
        setting.native_level().expect("validated scene level"),
    ) {
        step(link, route, stamp, deadline, value).await?;
    }
    Ok(())
}
pub(super) async fn step(
    link: &Transactions<Connection>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    step: codec::Step,
) -> Result<(), TransactionError> {
    let (value, repeat) = match step {
        codec::Step::Send(v) => (v, false),
        codec::Step::Repeat(v) => (v, true),
        codec::Step::Query(_) => unreachable!("settings plan"),
    };
    match link
        .execute_until(
            Command::Send {
                bus: route.bus,
                frame: DaliFrame::Bits16(value),
                repeat,
            },
            deadline,
            Some((stamp.actor, stamp.bus)),
        )
        .await?
    {
        Response::Dali(replies)
            if !replies.is_empty() && replies.iter().all(|r| matches!(r, BusReply::NoResponse)) =>
        {
            Ok(())
        }
        _ => Err(TransactionError::StreamLoss),
    }
}
