use super::*;
fn address(raw: u8) -> ShortAddress {
    ShortAddress::new(raw).unwrap()
}
fn group(raw: u8) -> GroupAddress {
    GroupAddress::new(raw).unwrap()
}
fn sample(address: u8, mask: u16, level: u8) -> Sample {
    let [low, high] = mask.to_le_bytes();
    Sample {
        address: super::tests::address(address),
        membership_low: Some(BusReply::Value(low)),
        membership_high: Some(BusReply::Value(high)),
        actual_level: Some(BusReply::Value(level)),
        status: Some(BusReply::Value(0)),
    }
}
#[test]
fn group_state_separates_uniform_power_from_mixed_native_levels() {
    for (levels, power, raw, power_mixed, level_mixed, quality) in [
        ([0, 0], Some(false), Some(0), false, false, Quality::Uniform),
        (
            [100, 100],
            Some(true),
            Some(100),
            false,
            false,
            Quality::Uniform,
        ),
        ([1, 254], Some(true), None, false, true, Quality::Mixed),
        ([0, 254], None, None, true, true, Quality::Mixed),
    ] {
        let s = summarize(
            group(0),
            &[address(0), address(63)],
            &[sample(0, 1, levels[0]), sample(63, 1, levels[1])],
        )
        .unwrap();
        assert_eq!(
            (s.quality, s.power, s.level.map(Level::raw)),
            (quality, power, raw),
            "group power/level uniformity was conflated"
        );
        assert_eq!(
            (s.power_mixed, s.level_mixed),
            (Some(power_mixed), Some(level_mixed))
        );
        assert_eq!(s.known_members, 2);
        assert!(s.unresolved.is_empty());
        assert_eq!(s.brightness(), raw.map(|v| f64::from(v) / 254.0));
    }
}
#[test]
fn group_state_incomplete_roster_cannot_promote_an_agreeing_prefix() {
    for tail in [
        None,
        Some(Sample::unvisited(address(63))),
        Some(sample(63, 1, 255)),
    ] {
        let mut samples = vec![sample(0, 1, 100)];
        if let Some(tail) = tail {
            samples.push(tail);
        }
        let s = summarize(group(0), &[address(0), address(63)], &samples).unwrap();
        assert_eq!(
            s.quality,
            Quality::Partial,
            "an incomplete saved roster became current"
        );
        assert_eq!(
            (s.power, s.level, s.power_mixed, s.level_mixed),
            (None, None, None, None)
        );
        assert_eq!(s.unresolved, vec![address(63)]);
    }
}
#[test]
fn group_state_missing_or_collided_membership_and_status_never_mean_off() {
    for field in 0..4 {
        for evidence in [
            None,
            Some(BusReply::NoResponse),
            Some(BusReply::ReceiveCollision(Some(7))),
            Some(BusReply::TransmitCollision(None)),
        ] {
            let mut row = sample(63, 0xffff, 0);
            *match field {
                0 => &mut row.membership_low,
                1 => &mut row.membership_high,
                2 => &mut row.actual_level,
                _ => &mut row.status,
            } = evidence;
            let s = summarize(group(15), &[address(63)], &[row]).unwrap();
            assert_eq!(
                s.quality,
                Quality::Partial,
                "missing/collided member evidence became usable"
            );
            assert_eq!((s.power, s.level), (None, None));
            assert_eq!(s.unresolved, vec![address(63)]);
        }
    }
    let mut row = Sample::unvisited(address(0));
    row.membership_low = Some(BusReply::NoResponse);
    row.membership_high = Some(BusReply::NoResponse);
    let s = summarize(group(0), &[address(0)], &[row]).unwrap();
    assert_eq!(s.quality, Quality::Unavailable);
    assert_eq!(s.power, None);
}
#[test]
fn group_state_uses_actual_membership_and_full_ff_bytes() {
    let mut outside = sample(0, 0, 0);
    outside.actual_level = None;
    outside.status = None;
    let s = summarize(
        group(15),
        &[address(0), address(63)],
        &[outside, sample(63, 0xffff, 254)],
    )
    .unwrap();
    assert_eq!(
        (s.quality, s.power, s.level.map(Level::raw), s.known_members),
        (Quality::Uniform, Some(true), Some(254), 1),
        "native high group or FF membership was lost"
    );
    let s = summarize(group(0), &[address(63)], &[sample(63, 0x8000, 255)]).unwrap();
    assert_eq!(
        (s.quality, s.power, s.known_members),
        (Quality::Empty, None, 0)
    );
    assert_eq!(
        summarize(group(0), &[], &[]).unwrap().quality,
        Quality::Empty
    );
}
#[test]
fn group_state_rejects_duplicate_or_outside_evidence() {
    assert_eq!(
        summarize(group(0), &[address(0), address(0)], &[]),
        Err(Error::DuplicateRoster(0))
    );
    assert_eq!(
        summarize(group(0), &[address(0)], &[sample(0, 1, 1), sample(0, 1, 1)]),
        Err(Error::DuplicateSample(0))
    );
    assert_eq!(
        summarize(group(0), &[address(0)], &[sample(63, 1, 1)]),
        Err(Error::UnexpectedSample(63))
    );
    assert_eq!(
        summarize(group(0), &vec![address(0); 65], &[]),
        Err(Error::TooMany)
    );
    let roster = (0..64).map(address).collect::<Vec<_>>();
    let samples = (0..64).map(|a| sample(a, 1, 1)).collect::<Vec<_>>();
    assert_eq!(
        summarize(group(0), &roster, &samples)
            .unwrap()
            .known_members,
        64
    );
}
