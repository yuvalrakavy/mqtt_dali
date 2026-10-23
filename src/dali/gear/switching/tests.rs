use super::*;

#[test]
fn dt7_threshold_endpoints_preserve_disabled_and_reject_upward_zero() {
    for raw in 0..=255 {
        assert_eq!(
            Threshold::from_raw(raw).raw(),
            raw,
            "DT7 threshold raw value was lost"
        );
        assert_eq!(
            UpThreshold::new(raw).is_ok(),
            raw != 0,
            "DT7 upward threshold validation changed"
        );
        if let Ok(value) = UpThreshold::new(raw) {
            assert_eq!(value.raw(), raw);
        }
    }
    assert_eq!(Threshold::from_raw(255), Threshold::Disabled);
    assert_eq!(
        Threshold::from_raw(0),
        Threshold::Level(Level::new(0).unwrap())
    );
}
#[test]
fn dt7_setter_plans_enable_once_before_one_repeated_addressed_command() {
    for (destination, address) in [
        (Destination::Short(ShortAddress::new(0).unwrap()), 1u16),
        (
            Destination::Group(super::super::GroupAddress::new(15).unwrap()),
            0x9f,
        ),
        (Destination::Broadcast, 0xff),
    ] {
        for (setting, dtr, opcode) in [
            (Setting::UpOn(UpThreshold::new(17).unwrap()), 0xa311, 0xe1),
            (Setting::UpOff(UpThreshold::new(255).unwrap()), 0xa3ff, 0xe2),
            (Setting::DownOn(Threshold::from_raw(0)), 0xa300, 0xe3),
            (Setting::DownOff(Threshold::from_raw(123)), 0xa37b, 0xe4),
        ] {
            assert_eq!(
                setting.plan(destination),
                [
                    Step {
                        frame: dtr,
                        repeat: false
                    },
                    Step {
                        frame: 0xc107,
                        repeat: false
                    },
                    Step {
                        frame: (address << 8) | opcode,
                        repeat: true
                    }
                ],
                "DT7 threshold plan changed enable/order/repeat/scope"
            );
        }
    }
}
#[test]
fn dt7_query_plans_and_correlated_uncertainty_are_literal() {
    for (query, opcode) in [
        (Query::Features, 0xf0),
        (Query::SwitchStatus, 0xf1),
        (Query::UpOn, 0xf2),
        (Query::UpOff, 0xf3),
        (Query::DownOn, 0xf4),
        (Query::DownOff, 0xf5),
        (Query::GearType, 0xf7),
        (Query::ExtendedVersion, 0xff),
    ] {
        assert_eq!(
            query.plan(ShortAddress::new(63).unwrap()),
            [0xc107, 0x7f00 | opcode],
            "DT7 individual query plan changed enable or opcode"
        );
        assert_eq!(
            query.decode(BusReply::NoResponse),
            Reply::NoResponse,
            "DT7 silence was decoded as a value"
        );
        for code in [None, Some(0), Some(255)] {
            assert_eq!(
                query.decode(BusReply::ReceiveCollision(code)),
                Reply::ReceiveCollision(code)
            );
            assert_eq!(
                query.decode(BusReply::TransmitCollision(code)),
                Reply::TransmitCollision(code)
            );
        }
    }
    assert_eq!(
        Query::UpOn.decode(BusReply::Value(0)),
        Reply::InvalidValue {
            query: Query::UpOn,
            raw: 0
        }
    );
    assert_eq!(
        Query::UpOff.decode(BusReply::Value(255)),
        Reply::Value(Value::UpOff(UpThreshold::new(255).unwrap()))
    );
    assert_eq!(
        Query::DownOff.decode(BusReply::Value(0)),
        Reply::Value(Value::DownOff(Threshold::from_raw(0)))
    );
    assert_eq!(
        Query::SwitchStatus.decode(BusReply::Value(255)),
        Reply::Value(Value::SwitchStatus(255))
    );
    assert_eq!(
        Query::ExtendedVersion.decode(BusReply::Value(255)),
        Reply::Value(Value::ExtendedVersion(255))
    );
}
#[test]
fn dt7_feature_and_gear_type_bits_keep_raw_unknown_fields() {
    for (raw, adjustable, selection) in [
        (0, false, false),
        (8, true, false),
        (128, false, true),
        (136, true, true),
        (119, false, false),
        (255, true, true),
    ] {
        let Reply::Value(Value::Features(value)) = Query::Features.decode(BusReply::Value(raw))
        else {
            panic!("feature byte lost")
        };
        assert_eq!(value.raw(), raw);
        assert_eq!(value.adjustable_thresholds(), adjustable);
        assert_eq!(value.physical_selection(), selection);
    }
    for (raw, flags) in [
        (0, [false; 5]),
        (1, [true, false, false, false, false]),
        (2, [false, true, false, false, false]),
        (4, [false, false, true, false, false]),
        (8, [false, false, false, true, false]),
        (16, [false, false, false, false, true]),
        (21, [true, false, true, false, true]),
        (224, [false; 5]),
        (255, [true; 5]),
    ] {
        let Reply::Value(Value::GearType(value)) = Query::GearType.decode(BusReply::Value(raw))
        else {
            panic!("gear type byte lost")
        };
        assert_eq!(value.raw(), raw);
        assert_eq!(
            [
                value.electronic_switch(),
                value.make_contact(),
                value.break_contact(),
                value.voltage_protection(),
                value.inrush_limiting()
            ],
            flags
        );
    }
}
