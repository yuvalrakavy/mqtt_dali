use super::*;
use crate::dali::gear::GroupAddress;

fn short(raw: u8) -> Destination {
    Destination::Short(ShortAddress::new(raw).unwrap())
}

#[test]
fn physical_channel_queries_keep_assignment_separate_from_channel_count() {
    assert_eq!(
        ChannelIndex::new(5)
            .unwrap()
            .query(ShortAddress::new(3).unwrap()),
        vec![Step::Send(0xa305), Step::Send(0xc108), Step::Query(0x07fc)],
        "assigned-colour query changed its channel register or opcode"
    );
    assert!(ChannelIndex::new(6).is_err());
    for (raw, slot) in [
        (0, None),
        (1, Some(0)),
        (3, Some(2)),
        (6, Some(5)),
        (7, None),
        (255, None),
    ] {
        assert_eq!(AssignedColour::decode(raw).slot(), slot);
    }
    assert_eq!(AssignedColour::decode(42), AssignedColour::Unknown(42));
    assert_eq!(
        Intent::temperature(Temperature::new(250).unwrap()).kind(),
        ColourType::Temperature
    );
}

#[test]
fn colour_values_validate_masks_units_and_normalized_boundaries() {
    for raw in [0, 65535] {
        assert!(
            Temperature::new(raw).is_err(),
            "invalid Tc became a normal value"
        );
    }
    assert_eq!(Temperature::from_kelvin(4000.0).unwrap().raw(), 250);
    assert_eq!(Temperature::new(250).unwrap().kelvin(), 4000.0);
    assert_eq!(Temperature::from_kelvin(2700.0).unwrap().raw(), 370);
    for raw in 1..=65534 {
        let value = Temperature::new(raw).unwrap();
        assert_eq!(Temperature::from_kelvin(value.kelvin()).unwrap(), value);
    }
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1.0, 1000001.0] {
        assert!(
            Temperature::from_kelvin(bad).is_err(),
            "unrepresentable Kelvin was rounded into range"
        );
    }
    assert!(Coordinate::new(65535).is_err());
    assert_eq!(
        Coordinate::from_normalized(0.5).unwrap().raw(),
        32768,
        "xy used the wrong fixed-point denominator"
    );
    assert_eq!(
        Coordinate::new(65534).unwrap().normalized(),
        65534.0 / 65536.0
    );
    for bad in [1.0, -0.001, f64::NAN, f64::INFINITY] {
        assert!(Coordinate::from_normalized(bad).is_err());
    }
    for raw in 0..=65534 {
        let value = Coordinate::new(raw).unwrap();
        assert_eq!(
            Coordinate::from_normalized(value.normalized()).unwrap(),
            value
        );
    }
    assert!(Channel::new(255).is_err());
    for raw in 0..=254 {
        let value = Channel::new(raw).unwrap();
        assert_eq!(Channel::from_normalized(value.normalized()).unwrap(), value);
    }
    assert_eq!(
        Channel::from_normalized(1.0).unwrap().raw(),
        254,
        "maximum channel selected MASK"
    );
    assert_eq!(Channel::from_normalized(0.001).unwrap().raw(), 0);
    for bad in [-0.01, 1.00001, f64::NAN, f64::INFINITY] {
        assert!(Channel::from_normalized(bad).is_err());
    }
}

#[test]
fn tc_and_xy_plans_match_literal_frames_and_never_drive_arc_power() {
    assert_eq!(
        Intent::temperature(Temperature::new(370).unwrap()).frames(short(3)),
        [0xa372, 0xc301, 0xc108, 0x07e7, 0xc108, 0x07e2],
        "Tc DTR byte order or per-command DT8 enable changed"
    );
    assert_eq!(
        Intent::xy(
            Coordinate::new(0x1234).unwrap(),
            Coordinate::new(0xabcd).unwrap()
        )
        .frames(short(0)),
        [0xa334, 0xc312, 0xc108, 0x01e0, 0xa3cd, 0xc3ab, 0xc108, 0x01e1, 0xc108, 0x01e2],
        "xy coordinate sequence changed"
    );
}

#[test]
fn rgbwaf_plans_preserve_untouched_channels_and_zero_is_a_real_value() {
    let channels = [
        Some(Channel::new(254).unwrap()),
        Some(Channel::new(0).unwrap()),
        Some(Channel::new(127).unwrap()),
        None,
        None,
        None,
    ];
    assert_eq!(
        Intent::rgbwaf(channels).unwrap().frames(short(63)),
        [
            0xa3fe, 0xc300, 0xc57f, 0xc108, 0x7feb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x7fec, 0xc108,
            0x7fe2
        ],
        "RGB write cleared unrequested WAF channels or changed control mode"
    );
    let waf = [
        None,
        None,
        None,
        Some(Channel::new(4).unwrap()),
        Some(Channel::new(5).unwrap()),
        Some(Channel::new(6).unwrap()),
    ];
    assert_eq!(
        Intent::rgbwaf(waf)
            .unwrap()
            .frames(Destination::Group(GroupAddress::new(15).unwrap())),
        [
            0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x9feb, 0xa304, 0xc305, 0xc506, 0xc108, 0x9fec, 0xc108,
            0x9fe2
        ]
    );
    assert!(
        Intent::rgbwaf([None; 6]).is_err(),
        "empty RGBWAF intent was accepted"
    );
}

