use super::*;
use crate::{
    atx::transactions::BusReply,
    dali::gear::{GroupAddress, LevelValue, Query, QueryReply, QueryValue, ShortAddress},
};

#[test]
fn gear_settings_checked_limits_and_context_specific_255_are_distinct() {
    for raw in 1..=254 {
        let value = LimitLevel::new(raw).unwrap();
        assert_eq!(value.raw(), raw);
        assert_eq!(value.normalized(), f64::from(raw) / 254.0);
    }
    assert!(LimitLevel::new(0).is_err() && LimitLevel::new(255).is_err());
    assert_eq!(
        PowerOnLevel::from_raw(0),
        PowerOnLevel::Fixed(Level::new(0).unwrap())
    );
    assert_eq!(
        SystemFailureLevel::from_raw(254),
        SystemFailureLevel::Fixed(Level::new(254).unwrap())
    );
    assert_eq!(
        Query::PowerOnLevel.decode(BusReply::Value(255)),
        QueryReply::Value(QueryValue::PowerOnLevel(PowerOnLevel::LastActive)),
        "startup255 lost its last-active meaning"
    );
    assert_eq!(
        Query::SystemFailureLevel.decode(BusReply::Value(255)),
        QueryReply::Value(QueryValue::SystemFailureLevel(
            SystemFailureLevel::Unchanged
        )),
        "failure255 lost its unchanged meaning"
    );
    assert_eq!(PowerOnLevel::LastActive.raw(), 255);
    assert_eq!(SystemFailureLevel::Unchanged.raw(), 255);
    assert_eq!(
        Query::ActualLevel.decode(BusReply::Value(255)),
        QueryReply::Value(QueryValue::ActualLevel(LevelValue::UnavailableMask))
    );
}

#[test]
fn gear_settings_queries_keep_literal_opcodes_and_uncertain_evidence() {
    for (query, frame) in [
        (Query::PowerOnLevel, 0x7fa3),
        (Query::SystemFailureLevel, 0x7fa4),
        (Query::FadeTimeRate, 0x7fa5),
    ] {
        assert_eq!(
            ShortAddress::new(63).unwrap().query(query),
            frame,
            "settings query opcode/address changed"
        );
        for (reply, expected) in [
            (BusReply::NoResponse, QueryReply::NoResponse),
            (
                BusReply::ReceiveCollision(Some(9)),
                QueryReply::ReceiveCollision(Some(9)),
            ),
            (
                BusReply::TransmitCollision(None),
                QueryReply::TransmitCollision(None),
            ),
        ] {
            assert_eq!(
                query.decode(reply),
                expected,
                "settings query invented a value from uncertain evidence"
            );
        }
    }
}

#[test]
fn gear_settings_fade_bytes_preserve_asymmetric_nibbles_ff_and_reserved_zero() {
    for (raw, time, rate) in [
        (0xf1, 15, Some(1)),
        (0x1f, 1, Some(15)),
        (0xff, 15, Some(15)),
        (0xf0, 15, None),
        (0x07, 0, Some(7)),
    ] {
        let QueryReply::Value(QueryValue::FadeTimeRate(value)) =
            Query::FadeTimeRate.decode(BusReply::Value(raw))
        else {
            panic!("fade byte became a MASK or lost query type");
        };
        assert_eq!(
            (
                value.raw(),
                value.time().raw(),
                value.rate().map(FadeRate::raw)
            ),
            (raw, time, rate),
            "fade nibbles or FF numeric codes were lost"
        );
    }
    assert_eq!(
        FadeTime::new(0).unwrap().seconds(),
        None,
        "basic fade0 promised instant output"
    );
    assert!(FadeTime::new(16).is_err() && FadeTime::new(255).is_err());
    assert!(FadeRate::new(0).is_err() && FadeRate::new(16).is_err());
}

#[test]
fn gear_settings_timing_conversions_match_independent_native_tables() {
    for (code, seconds) in [
        (1, 0.7071067811865476),
        (2, 1.0),
        (8, 8.0),
        (15, 90.50966799187809),
    ] {
        assert!(
            (FadeTime::new(code).unwrap().seconds().unwrap() - seconds).abs() < 1e-10,
            "fade time code did not match native seconds"
        );
    }
    for (code, steps) in [(1, 357.796), (2, 253.0), (7, 44.725), (15, 2.795)] {
        assert!(
            (FadeRate::new(code).unwrap().steps_per_second() - steps).abs() < 0.001,
            "fade rate code did not match native steps per second"
        );
    }
}

#[test]
fn gear_settings_native_plans_have_exact_dtr_setter_destination_and_repeat() {
    let short0 = Destination::Short(ShortAddress::new(0).unwrap());
    let short63 = Destination::Short(ShortAddress::new(63).unwrap());
    let group15 = Destination::Group(GroupAddress::new(15).unwrap());
    for (setting, destination, dtr, command) in [
        (
            Setting::Minimum(LimitLevel::new(1).unwrap()),
            short0,
            0xa301,
            0x012b,
        ),
        (
            Setting::Maximum(LimitLevel::new(254).unwrap()),
            short63,
            0xa3fe,
            0x7f2a,
        ),
        (
            Setting::PowerOn(PowerOnLevel::LastActive),
            group15,
            0xa3ff,
            0x9f2d,
        ),
        (
            Setting::SystemFailure(SystemFailureLevel::Unchanged),
            Destination::Broadcast,
            0xa3ff,
            0xff2c,
        ),
        (
            Setting::FadeTime(FadeTime::new(15).unwrap()),
            group15,
            0xa30f,
            0x9f2e,
        ),
        (
            Setting::FadeRate(FadeRate::new(1).unwrap()),
            short63,
            0xa301,
            0x7f2f,
        ),
        (
            Setting::PowerOn(PowerOnLevel::Fixed(Level::new(0).unwrap())),
            short0,
            0xa300,
            0x012d,
        ),
        (
            Setting::SystemFailure(SystemFailureLevel::Fixed(Level::new(127).unwrap())),
            short63,
            0xa37f,
            0x7f2c,
        ),
        (
            Setting::FadeTime(FadeTime::new(0).unwrap()),
            Destination::Broadcast,
            0xa300,
            0xff2e,
        ),
    ] {
        assert_eq!(
            setting.plan(destination),
            [
                Step {
                    frame: dtr,
                    repeat: false
                },
                Step {
                    frame: command,
                    repeat: true
                }
            ],
            "settings native DTR/setter/repeat vector changed"
        );
    }
}
