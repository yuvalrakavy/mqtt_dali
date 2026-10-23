//! Independent literal Part 103 settings, never production codec frames.
use super::*;
use serde_json::json;

fn physical() -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[
            {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":99}],
             "inputs":[{"id":"panel","short_address":0,"instances":[1,2,3,4,31],
         "values":[{"instance":0,"resolution":1,"bytes":[255]},
                   {"instance":1,"resolution":10,"bytes":[171,192]}]},
             {"id":"peer","short_address":63,"instances":[1]},
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

#[test]
fn literal_input_settings_every_write_requires_adjacent_repeat() {
    for opcode in 0x61..=0x68 {
        let mut m = physical();
        if opcode == 0x62 {
            pair(&mut m, 0x010063);
        }
        m.transmit_input(0, 0xc13000 | if opcode == 0x61 { 2 } else { 0 }, 0);
        let before = m.snapshot()["buses"][0]["inputs"][0]["settings"].clone();
        assert_eq!(m.transmit_input(0, 0x010000 | opcode, 0), Reply::None);
        assert_eq!(
            m.snapshot()["buses"][0]["inputs"][0]["settings"],
            before,
            "single instance setting applied"
        );
        assert_eq!(m.transmit_input(0, 0x010000 | opcode, 100), Reply::None);
        assert_ne!(
            m.snapshot()["buses"][0]["inputs"][0]["settings"],
            before,
            "repeated instance setting did not apply"
        );
    }
}

#[test]
fn literal_input_settings_read_write_scope_and_value_latch() {
    let mut m = physical();
    assert_eq!(
        m.transmit_input(0, 0x010084, 0),
        Reply::Value(4),
        "input priority query missing"
    );
    assert_eq!(m.transmit_input(0, 0x010086, 0), Reply::Value(255));
    assert_eq!(m.transmit_input(0, 0x01018b, 0), Reply::Value(2));
    for (instance, mask) in [(0, 255), (1, 1), (2, 31), (3, 1), (4, 0)] {
        assert_eq!(
            m.transmit_input(0, 0x010090 | (instance << 8), 0),
            Reply::Value(mask)
        );
    }
    // The pending full-value latch must survive unrelated settings writes.
    assert_eq!(m.transmit_input(0, 0x01018c, 0), Reply::Value(171));
    m.set_input_value("panel", 1, vec![18, 52]).unwrap();
    m.transmit_input(0, 0xc13002, 0);
    pair(&mut m, 0x010061);
    m.transmit_input(0, 0xc13004, 0);
    pair(&mut m, 0x010067);
    for (dtr, set, query) in [(31, 0x64, 0x88), (0, 0x65, 0x89), (7, 0x66, 0x8a)] {
        m.transmit_input(0, 0xc13000 | dtr, 0);
        pair(&mut m, 0x010000 | set);
        assert_eq!(
            m.transmit_input(0, 0x010000 | query, 0),
            Reply::Value(dtr as u8)
        );
    }
    pair(&mut m, 0x010063);
    assert_eq!(
        m.transmit_input(0, 0x010086, 0),
        Reply::None,
        "disabled input still answers YES"
    );
    assert_eq!(
        m.transmit_input(0, 0x010080, 0),
        Reply::Value(1),
        "disabled instance lost type presence"
    );
    assert_eq!(m.transmit_input(0, 0x010084, 0), Reply::Value(2));
    assert_eq!(m.transmit_input(0, 0x01008b, 0), Reply::Value(4));
    assert_eq!(
        m.transmit_input(0, 0x01018d, 0),
        Reply::Value(192),
        "settings write reset a captured value latch"
    );
    assert_eq!(m.transmit_input(0, 0x01008c, 0), Reply::Value(255));
    pair(&mut m, 0x010062);
    assert_eq!(m.transmit_input(0, 0x010086, 0), Reply::Value(255));
    // Unassign a group using FF; it is not group31.
    m.transmit_input(0, 0xc130ff, 0);
    pair(&mut m, 0x010064);
    assert_eq!(m.transmit_input(0, 0x010088, 0), Reply::Value(255));
    let s = m.snapshot();
    assert_eq!(
        s["buses"][0]["inputs"][0]["settings"][1]["priority"], 4,
        "setting changed sibling instance"
    );
    assert_eq!(
        s["buses"][0]["inputs"][1]["settings"][0]["priority"], 4,
        "setting changed sibling module"
    );
    assert_eq!(
        s["buses"][1]["inputs"][0]["settings"][0]["priority"], 4,
        "setting changed another bus"
    );
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(99));
}

#[test]
fn literal_input_settings_filters_keep_24bits_and_common_reserved_bits() {
    let mut m = physical();
    for f in [0xc130ef, 0xc131cd, 0xc132ab] {
        m.transmit_input(0, f, 0);
    }
    pair(&mut m, 0x010468); // unknown type31: preserve all raw bits
    for (q, v) in [(0x90, 0xef), (0x91, 0xcd), (0x92, 0xab)] {
        assert_eq!(
            m.transmit_input(0, 0x010400 | q, 0),
            Reply::Value(v),
            "24bit event filter was truncated"
        );
    }
    for (instance, expected) in [(0, 0xef), (1, 1), (2, 15), (3, 1)] {
        pair(&mut m, 0x010068 | (instance << 8));
        assert_eq!(
            m.transmit_input(0, 0x010090 | (instance << 8), 0),
            Reply::Value(expected),
            "common filter kept reserved bits"
        );
        assert_eq!(
            m.transmit_input(0, 0x010091 | (instance << 8), 0),
            Reply::Value(0)
        );
    }
    for f in [0xc130ff, 0xc131ff, 0xc132ff] {
        m.transmit_input(0, f, 0);
    }
    pair(&mut m, 0x010468);
    for q in [0x90, 0x91, 0x92] {
        assert_eq!(
            m.transmit_input(0, 0x010400 | q, 0),
            Reply::Value(255),
            "FF filter byte became MASK"
        );
    }
}

#[test]
fn literal_input_settings_repeat_window_interruption_and_invalid_dtr() {
    for (gap, interruption, changed) in [
        (100, 0, true),
        (101, 0, false),
        (1, 1, false),
        (1, 2, false),
    ] {
        let mut m = physical();
        m.transmit_input(0, 0x010063, 0);
        assert_eq!(
            m.transmit_input(0, 0x010086, 0),
            Reply::Value(255),
            "single instance setting applied"
        );
        // Start a fresh pair after the query above broke the first one.
        m.transmit_input(0, 0x010063, 1);
        match interruption {
            1 => {
                m.transmit_input(0, 0x010180, 1);
            }
            2 => {
                m.transmit(0, 0x01a0, 1);
            }
            _ => {}
        }
        m.transmit_input(0, 0x010063, 1 + gap);
        assert_eq!(
            m.transmit_input(0, 0x010086, 1 + gap),
            if changed {
                Reply::None
            } else {
                Reply::Value(255)
            },
            "input setting repeat window/interruption violated"
        );
    }
    for (opcode, invalid) in [
        (0x61, 1),
        (0x61, 6),
        (0x61, 255),
        (0x67, 5),
        (0x67, 255),
        (0x64, 32),
        (0x65, 254),
        (0x66, 32),
    ] {
        let mut m = physical();
        m.transmit_input(0, 0xc13000 | invalid, 0);
        let before = m.snapshot()["buses"][0]["inputs"][0]["settings"].clone();
        pair(&mut m, 0x010000 | opcode);
        assert_eq!(
            m.snapshot()["buses"][0]["inputs"][0]["settings"],
            before,
            "invalid input setting was accepted"
        );
    }
}

#[test]
fn literal_input_settings_broadcast_selection_missing_instances_and_address_preservation() {
    let mut m = physical();
    // FD reaches only unaddressed modules, FF reaches all on this bus.
    pair(&mut m, 0xfd0063);
    assert_eq!(m.transmit_input(0, 0xfd0080, 0), Reply::Value(1));
    assert_eq!(m.transmit_input(0, 0xfd0086, 0), Reply::None);
    assert_eq!(m.transmit_input(0, 0x010086, 0), Reply::Value(255));
    pair(&mut m, 0xff0063);
    assert_eq!(m.transmit_input(0, 0x7f0086, 0), Reply::None);
    assert_eq!(m.transmit_input(1, 0x010086, 0), Reply::Value(255));
    let before = m.snapshot()["buses"][0]["inputs"][0]["settings"].clone();
    pair(&mut m, 0x011f62); // absent instance31 on this particular module
    assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["settings"], before);
    m.set_address("panel", Some(7)).unwrap();
    assert_eq!(
        m.snapshot()["buses"][0]["inputs"][0]["settings"],
        before,
        "readdressing reset input settings"
    );
    assert_eq!(m.transmit_input(0, 0x0f0086, 0), Reply::None);
    assert_eq!(m.transmit_input(0, 0x010086, 0), Reply::None); // missing module
}

#[test]
fn literal_input_settings_last_address_last_instance_and_each_scheme() {
    let mut m = Model::new(
        serde_json::from_value(json!({"version":1,"buses":[
            {"number":0,"inputs":[{"id":"last","short_address":63,"instances":vec![1;32]}]}
        ]}))
        .unwrap(),
    )
    .unwrap();
    for raw in 0..=4 {
        m.transmit_input(0, 0xc13000 | raw, 0);
        pair(&mut m, 0x7f1f67);
        assert_eq!(m.transmit_input(0, 0x7f1f8b, 0), Reply::Value(raw as u8));
        assert_eq!(
            m.transmit_input(0, 0x7f008b, 0),
            Reply::Value(2),
            "last instance write changed instance zero"
        );
    }
}
