//! Literal native Part 102 settings; independent of the bridge codec.
use super::*;
use serde_json::json;

#[test]
fn literal_gear_settings_physical_minimum_has_fixed_profile_readback() {
    let mut m = physical();
    assert_eq!(
        m.transmit(0, 0x7f9a, 0),
        Reply::Value(1),
        "physical minimum query missing from the fixed output profile"
    );
    assert_eq!(m.transmit(1, 0x7f9a, 0), Reply::Value(1));
    assert_eq!(m.transmit(0, 0x7fa2, 0), Reply::Value(1));
}

fn physical() -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[
            {"number":0,"outputs":[
                {"id":"lit","short_address":63,"level":100,"groups":32768},
                {"id":"off","short_address":0,"level":0,"groups":32768},
                {"id":"peer","short_address":1,"level":90}]},
            {"number":1,"outputs":[{"id":"other","short_address":63,"level":100}]}
        ]}))
        .unwrap(),
    )
    .unwrap()
}
fn store(m: &mut Model, dtr: u16, setter: u16) {
    assert_eq!(m.transmit(0, dtr, 0), Reply::None);
    assert_eq!(m.transmit(0, setter, 0), Reply::None);
    assert_eq!(m.transmit(0, setter, 0), Reply::None);
}

#[test]
fn literal_gear_settings_all_stores_and_fade_nibbles_preserve_raw_values() {
    let mut m = physical();
    for (dtr, setter, query, expected) in [
        (0xa3c8, 0x7f2a, 0x7fa1, 200),
        (0xa305, 0x7f2b, 0x7fa2, 5),
        (0xa300, 0x7f2d, 0x7fa3, 0),
        (0xa37f, 0x7f2c, 0x7fa4, 127),
        (0xa3ff, 0x7f2d, 0x7fa3, 255),
        (0xa3ff, 0x7f2c, 0x7fa4, 255),
        (0xa30f, 0x7f2e, 0x7fa5, 0xf7),
        (0xa301, 0x7f2f, 0x7fa5, 0xf1),
        (0xa301, 0x7f2e, 0x7fa5, 0x11),
        (0xa30f, 0x7f2f, 0x7fa5, 0x1f),
        (0xa30f, 0x7f2e, 0x7fa5, 0xff),
        (0xa310, 0x7f2e, 0x7fa5, 0xff),
        (0xa300, 0x7f2f, 0x7fa5, 0xff),
        (0xa300, 0x7f2e, 0x7fa5, 0x0f),
    ] {
        store(&mut m, dtr, setter);
        assert_eq!(
            m.transmit(0, query, 0),
            Reply::Value(expected),
            "literal stored setting/readback or unrequested fade nibble changed"
        );
    }
    for (bus, address) in [(0, 0x01), (0, 0x03), (1, 0x7f)] {
        for (opcode, expected) in [(0xa1, 254), (0xa2, 1), (0xa3, 254), (0xa4, 254), (0xa5, 7)] {
            assert_eq!(
                m.transmit(bus, (address << 8) | opcode, 0),
                Reply::Value(expected),
                "settings leaked to an unselected gear or another bus"
            );
        }
    }
}

#[test]
fn literal_gear_settings_every_setter_needs_adjacent_repeat_within_window() {
    for (opcode, dtr, query, original, applied) in [
        (0x2a, 200, 0xa1, 254, 200),
        (0x2b, 5, 0xa2, 1, 5),
        (0x2c, 99, 0xa4, 254, 99),
        (0x2d, 98, 0xa3, 254, 98),
        (0x2e, 15, 0xa5, 7, 0xf7),
        (0x2f, 1, 0xa5, 7, 1),
    ] {
        for interrupted in [false, true] {
            let mut m = physical();
            let setter = 0x7f00 | opcode;
            let read = 0x7f00 | query;
            m.transmit(0, 0xa300 | dtr, 0);
            m.transmit(0, setter, 1);
            if interrupted {
                m.transmit(0, 0x01a0, 2);
            }
            let at = if interrupted { 3 } else { 102 };
            m.transmit(0, setter, at);
            assert_eq!(
                m.transmit(0, read, at),
                Reply::Value(original),
                "interrupted/expired setter pair changed a setting"
            );
            m.transmit(0, setter, at + 1);
            m.transmit(0, setter, at + 101);
            assert_eq!(
                m.transmit(0, read, at + 101),
                Reply::Value(applied),
                "valid boundary repeat did not apply setting"
            );
        }
    }
}

#[test]
fn literal_gear_settings_group_limits_clamp_lit_preserve_off_and_bus_scope() {
    let mut m = physical();
    store(&mut m, 0xa378, 0x9f2b);
    assert_eq!(m.transmit(0, 0x7fa0, 0), Reply::Value(120));
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(0),
        "raising minimum turned on an off fixture"
    );
    assert_eq!(m.transmit(0, 0x03a2, 0), Reply::Value(1));
    store(&mut m, 0xa332, 0x9f2a);
    assert_eq!(
        m.transmit(0, 0x7fa1, 0),
        Reply::Value(120),
        "maximum crossed minimum rather than clamping"
    );
    store(&mut m, 0xa301, 0x9f2b);
    store(&mut m, 0xa332, 0x9f2a);
    assert_eq!(m.transmit(0, 0x7fa0, 0), Reply::Value(50));
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(0));
    assert_eq!(m.transmit(0, 0x03a0, 0), Reply::Value(90));
    assert_eq!(m.transmit(1, 0x7fa0, 0), Reply::Value(100));
    store(&mut m, 0xa3ff, 0xff2c);
    for frame in [0x01a4, 0x03a4, 0x7fa4] {
        assert_eq!(m.transmit(0, frame, 0), Reply::Value(255));
    }
    assert_eq!(m.transmit(1, 0x7fa4, 0), Reply::Value(254));
}
