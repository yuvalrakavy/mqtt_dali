//! DT8 sampling and group writes inside the existing owned group operation.
use super::*;
use std::collections::BTreeMap;
pub(super) type Samples = BTreeMap<u8, output::colour::Observation>;

pub(super) fn included(route: &Route) -> bool {
    matches!(route.action, Action::ReadColor | Action::Color { .. })
        || matches!(route.action, Action::Read) && route.representation.is_some()
}
fn member(sample: &Sample, route: &Route) -> bool {
    sample
        .membership()
        .is_some_and(|mask| mask & (1 << route.group.raw()) != 0)
}
pub(super) fn roster(route: &Route, samples: &[Sample]) -> Result<(), String> {
    if samples.iter().any(|sample| sample.membership().is_none()) {
        return Err("group color requires complete saved-roster membership evidence".into());
    }
    if !samples.iter().any(|sample| member(sample, route)) {
        return Err("group color has no known hardware members".into());
    }
    Ok(())
}
pub(super) async fn read(
    bus: &commissioning::Bus<'_>,
    route: &Route,
    samples: &[Sample],
    colors: &mut Samples,
    refresh: bool,
) -> Result<(), commissioning::Error> {
    colors.retain(|address, _| {
        samples
            .iter()
            .any(|sample| sample.address.raw() == *address && member(sample, route))
    });
    for sample in samples.iter().filter(|sample| member(sample, route)) {
        let address = sample.address.raw();
        let native = output::Route {
            controller: route.controller.clone(),
            bus: route.bus,
            address: sample.address,
            context: route.context.clone(),
        };
        let captured = colors.entry(address).or_default();
        if refresh && captured.dt8 == Some(true) {
            output::colour::read_values(captured, bus.link, &native, bus.stamp, bus.deadline)
                .await?;
        } else {
            *captured = output::colour::Observation::default();
            output::colour::discover(captured, bus.link, &native, bus.stamp, bus.deadline).await?;
        }
    }
    Ok(())
}
pub(super) fn preflight(
    route: &Route,
    samples: &[Sample],
    colors: &Samples,
    intent: crate::dali::colour::Intent,
) -> Result<(), String> {
    roster(route, samples)?;
    for sample in samples.iter().filter(|sample| member(sample, route)) {
        colors
            .get(&sample.address.raw())
            .ok_or("group member has no color evidence")?
            .preflight(intent)
            .map_err(|reason| format!("group member {}: {reason}", sample.address.raw()))?;
    }
    Ok(())
}
pub(super) fn invalidate(colors: &mut Samples) {
    for sample in colors.values_mut() {
        sample.quality = output::colour::Quality::Partial;
        sample.values.clear();
        sample.raw_values.clear();
        sample.status = None;
    }
}
pub(super) async fn write(
    bus: &commissioning::Bus<'_>,
    route: &Route,
    intent: output::WriteIntent,
) -> Result<(), commissioning::Error> {
    let destination = Destination::Group(route.group);
    if let Some(base) = intent.base().filter(|base| base.is_off()) {
        bus.command(DaliFrame::Bits16(base.frame(destination)), false)
            .await?;
    }
    for frame in intent
        .colour()
        .expect("validated group color intent")
        .frames(destination)
    {
        bus.command(DaliFrame::Bits16(frame), false).await?;
    }
    if let Some(base) = intent.base().filter(|base| !base.is_off()) {
        bus.command(DaliFrame::Bits16(base.frame(destination)), false)
            .await?;
    }
    Ok(())
}
pub(super) fn confirms(
    route: &Route,
    samples: &[Sample],
    colors: &Samples,
    intent: crate::dali::colour::Intent,
) -> bool {
    roster(route, samples).is_ok()
        && samples
            .iter()
            .filter(|sample| member(sample, route))
            .all(|sample| {
                colors
                    .get(&sample.address.raw())
                    .is_some_and(|color| color.confirms(intent))
            })
}
