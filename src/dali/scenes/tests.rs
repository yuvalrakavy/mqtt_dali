use super::*;
use crate::dali::gear::GroupAddress;
fn address(raw: u8) -> ShortAddress {
    ShortAddress::new(raw).unwrap()
}

#[test]
fn dt7_scene_capture_literal_plan_preserves_address_and_repeat() {
    for (raw, store, query) in [(0, 0x0121, 0x0198), (63, 0x7f21, 0x7f98)] {
        assert_eq!(
            capture_level(address(raw)),
            [Step::Send(0xa3ff), Step::Repeat(store), Step::Query(query)],
            "DT7 virtual capture plan lost poison, repeat or individual address"
        );
    }
}

#[test]
fn scene_literal_slot_level_and_repeat_contract() {
    let first = Slot::new(0).unwrap();
    let last = Slot::new(15).unwrap();
    assert!(Slot::new(16).is_err());
    assert!(Slot::new(255).is_err());
    assert_eq!(last.raw(), 15);
    assert_eq!(first.recall(Destination::Short(address(0))), 0x0110);
    assert_eq!(last.recall(Destination::Short(address(63))), 0x7f1f);
    assert_eq!(
        first.recall(Destination::Group(GroupAddress::new(0).unwrap())),
        0x8110
    );
    assert_eq!(
        last.recall(Destination::Group(GroupAddress::new(15).unwrap())),
        0x9f1f
    );
    assert_eq!(last.recall(Destination::Broadcast), 0xff1f);
    assert_eq!(
        (first.query(address(0)), last.query(address(63))),
        (0x01b0, 0x7fbf)
    );
    assert_eq!(
        first.store(address(0), Level::decode(0)),
        [Step::Send(0xa300), Step::Repeat(0x0140)],
        "scene Store lost Off or repeated-setting semantics"
    );
    assert_eq!(
        last.store(address(63), Level::Mask),
        [Step::Send(0xa3ff), Step::Repeat(0x7f4f)]
    );
    assert_eq!(last.remove(address(63)), Step::Repeat(0x7f5f));
    for raw in 0..=255 {
        assert_eq!(Level::decode(raw).raw(), raw);
    }
    assert_ne!(Level::decode(0), Level::Mask);
}

#[test]
fn scene_report_plans_select_stored_scene_without_refreshing_actual() {
    let slot = Slot::new(15).unwrap();
    assert_eq!(
        ReportValue::Temperature.plan(address(3), slot),
        [
            Step::Query(0x07bf),
            Step::Send(0xa3e2),
            Step::Send(0xc108),
            Step::Query(0x07fa),
            Step::Query(0x0798)
        ],
        "scene report plan refreshed actual values or lost word LSB"
    );
    assert_eq!(
        ReportValue::Type.plan(address(63), Slot::new(0).unwrap()),
        [
            Step::Query(0x7fb0),
            Step::Send(0xa3f0),
            Step::Send(0xc108),
            Step::Query(0x7ffa)
        ]
    );
    for (item, selector, width) in [
        (ReportValue::X, 224, Width::Word),
        (ReportValue::Y, 225, Width::Word),
        (ReportValue::Temperature, 226, Width::Word),
        (ReportValue::Red, 233, Width::Byte),
        (ReportValue::Green, 234, Width::Byte),
        (ReportValue::Blue, 235, Width::Byte),
        (ReportValue::White, 236, Width::Byte),
        (ReportValue::Amber, 237, Width::Byte),
        (ReportValue::Free, 238, Width::Byte),
        (ReportValue::RgbwafControl, 239, Width::Byte),
        (ReportValue::Type, 240, Width::Byte),
    ] {
        assert_eq!((item.selector(), item.width()), (selector, width));
    }
    assert_eq!(copy_report_to_temporary(address(3)), [0xc108, 0x07ee]);
}

