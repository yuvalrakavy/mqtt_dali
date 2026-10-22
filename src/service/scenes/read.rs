use super::*;

/// Capability probes finish before any stored report is selected. Their actual
/// colour quality is independent from completeness of this stored configuration.
pub(in crate::service) async fn stored(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
) -> Result<Stored, TransactionError> {
    let address = route.address.expect("scene read has an output address");
    let output_route = output::Route {
        controller: route.controller.clone(),
        bus: route.bus,
        address,
        context: route.context.clone(),
    };
    let mut capabilities = output::colour::Observation::default();
    output::colour::discover(&mut capabilities, link, &output_route, stamp, deadline).await?;
    with_capabilities(link, route, stamp, deadline, &capabilities).await
}

pub(in crate::service) async fn with_capabilities(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    capabilities: &output::colour::Observation,
) -> Result<Stored, TransactionError> {
    let address = route.address.expect("scene read has an output address");
    let level = Reading::byte(
        frame(link, route, stamp, deadline, route.slot.query(address)).await?,
        true,
    );
    let mut sample = Stored {
        level,
        dt8: capabilities.dt8,
        color_type: None,
        values: Default::default(),
        quality: Quality::Partial,
        stale: false,
        reason: None,
    };
    if capabilities.dt8 == Some(false) {
        if sample.level.answered() {
            sample.quality = Quality::Complete;
        }
        return Ok(sample);
    }
    if capabilities.dt8 != Some(true) {
        sample.reason = Some("DT8 presence is unconfirmed".into());
        return Ok(sample);
    }
    let kind = value(link, route, stamp, deadline, codec::ReportValue::Type).await?;
    sample.color_type = Some(kind.clone());
    if matches!(kind, Reading::Mask { .. }) {
        if sample.level.answered() {
            sample.quality = Quality::Complete;
        }
        return Ok(sample);
    }
    let Reading::Value { raw } = kind else {
        sample.reason = Some("stored colour type is unresolved".into());
        return Ok(sample);
    };
    use codec::ReportValue as V;
    let Some(Reply::Value(features)) = capabilities
        .features
        .filter(|f| matches!(f,Reply::Value(v) if v.counts_valid()))
    else {
        sample.reason = Some("colour features are unconfirmed".into());
        return Ok(sample);
    };
    let fields: &[(&str, V)] = match raw {
        0x10 if features.xy() => &[("x", V::X), ("y", V::Y)],
        0x20 if features.temperature() => &[("temperature", V::Temperature)],
        0x80 if features.rgbwaf_channels() > 0 => &[
            ("red", V::Red),
            ("green", V::Green),
            ("blue", V::Blue),
            ("white", V::White),
            ("amber", V::Amber),
            ("free", V::Free),
            ("control", V::RgbwafControl),
        ],
        _ => {
            sample.quality = Quality::Unsupported;
            sample.reason = Some(format!(
                "stored colour type {raw} is unsupported or contradicts capabilities"
            ));
            return Ok(sample);
        }
    };
    for (name, selector) in fields {
        sample.values.insert(
            (*name).into(),
            value(link, route, stamp, deadline, *selector).await?,
        );
    }
    if sample.level.answered() && sample.values.values().all(Reading::answered) {
        sample.quality = Quality::Complete;
    } else {
        sample.reason = Some("stored scene data is incomplete or collided".into());
    }
    Ok(sample)
}

pub(in crate::service) async fn value(
    link: &Transactions<Serial>,
    route: &Route,
    stamp: output::Stamp,
    deadline: Instant,
    selector: codec::ReportValue,
) -> Result<Reading, TransactionError> {
    let mut replies = Vec::new();
    for step in selector.plan(route.address.expect("scene read address"), route.slot) {
        match step {
            codec::Step::Send(value) => {
                if !matches!(
                    frame(link, route, stamp, deadline, value).await?,
                    BusReply::NoResponse
                ) {
                    return Err(TransactionError::StreamLoss);
                }
            }
            codec::Step::Query(value) => {
                let reply = frame(link, route, stamp, deadline, value).await?;
                if !matches!(reply, BusReply::Value(_)) {
                    return Ok(Reading::byte(reply, false));
                }
                replies.push(reply);
            }
            codec::Step::Repeat(_) => unreachable!("stored scene query has no repeated settings"),
        }
    }
    match selector.width() {
        Width::Byte => Ok(Reading::byte(
            replies.pop().expect("byte query result"),
            true,
        )),
        Width::Word => {
            let low = replies.pop().expect("word low result");
            let high = replies.pop().expect("word high result");
            match colour_codec::word_reply(high, low) {
                Reply::Value(colour_codec::WordValue::Known(raw)) => Ok(Reading::Value { raw }),
                Reply::Value(colour_codec::WordValue::Mask) => Ok(Reading::Mask { raw: 65535 }),
                _ => unreachable!("all word query steps answered"),
            }
        }
    }
}