#[test]
fn colour_destinations_share_validated_output_address_spaces() {
    let intent = Intent::temperature(Temperature::new(250).unwrap());
    for (target, commands) in [
        (short(0), [0x01e7, 0x01e2]),
        (short(63), [0x7fe7, 0x7fe2]),
        (
            Destination::Group(GroupAddress::new(0).unwrap()),
            [0x81e7, 0x81e2],
        ),
        (
            Destination::Group(GroupAddress::new(15).unwrap()),
            [0x9fe7, 0x9fe2],
        ),
        (Destination::Broadcast, [0xffe7, 0xffe2]),
    ] {
        let frames = intent.frames(target);
        assert_eq!(
            [frames[3], frames[5]],
            commands,
            "colour destination selected wrong address space"
        );
    }
}

#[test]
fn colour_queries_have_literal_opcodes_widths_and_reporting_order() {
    let address = ShortAddress::new(3).unwrap();
    for (query, word) in [
        (Query::GearFeatures, 0x07f7),
        (Query::Status, 0x07f8),
        (Query::TypeFeatures, 0x07f9),
        (Query::RgbwafControl, 0x07fb),
        (Query::ExtendedVersion, 0x07ff),
    ] {
        assert_eq!(
            query.plan(address),
            [Step::Send(0xc108), Step::Query(word)],
            "DT8 query opcode or enable changed"
        );
    }
    for (selector, raw, width) in [
        (ValueQuery::X, 0, Width::Word),
        (ValueQuery::Y, 1, Width::Word),
        (ValueQuery::Temperature, 2, Width::Word),
        (ValueQuery::Red, 9, Width::Byte),
        (ValueQuery::Green, 10, Width::Byte),
        (ValueQuery::Blue, 11, Width::Byte),
        (ValueQuery::White, 12, Width::Byte),
        (ValueQuery::Amber, 13, Width::Byte),
        (ValueQuery::Free, 14, Width::Byte),
        (ValueQuery::RgbwafControl, 15, Width::Byte),
        (ValueQuery::Coolest, 128, Width::Word),
        (ValueQuery::PhysicalCoolest, 129, Width::Word),
        (ValueQuery::Warmest, 130, Width::Word),
        (ValueQuery::PhysicalWarmest, 131, Width::Word),
    ] {
        assert_eq!(
            selector.width(),
            width,
            "colour selector response width changed"
        );
        let mut expected = vec![
            Step::Query(0x07a0),
            Step::Send(0xa300 | raw),
            Step::Send(0xc108),
            Step::Query(0x07fa),
        ];
        if width == Width::Word {
            expected.push(Step::Query(0x0798));
        }
        assert_eq!(
            selector.plan(address),
            expected,
            "colour value read order or low-byte query changed"
        );
    }
}

#[test]
fn colour_replies_preserve_transport_failures_and_full_word_mask() {
    use crate::atx::transactions::BusReply as B;
    assert_eq!(
        byte_reply(B::Value(255)),
        Reply::Value(255),
        "bitmap FF was mistaken for MASK"
    );
    assert_eq!(ByteValue::decode(255), ByteValue::Mask);
    assert_eq!(ByteValue::decode(0), ByteValue::Known(0));
    assert_eq!(
        word_reply(B::Value(255), B::Value(0)),
        Reply::Value(WordValue::Known(65280)),
        "high byte FF destroyed valid word data"
    );
    assert_eq!(
        word_reply(B::Value(255), B::Value(255)),
        Reply::Value(WordValue::Mask)
    );
    assert_eq!(
        word_reply(B::Value(1), B::Value(114)),
        Reply::Value(WordValue::Known(370)),
        "word byte order changed"
    );
    for (failure, expected) in [
        (B::NoResponse, Reply::NoResponse),
        (
            B::ReceiveCollision(Some(7)),
            Reply::ReceiveCollision(Some(7)),
        ),
        (B::TransmitCollision(None), Reply::TransmitCollision(None)),
    ] {
        assert_eq!(
            byte_reply(failure.clone()),
            expected,
            "failed query fabricated a byte"
        );
        assert_eq!(
            word_reply(failure.clone(), B::Value(0)),
            expected.map(|raw| WordValue::Known(u16::from(raw)))
        );
        assert_eq!(
            word_reply(B::Value(0), failure),
            expected.map(|raw| WordValue::Known(u16::from(raw))),
            "failed low byte fabricated colour data"
        );
    }
}

#[test]
fn colour_features_and_status_preserve_reserved_and_ambiguous_information() {
    let features = Features::decode(0xc3);
    assert!(features.xy() && features.temperature());
    assert_eq!(features.primary_count(), 0);
    assert_eq!(
        features.rgbwaf_channels(),
        6,
        "RGBWAF count lost high feature bits"
    );
    assert!(features.counts_valid());
    assert_eq!(Features::decode(0x98).primary_count(), 6);
    assert_eq!(Features::decode(0x98).rgbwaf_channels(), 4);
    let reserved = Features::decode(255);
    assert_eq!(reserved.raw(), 255);
    assert_eq!(reserved.primary_count(), 7);
    assert_eq!(reserved.rgbwaf_channels(), 7);
    assert!(
        !reserved.counts_valid(),
        "reserved seven-channel feature was advertised as valid"
    );
    for (raw, expected) in [
        (0, None),
        (0x10, Some(ColourType::Xy)),
        (0x20, Some(ColourType::Temperature)),
        (0x40, Some(ColourType::PrimaryN)),
        (0x80, Some(ColourType::Rgbwaf)),
        (0x30, None),
        (255, None),
    ] {
        assert_eq!(
            Status::decode(raw).active(),
            expected,
            "ambiguous active colour type was guessed"
        );
    }
    let status = Status::decode(0x8f);
    assert_eq!(status.raw(), 0x8f);
    assert!(status.xy_out_of_range() && status.temperature_out_of_range());
    assert!(status.calibration_running() && status.calibration_successful());
}