#[test]
fn scene_masks_unsupported_types_and_missing_replies_remain_distinct() {
    assert_eq!(level_reply(BusReply::Value(255)), Reply::Value(Level::Mask));
    assert_eq!(
        level_reply(BusReply::NoResponse),
        Reply::NoResponse,
        "missing scene level became MASK or Off"
    );
    assert_eq!(
        type_reply(BusReply::Value(255)),
        Reply::Value(ReportType::Mask)
    );
    assert_eq!(
        type_reply(BusReply::Value(0x40)),
        Reply::Value(ReportType::Known(ColourType::PrimaryN))
    );
    assert_eq!(
        type_reply(BusReply::Value(0x30)),
        Reply::Value(ReportType::Unsupported(0x30))
    );
    assert_eq!(
        type_reply(BusReply::ReceiveCollision(Some(2))),
        Reply::ReceiveCollision(Some(2))
    );
    assert_eq!(
        type_reply(BusReply::TransmitCollision(None)),
        Reply::TransmitCollision(None)
    );
    assert_eq!(
        colour::word_reply(BusReply::Value(255), BusReply::Value(254)),
        Reply::Value(colour::WordValue::Known(65534))
    );
    assert_eq!(
        colour::word_reply(BusReply::Value(255), BusReply::Value(255)),
        Reply::Value(colour::WordValue::Mask)
    );
    assert_eq!(
        colour::word_reply(BusReply::Value(1), BusReply::NoResponse),
        Reply::NoResponse
    );
}

#[test]
fn scene_live_intents_can_stage_without_changing_existing_activation_vectors() {
    let target = Destination::Short(address(3));
    for (intent, expected) in [
        (
            colour::Intent::temperature(Temperature::new(350).unwrap()),
            vec![0xa35e, 0xc301, 0xc108, 0x07e7],
        ),
        (
            colour::Intent::xy_parts(None, Some(Coordinate::new(0x1234).unwrap())).unwrap(),
            vec![
                0xa3ff, 0xc3ff, 0xc108, 0x07e0, 0xa334, 0xc312, 0xc108, 0x07e1,
            ],
        ),
        (
            colour::Intent::rgbwaf([Some(Channel::new(0).unwrap()), None, None, None, None, None])
                .unwrap(),
            vec![
                0xa300, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07ec,
            ],
        ),
    ] {
        assert_eq!(
            intent.staged_frames(target),
            expected,
            "staging live intent activated or changed values"
        );
        let mut active = expected;
        active.extend([0xc108, 0x07e2]);
        assert_eq!(intent.frames(target), active);
    }
    assert!(colour::Intent::rgbwaf([None; 6]).is_err());
    assert!(colour::Intent::xy_parts(None, None).is_err());
}

#[test]
fn scene_stored_colours_allow_masked_attributes_and_explicit_rgb_control() {
    assert_eq!(
        Colour::Temperature(None).staged_frames(address(3)),
        [0xa3ff, 0xc3ff, 0xc108, 0x07e7]
    );
    assert_eq!(
        Colour::Xy { x: None, y: None }.staged_frames(address(3)),
        [0xa3ff, 0xc3ff, 0xc108, 0x07e0, 0xa3ff, 0xc3ff, 0xc108, 0x07e1]
    );
    assert_eq!(
        Colour::Rgbwaf {
            channels: [None; 6],
            control: Some(RgbwafControl::new(0x41).unwrap())
        }
        .staged_frames(address(3)),
        [
            0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07ec, 0xa341,
            0xc108, 0x07ed
        ],
        "stored RGB staging lost masks or control"
    );
    let masked = Colour::Rgbwaf {
        channels: [None; 6],
        control: None,
    }
    .staged_frames(address(3));
    assert_eq!(&masked[10..], [0xa3ff, 0xc108, 0x07ed]);
    for raw in 0..=191 {
        assert_eq!(RgbwafControl::new(raw).unwrap().raw(), raw);
    }
    for raw in 192..=255 {
        assert!(RgbwafControl::new(raw).is_err());
    }
}
