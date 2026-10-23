//! Literal independent sensor settings; never production codec plans.
use super::*;
use serde_json::json;

fn physical() -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[
            {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":99}],
             "inputs":[{"id":"panel","short_address":0,"instances":[2,3,4,1,31],
              "values":[{"instance":0,"resolution":16,"bytes":[171,205]}]},
              {"id":"last","short_address":63,"instances":vec![4;32]},
              {"id":"new","short_address":null,"instances":[2,3,4]}]},
            {"number":1,"inputs":[{"id":"other","short_address":0,"instances":[2,3,4]}]}
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
fn literal_sensor_settings_all_nine_bindings_require_repeat_and_keep_ff() {
    for (instance, set, query, initial, value) in [
        (0, 0x10, 0x1e, 0, 255),
        (0, 0x11, 0x1d, 2, 255),
        (1, 0x21, 0x2d, 90, 254),
        (1, 0x22, 0x2e, 20, 255),
        (1, 0x23, 0x2c, 2, 255),
        (2, 0x30, 0x3e, 30, 255),
        (2, 0x31, 0x3f, 5, 25),
        (2, 0x32, 0x3d, 30, 255),
        (2, 0x33, 0x3c, 0, 255),
    ] {
        let mut m = physical();
        let prefix = 0x010000 | (instance << 8);
        assert_eq!(
            m.transmit_input(0, prefix | query, 0),
            Reply::Value(initial),
            "sensor parameter query is missing"
        );
        m.transmit_input(0, 0xc13000 | value, 0);
        m.transmit_input(0, prefix | set, 0);
        assert_eq!(
            m.transmit_input(0, prefix | query, 0),
            Reply::Value(initial),
            "single sensor setting write applied"
        );
        pair(&mut m, prefix | set);
        assert_eq!(
            m.transmit_input(0, prefix | query, 0),
            Reply::Value(value as u8),
            "repeated sensor setting or FF data was lost"
        );
    }
}
#[test]
fn literal_sensor_settings_zero_invalid_hold_hysteresis_and_wrong_types() {
    let mut m = physical();
    for (instance, opcode, query) in [
        (0, 0x10, 0x1e),
        (0, 0x11, 0x1d),
        (1, 0x21, 0x2d),
        (1, 0x22, 0x2e),
        (1, 0x23, 0x2c),
        (2, 0x30, 0x3e),
        (2, 0x31, 0x3f),
        (2, 0x32, 0x3d),
        (2, 0x33, 0x3c),
    ] {
        set(&mut m, instance, opcode, 0);
        assert_eq!(
            m.transmit_input(0, 0x010000 | (u32::from(instance) << 8) | query, 0),
            Reply::Value(0)
        );
    }
    set(&mut m, 1, 0x21, 255);
    assert_eq!(
        m.transmit_input(0, 0x01012d, 0),
        Reply::Value(0),
        "MASK hold write was applied"
    );
    for value in [26, 255] {
        set(&mut m, 2, 0x31, value);
    }
    assert_eq!(
        m.transmit_input(0, 0x01023f, 0),
        Reply::Value(0),
        "invalid sensor hysteresis applied"
    );
    for instance in [0, 3, 4] {
        set(&mut m, instance, 0x21, 50);
        assert_eq!(
            m.transmit_input(0, 0x010000 | (u32::from(instance) << 8) | 0x2d, 0),
            Reply::None,
            "wrong instance type claimed occupancy hold"
        );
    }
    assert_eq!(
        m.transmit_input(0, 0x01fe30, 0),
        Reply::Value(0),
        "sensor repeat rule swallowed device-wide status"
    );
}
#[test]
fn literal_sensor_settings_repeat_boundary_and_interruption() {
    for (instance, opcode, query, initial) in
        [(0, 0x10, 0x1e, 0), (1, 0x21, 0x2d, 90), (2, 0x30, 0x3e, 30)]
    {
        for (gap, interrupt, changed) in [
            (100, 0, true),
            (101, 0, false),
            (1, 1, false),
            (1, 2, false),
        ] {
            let mut m = physical();
            let frame = 0x010000 | (instance << 8) | opcode;
            m.transmit_input(0, 0xc13028, 0);
            m.transmit_input(0, frame, 1);
            match interrupt {
                1 => {
                    m.transmit_input(0, 0x010380, 1);
                }
                2 => {
                    m.transmit(0, 0x01a0, 1);
                }
                _ => {}
            }
            m.transmit_input(0, frame, 1 + gap);
            assert_eq!(
                m.transmit_input(0, 0x010000 | (instance << 8) | query, 1 + gap),
                Reply::Value(if changed { 40 } else { initial }),
                "sensor repeat boundary or interruption violated"
            );
        }
    }
}
#[test]
fn literal_sensor_settings_scope_latch_and_readdress_retention() {
    let mut m = physical();
    let before = m.snapshot();
    assert_eq!(m.transmit_input(0, 0x01008c, 0), Reply::Value(171));
    m.set_input_value("panel", 0, vec![18, 52]).unwrap();
    set(&mut m, 0, 0x10, 50);
    assert_eq!(
        m.transmit_input(0, 0x01008d, 0),
        Reply::Value(205),
        "sensor setting changed captured value latch"
    );
    m.transmit_input(0, 0xc130ff, 0);
    pair(&mut m, 0x7f1f33);
    assert_eq!(m.transmit_input(0, 0x7f1f3c, 0), Reply::Value(255));
    assert_eq!(
        m.transmit_input(0, 0x7f003c, 0),
        Reply::Value(0),
        "last sensor instance write changed instance zero"
    );
    m.set_address("panel", Some(17)).unwrap();
    assert_eq!(m.transmit_input(0, 0x01001e, 0), Reply::None);
    assert_eq!(m.transmit_input(0, 0x23001e, 0), Reply::Value(50));
    let after = m.snapshot();
    assert_eq!(after["buses"][0]["outputs"], before["buses"][0]["outputs"]);
    assert_eq!(after["buses"][1], before["buses"][1]);
    for field in ["enabled", "priority", "scheme", "filter", "groups"] {
        assert_eq!(
            after["buses"][0]["inputs"][0]["settings"][0][field],
            before["buses"][0]["inputs"][0]["settings"][0][field]
        );
    }
    m.transmit_input(0, 0xc13028, 0);
    pair(&mut m, 0xfd0010);
    assert_eq!(m.transmit_input(0, 0xfd001e, 0), Reply::Value(40));
    assert_eq!(m.transmit_input(0, 0x23001e, 0), Reply::Value(50));
    m.transmit_input(0, 0xc1303c, 0);
    pair(&mut m, 0xff0010);
    assert_eq!(m.transmit_input(0, 0xff001e, 0), Reply::Value(60));
    assert_eq!(m.transmit_input(1, 0x01001e, 0), Reply::Value(0));
}
