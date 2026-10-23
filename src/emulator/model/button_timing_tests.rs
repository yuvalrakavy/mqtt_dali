//! Literal Part 301 checks; no production input codec or setting plans.
use super::*;
use serde_json::json;

fn physical() -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[
            {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":99}],
             "inputs":[{"id":"panel","short_address":0,"instances":[1,2,3,4,31,1],
              "values":[{"instance":0,"resolution":10,"bytes":[171,192]},
                        {"instance":1,"resolution":1,"bytes":[255]}]},
             {"id":"last","short_address":63,"instances":vec![1;32]},
             {"id":"new","short_address":null,"instances":[1]}]},
            {"number":1,"inputs":[{"id":"other-bus","short_address":0,"instances":[1]}]}
        ]}))
        .unwrap(),
    )
    .unwrap()
}
fn pair(m: &mut Model, frame: u32) {
    assert_eq!(m.transmit_input(0, frame, 0), Reply::None);
    assert_eq!(m.transmit_input(0, frame, 0), Reply::None);
}
fn set(m: &mut Model, instance: u8, opcode: u8, value: u8) {
    m.transmit_input(0, 0xc13000 | u32::from(value), 0);
    pair(m, 0x010000 | (u32::from(instance) << 8) | u32::from(opcode));
}

#[test]
fn literal_button_timing_all_settings_queries_and_single_send_refusal() {
    for (opcode, query, initial, value) in [
        (0, 0x0a, 25, 255),
        (1, 0x0c, 0, 100),
        (2, 0x0e, 8, 5),
        (3, 0x0f, 20, 255),
    ] {
        let mut m = physical();
        assert_eq!(
            m.transmit_input(0, 0x010000 | query, 0),
            Reply::Value(initial),
            "button timer query is missing"
        );
        m.transmit_input(0, 0xc13000 | value, 0);
        m.transmit_input(0, 0x010000 | opcode, 0);
        assert_eq!(
            m.transmit_input(0, 0x010000 | query, 0),
            Reply::Value(initial),
            "single button timer write applied"
        );
        pair(&mut m, 0x010000 | opcode);
        assert_eq!(
            m.transmit_input(0, 0x010000 | query, 0),
            Reply::Value(value as u8),
            "repeated button timer write did not apply"
        );
    }
    let mut m = physical();
    assert_eq!(m.transmit_input(0, 0x01000b, 0), Reply::Value(25));
    assert_eq!(m.transmit_input(0, 0x01000d, 0), Reply::Value(10));
    set(&mut m, 0, 1, 50);
    set(&mut m, 0, 1, 0);
    assert_eq!(
        m.transmit_input(0, 0x01000c, 0),
        Reply::Value(0),
        "double timer zero did not disable timing"
    );
}

#[test]
fn literal_button_timing_invalid_profile_values_and_other_types_are_inert() {
    let mut m = physical();
    for (opcode, query, initial, bad) in [
        (0, 0x0a, 25, 0),
        (0, 0x0a, 25, 24),
        (1, 0x0c, 0, 1),
        (1, 0x0c, 0, 101),
        (1, 0x0c, 0, 255),
        (2, 0x0e, 8, 4),
        (2, 0x0e, 8, 101),
        (3, 0x0f, 20, 4),
    ] {
        set(&mut m, 0, opcode, bad);
        assert_eq!(
            m.transmit_input(0, 0x010000 | query, 0),
            Reply::Value(initial),
            "invalid emulator timer profile value applied"
        );
    }
    for instance in 1..=4 {
        for opcode in 0..=3 {
            set(&mut m, instance, opcode, 50);
        }
        for query in 0x0a..=0x0f {
            assert_eq!(
                m.transmit_input(0, 0x010000 | (u32::from(instance) << 8) | query, 0),
                Reply::None,
                "non-button instance claimed a button timer"
            );
        }
    }
    let snapshot = m.snapshot();
    for instance in 1..=4 {
        assert!(snapshot["buses"][0]["inputs"][0]["settings"][instance]["button_timers"].is_null());
    }
    assert_eq!(m.transmit_input(0, 0x01018c, 0), Reply::Value(255));
}

#[test]
fn literal_button_timing_repeat_window_and_input_output_interruption() {
    for (gap, interrupt, expected) in [(100, 0, 99), (101, 0, 25), (1, 1, 25), (1, 2, 25)] {
        let mut m = physical();
        m.transmit_input(0, 0xc13063, 0);
        m.transmit_input(0, 0x010000, 1);
        match interrupt {
            1 => {
                m.transmit_input(0, 0x010580, 1);
            }
            2 => {
                m.transmit(0, 0x01a0, 1);
            }
            _ => {}
        }
        m.transmit_input(0, 0x010000, 1 + gap);
        assert_eq!(
            m.transmit_input(0, 0x01000a, 1 + gap),
            Reply::Value(expected),
            "button timer repeat boundary or interruption violated"
        );
    }
}

#[test]
fn literal_button_timing_last_instance_scope_latch_and_address_retention() {
    let mut m = physical();
    let before = m.snapshot();
    m.transmit_input(0, 0xc130ff, 0);
    pair(&mut m, 0x7f1f00);
    assert_eq!(
        m.transmit_input(0, 0x7f1f0a, 0),
        Reply::Value(255),
        "last button instance timer write missing"
    );
    assert_eq!(
        m.transmit_input(0, 0x7f000a, 0),
        Reply::Value(25),
        "last button instance write changed instance zero"
    );
    assert_eq!(m.transmit_input(1, 0x01000a, 0), Reply::Value(25));
    assert_eq!(m.transmit_input(0, 0x01008c, 0), Reply::Value(171));
    m.set_input_value("panel", 0, vec![18, 52]).unwrap();
    set(&mut m, 0, 0, 75);
    assert_eq!(
        m.transmit_input(0, 0x01008d, 0),
        Reply::Value(192),
        "timer write changed captured value latch"
    );
    m.set_address("panel", Some(17)).unwrap();
    assert_eq!(m.transmit_input(0, 0x01000a, 0), Reply::None);
    assert_eq!(m.transmit_input(0, 0x23000a, 0), Reply::Value(75));
    let after = m.snapshot();
    assert_eq!(after["buses"][0]["outputs"], before["buses"][0]["outputs"]);
    assert_eq!(after["buses"][1], before["buses"][1]);
    for key in ["enabled", "priority", "scheme", "filter", "groups"] {
        assert_eq!(
            after["buses"][0]["inputs"][0]["settings"][0][key],
            before["buses"][0]["inputs"][0]["settings"][0][key]
        );
    }
    assert_eq!(
        after["buses"][0]["inputs"][0]["settings"][5],
        before["buses"][0]["inputs"][0]["settings"][5]
    );
    // Broadcast-to-unaddressed does not touch addressed modules.
    m.transmit_input(0, 0xc13028, 0);
    pair(&mut m, 0xfd0000);
    assert_eq!(m.transmit_input(0, 0xfd000a, 0), Reply::Value(40));
    assert_eq!(m.transmit_input(0, 0x23000a, 0), Reply::Value(75));
    // Full broadcast affects only the selected bus/instance, including unaddressed.
    m.transmit_input(0, 0xc13032, 0);
    pair(&mut m, 0xff0000);
    assert_eq!(m.transmit_input(0, 0xff000a, 0), Reply::Value(50));
    assert_eq!(m.transmit_input(0, 0x23050a, 0), Reply::Value(25));
    assert_eq!(m.transmit_input(1, 0x01000a, 0), Reply::Value(25));
}
