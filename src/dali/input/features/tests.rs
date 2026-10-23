use super::*;
use crate::dali::input::{
    decode as event,
    feedback::{Action, Capabilities, Query as FeedbackQuery},
};
#[test]
fn feature_feedback_literal_frames_keep_selector_namespaces_distinct() {
    for (address, instance, prefix) in [(0, 0, 0x010000), (63, 31, 0x7f1f00)] {
        let a = Address::new(address).unwrap();
        let i = Instance::new(instance).unwrap();
        let scope = Scope::Instance(i);
        assert_eq!(scope.query(a, Query::Type), prefix | 0x8e);
        assert_eq!(scope.query(a, Query::Next), prefix | 0x8f);
        assert_eq!(
            Scope::Device.query(a, Query::Type),
            (prefix & 0xff0000) | 0xfe8e
        );
        assert_eq!(
            Scope::Device.query(a, Query::Next),
            (prefix & 0xff0000) | 0xfe8f
        );
        for (scope, selector) in [(scope, 0x20 | instance), (Scope::Device, 0xfc)] {
            let base = (prefix & 0xff0000) | (u32::from(selector) << 8);
            for (frame, opcode) in [
                (Action::Activate.frame(a, scope), 0x10),
                (Action::Stop.frame(a, scope), 0x11),
                (FeedbackQuery::Capability.frame(a, scope), 0x4f),
                (FeedbackQuery::Active.frame(a, scope), 0x4e),
            ] {
                assert_eq!(
                    frame,
                    base | opcode,
                    "feedback frame selector/opcode changed"
                );
                assert_eq!(
                    event(frame).unwrap(),
                    None,
                    "feedback command decoded as an input event"
                );
            }
        }
    }
}
#[test]
fn feature_enumeration_distinguishes_fe_empty_end_from_ff_multiple() {
    assert_eq!(
        decode(254, &[]).unwrap(),
        vec![],
        "feature FE must be a complete empty list"
    );
    for raw in 32..=96 {
        assert_eq!(
            decode(raw, &[]).unwrap(),
            vec![FeatureType::new(raw).unwrap()]
        );
    }
    assert_eq!(FeatureType::FEEDBACK.raw(), 32);
    assert_eq!(
        decode(255, &[32, 33, 96, 254])
            .unwrap()
            .iter()
            .map(|v| v.raw())
            .collect::<Vec<_>>(),
        vec![32, 33, 96],
        "multiple feature FF lost its enumerated list"
    );
    let all = (32..=96).chain([254]).collect::<Vec<_>>();
    assert_eq!(
        decode(255, &all).unwrap().len(),
        65,
        "maximum feature list lost its terminating proof"
    );
    for raw in 0..=255 {
        assert_eq!(FeatureType::new(raw).is_ok(), (32..=96).contains(&raw));
    }
    for (first, rest) in [
        (255, vec![]),
        (255, vec![254]),
        (255, vec![32, 254]),
        (255, vec![32, 33]),
        (255, vec![32, 33, 96]),
        (255, vec![32, 32, 254]),
        (255, vec![33, 32, 254]),
        (255, vec![31, 32, 254]),
        (255, vec![32, 97, 254]),
        (255, vec![32, 255, 254]),
        (255, vec![32, 33, 254, 96]),
        (254, vec![32]),
        (32, vec![33, 254]),
        (0, vec![]),
        (253, vec![]),
    ] {
        assert!(
            decode(first, &rest).is_err(),
            "invalid feature enumeration accepted: {first}/{rest:?}"
        );
    }
    let too_long = (32..=96).chain([96, 254]).collect::<Vec<_>>();
    assert!(decode(255, &too_long).is_err());
}
#[test]
fn feedback_capability_flags_preserve_every_raw_and_unknown_bit() {
    for raw in 0..=255 {
        let v = Capabilities::from_raw(raw);
        assert_eq!(v.raw(), raw);
        assert_eq!(
            [
                v.visible(),
                v.brightness(),
                v.colour(),
                v.audible(),
                v.volume(),
                v.pitch()
            ],
            [1, 2, 4, 8, 16, 32].map(|mask| raw & mask != 0)
        );
        assert_eq!(
            v.unknown_bits(),
            raw & 0xc0,
            "unknown feedback capability bits were lost"
        );
    }
}
