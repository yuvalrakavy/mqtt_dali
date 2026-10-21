use super::*;
use crate::atx::transactions::BusReply;
use crate::atx::wire::{Command, DaliFrame};

#[test]
fn address_bounds_are_rejected_before_frame_creation() {
    assert!(ShortAddress::new(63).is_ok());
    for raw in [64, 127, 255] {
        assert!(
            ShortAddress::new(raw).is_err(),
            "invalid short address was accepted"
        );
    }
    assert!(GroupAddress::new(15).is_ok());
    for raw in [16, 63, 255] {
        assert!(
            GroupAddress::new(raw).is_err(),
            "invalid group address was accepted"
        );
    }
}

#[test]
fn output_frames_match_independent_short_group_and_broadcast_vectors() {
    let cases = [
        (
            Destination::Short(ShortAddress::new(0).unwrap()),
            0x00fe,
            0x0100,
            0x0105,
        ),
        (
            Destination::Short(ShortAddress::new(63).unwrap()),
            0x7efe,
            0x7f00,
            0x7f05,
        ),
        (
            Destination::Group(GroupAddress::new(0).unwrap()),
            0x80fe,
            0x8100,
            0x8105,
        ),
        (
            Destination::Group(GroupAddress::new(15).unwrap()),
            0x9efe,
            0x9f00,
            0x9f05,
        ),
        (Destination::Broadcast, 0xfefe, 0xff00, 0xff05),
    ];
    let maximum = Level::new(254).unwrap();
    for (destination, dapc, off, on) in cases {
        assert_eq!(
            destination.set_level(maximum),
            dapc,
            "DAPC address/control byte is wrong"
        );
        assert_eq!(destination.off(), off, "OFF address/control byte is wrong");
        assert_eq!(
            destination.recall_maximum(),
            on,
            "RECALL MAX address/control byte is wrong"
        );
    }
}

#[test]
fn normalized_maximum_is_254_and_mask_is_not_an_ordinary_level() {
    let maximum = Level::from_normalized(1.0).expect("normalized maximum was not representable");
    assert_eq!(maximum.raw(), 254, "maximum brightness selected DALI MASK");
    assert_eq!(maximum.normalized(), 1.0);
    assert!(
        Level::new(255).is_err(),
        "MASK was accepted as an ordinary level"
    );
    assert_eq!(Level::from_normalized(0.0).unwrap().raw(), 0);
    assert_eq!(Level::from_normalized(-0.0).unwrap().raw(), 0);
    assert_eq!(Level::from_normalized(0.5).unwrap().raw(), 127);
    assert_eq!(Level::from_normalized(0.25).unwrap().raw(), 64);
    assert_eq!(
        Level::from_normalized(f64::MIN_POSITIVE).unwrap().raw(),
        1,
        "positive sub-step brightness became OFF"
    );
}

#[test]
fn brightness_validation_and_all_raw_level_round_trips() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.0001] {
        assert!(
            Level::from_normalized(value).is_err(),
            "invalid normalized brightness was accepted"
        );
    }
    for raw in 0..=254 {
        let level = Level::new(raw).unwrap();
        assert_eq!(
            Level::from_normalized(level.normalized()).unwrap(),
            level,
            "raw level failed normalized round trip"
        );
    }
}

#[test]
fn atomic_power_and_brightness_intents_have_explicit_frame_policy() {
    let destination = Destination::Short(ShortAddress::new(2).unwrap());
    for (power, brightness, word) in [
        (Some(false), None, 0x0500),
        (Some(false), Some(0.0), 0x0500),
        (Some(true), None, 0x0505),
        (None, Some(0.0), 0x0400),
        (None, Some(0.5), 0x047f),
        (Some(true), Some(1.0), 0x04fe),
    ] {
        assert_eq!(
            Intent::new(power, brightness).unwrap().frame(destination),
            word,
            "power/brightness intent selected the wrong action"
        );
    }
}

#[test]
fn invalid_or_contradictory_intent_cannot_produce_a_partial_plan() {
    for (power, brightness) in [
        (None, None),
        (Some(false), Some(0.5)),
        (Some(true), Some(0.0)),
        (Some(true), Some(f64::NAN)),
        (Some(false), Some(1.1)),
    ] {
        assert!(
            Intent::new(power, brightness).is_err(),
            "invalid or contradictory output intent was accepted"
        );
    }
}

#[test]
fn query_opcodes_match_literal_short_address_vectors() {
    let address = ShortAddress::new(3).unwrap();
    for (query, word) in [
        (Query::Status, 0x0790),
        (Query::Version, 0x0797),
        (Query::DeviceType, 0x0799),
        (Query::PhysicalMinimum, 0x079a),
        (Query::ActualLevel, 0x07a0),
        (Query::MaximumLevel, 0x07a1),
        (Query::MinimumLevel, 0x07a2),
    ] {
        assert_eq!(address.query(query), word, "query opcode/address is wrong");
    }
}

#[test]
fn query_replies_preserve_each_query_kind_and_level_values() {
    let level = LevelValue::Known(Level::new(42).unwrap());
    for (query, expected) in [
        (Query::ActualLevel, QueryValue::ActualLevel(level)),
        (Query::MaximumLevel, QueryValue::MaximumLevel(level)),
        (Query::MinimumLevel, QueryValue::MinimumLevel(level)),
        (Query::PhysicalMinimum, QueryValue::PhysicalMinimum(level)),
        (Query::Version, QueryValue::Version(42)),
        (
            Query::DeviceType,
            QueryValue::DeviceType(DeviceType::Specific(42)),
        ),
    ] {
        assert_eq!(
            query.decode(BusReply::Value(42)),
            QueryReply::Value(expected),
            "reply lost its query-specific meaning"
        );
    }
}

#[test]
fn mask_type_and_status_sentinels_are_specific_to_the_query() {
    for (query, expected) in [
        (
            Query::ActualLevel,
            QueryValue::ActualLevel(LevelValue::UnavailableMask),
        ),
        (
            Query::MaximumLevel,
            QueryValue::MaximumLevel(LevelValue::UnavailableMask),
        ),
        (
            Query::MinimumLevel,
            QueryValue::MinimumLevel(LevelValue::UnavailableMask),
        ),
        (
            Query::PhysicalMinimum,
            QueryValue::PhysicalMinimum(LevelValue::UnavailableMask),
        ),
        (
            Query::DeviceType,
            QueryValue::DeviceType(DeviceType::Multiple),
        ),
        (Query::Version, QueryValue::Version(255)),
    ] {
        assert_eq!(
            query.decode(BusReply::Value(255)),
            QueryReply::Value(expected),
            "query-specific MASK or device type was decoded as ordinary brightness"
        );
    }
    assert_eq!(
        Query::DeviceType.decode(BusReply::Value(254)),
        QueryReply::Value(QueryValue::DeviceType(DeviceType::None))
    );
    assert_eq!(
        Query::DeviceType.decode(BusReply::Value(0)),
        QueryReply::Value(QueryValue::DeviceType(DeviceType::Specific(0)))
    );
    assert_eq!(
        Query::ActualLevel.decode(BusReply::Value(0)),
        QueryReply::Value(QueryValue::ActualLevel(LevelValue::Known(
            Level::new(0).unwrap()
        ))),
        "a genuine zero level was confused with no response"
    );
    assert_eq!(
        Query::ActualLevel.decode(BusReply::Value(254)),
        QueryReply::Value(QueryValue::ActualLevel(LevelValue::Known(
            Level::new(254).unwrap()
        )))
    );
    let QueryReply::Value(QueryValue::Status(status)) = Query::Status.decode(BusReply::Value(255))
    else {
        panic!("status 255 was incorrectly treated as a level MASK");
    };
    assert_eq!(status.raw(), 255);
}

#[test]
fn no_response_and_collisions_never_become_an_off_state() {
    let queries = [
        Query::Status,
        Query::ActualLevel,
        Query::MinimumLevel,
        Query::MaximumLevel,
        Query::PhysicalMinimum,
        Query::Version,
        Query::DeviceType,
    ];
    for query in queries {
        for (reply, expected) in [
            (BusReply::NoResponse, QueryReply::NoResponse),
            (
                BusReply::ReceiveCollision(None),
                QueryReply::ReceiveCollision(None),
            ),
            (
                BusReply::ReceiveCollision(Some(0x91)),
                QueryReply::ReceiveCollision(Some(0x91)),
            ),
            (
                BusReply::TransmitCollision(None),
                QueryReply::TransmitCollision(None),
            ),
            (
                BusReply::TransmitCollision(Some(0xa0)),
                QueryReply::TransmitCollision(Some(0xa0)),
            ),
        ] {
            assert_eq!(
                query.decode(reply),
                expected,
                "transport failure became a successful fixture value"
            );
        }
    }
}

#[test]
fn status_bits_preserve_raw_fault_power_and_fade_running_semantics() {
    for bit in 0..8 {
        let raw = 1 << bit;
        let QueryReply::Value(QueryValue::Status(status)) =
            Query::Status.decode(BusReply::Value(raw))
        else {
            panic!("status byte lost its type");
        };
        assert_eq!(status.raw(), raw);
        let flags = [
            status.gear_failure(),
            status.lamp_failure(),
            status.lamp_on(),
            status.limit_error(),
            status.fade_running(),
            status.reset_state(),
            status.missing_short_address(),
            status.power_failure(),
        ];
        for (index, set) in flags.into_iter().enumerate() {
            assert_eq!(
                set,
                index == bit,
                "status bit meaning is inverted or shifted"
            );
        }
    }
}

#[test]
fn typed_output_frame_uses_the_production_atx_encoder() {
    let frame = Destination::Group(GroupAddress::new(15).unwrap())
        .set_level(Level::from_normalized(1.0).unwrap());
    let command = Command::Send {
        bus: 2,
        frame: DaliFrame::Bits16(frame),
        repeat: false,
    };
    assert_eq!(
        command.encode().unwrap(),
        b"2h9EFE\n",
        "output frame and ATX UART encoding disagree with the literal transcript"
    );
}
