use super::*;
use serde_json::{json, Value};

fn physical() -> Value {
    json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[
        {"id":"mixed","short_address":3,"level":100,"groups":1,"device_type":8,
        "colour":{"active":"temperature","xy":[4660,9029],"rgbwaf":[1,2,3,4,5,6],
        "temperature":{"value":250,"coolest":200,"warmest":400,"physical_coolest":153,"physical_warmest":500}}},
        {"id":"tc","short_address":4,"level":80,"groups":1,"device_type":8,
        "colour":{"active":"temperature","temperature":{"value":300,"coolest":200,"warmest":400,"physical_coolest":153,"physical_warmest":500}}},
        {"id":"plain","short_address":5,"level":42,"groups":1}
    ]}]})
}
fn model(value: Value) -> Model {
    Model::new(serde_json::from_value(value).unwrap()).unwrap()
}
fn send(model: &mut Model, frames: &[u16]) {
    for &frame in frames {
        assert_eq!(
            model.transmit(0, frame, 0),
            Reply::None,
            "write fabricated a backward reply"
        );
    }
}
fn query(model: &mut Model, frame: u16) -> Reply {
    send(model, &[0xc108]);
    model.transmit(0, frame, 0)
}
fn value(model: &mut Model, selector: u8) -> Reply {
    send(model, &[0xa300 | u16::from(selector)]);
    query(model, 0x07fa)
}

#[test]
fn emulator_colour_profiles_validate_capabilities_and_native_ranges() {
    let valid: Scenario = serde_json::from_value(physical()).unwrap();
    assert!(valid.validate().is_ok());
    for (pointer, bad) in [
        ("/buses/0/outputs/0/device_type", json!(0)),
        ("/buses/0/outputs/0/colour/active", json!("primary_n")),
        ("/buses/0/outputs/0/colour/xy", json!([65535, 1])),
        ("/buses/0/outputs/0/colour/rgbwaf", json!([])),
        (
            "/buses/0/outputs/0/colour/rgbwaf",
            json!([0, 0, 0, 0, 0, 0, 0]),
        ),
        ("/buses/0/outputs/0/colour/rgbwaf", json!([255])),
        ("/buses/0/outputs/0/colour/temperature/value", json!(0)),
        (
            "/buses/0/outputs/0/colour/temperature/physical_warmest",
            json!(65535),
        ),
        ("/buses/0/outputs/0/colour/temperature/coolest", json!(100)),
        ("/buses/0/outputs/1/colour/active", json!("rgbwaf")),
    ] {
        let mut invalid = physical();
        *invalid.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<Scenario>(invalid)
                .map_or(true, |scenario| scenario.validate().is_err()),
            "invalid colour profile accepted at {pointer}"
        );
    }
    let old: Scenario = serde_json::from_value(
        json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"legacy","short_address":0}]}]}),
    )
    .unwrap();
    assert!(old.validate().is_ok());
}

#[test]
fn emulator_dt8_enable_is_one_shot_for_standard_commands_only() {
    let mut m = model(physical());
    assert_eq!(m.transmit(0, 0x07f9, 0), Reply::None);
    assert_eq!(
        query(&mut m, 0x07f9),
        Reply::Value(0xc3),
        "DT8 capabilities were not modeled independently"
    );
    assert_eq!(query(&mut m, 0x07f7), Reply::Value(0));
    assert_eq!(query(&mut m, 0x07ff), Reply::Value(1));
    assert_eq!(
        m.transmit(0, 0x07f9, 0),
        Reply::None,
        "DT8 selection survived its command"
    );
    send(&mut m, &[0xc107]);
    assert_eq!(m.transmit(0, 0x07f9, 0), Reply::None);
    send(&mut m, &[0xc108, 0x0b00]); // Other fixture consumes selection on every gear.
    assert_eq!(
        m.transmit(0, 0x07f9, 0),
        Reply::None,
        "unaddressed gear retained the DT8 latch"
    );
    send(&mut m, &[0xc108, 0x092e]); // First repeat-required standard command.
    assert_eq!(
        m.transmit(0, 0x07f9, 0),
        Reply::None,
        "repeat-first command retained DT8 selection"
    );
    send(&mut m, &[0xc108, 0xa302]); // Special DTR load preserves selection.
    assert_eq!(m.transmit(0, 0x07fa, 0), Reply::Value(0));
    send(&mut m, &[0xc108]);
    m.intervening_frame(0); // 24-bit traffic is not a gear command.
    assert_eq!(
        m.transmit(0, 0x07f9, 0),
        Reply::Value(0xc3),
        "input traffic incorrectly cancelled DT8 selection"
    );
    assert_eq!(
        query(&mut m, 0x0bf9),
        Reply::None,
        "plain gear fabricated DT8 support"
    );
}

#[test]
fn emulator_tc_stages_clamps_and_mask_overwrites_before_activation() {
    let mut m = model(physical());
    send(&mut m, &[0xa3c2, 0xc301, 0xc108, 0x07e7]); // 450, above configured warmest.
    assert_eq!(
        value(&mut m, 2),
        Reply::Value(0),
        "temporary Tc changed actual value before ACTIVATE"
    );
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(250));
    send(&mut m, &[0xc108, 0x07e2]);
    let staged = &m.snapshot()["buses"][0]["outputs"][0]["colour_state"]["temporary"];
    assert!(
        staged["kind"].is_null(),
        "ACTIVATE retained the temporary colour type"
    );
    assert_eq!(
        staged["temperature"], 65535,
        "ACTIVATE retained the temporary Tc"
    );
    assert_eq!(value(&mut m, 2), Reply::Value(1));
    assert_eq!(
        m.transmit(0, 0x0798, 0),
        Reply::Value(144),
        "Tc was not clamped to configured limits"
    );
    assert_eq!(query(&mut m, 0x07f8), Reply::Value(0x22));
    send(
        &mut m,
        &[
            0xa32c, 0xc301, 0xc108, 0x07e7, 0xa3ff, 0xc3ff, 0xc108, 0x07e7, 0xc108, 0x07e2,
        ],
    );
    assert_eq!(value(&mut m, 2), Reply::Value(1));
    assert_eq!(
        m.transmit(0, 0x0798, 0),
        Reply::Value(144),
        "MASK failed to replace the old staged Tc"
    );
    send(&mut m, &[0xc108, 0x07e2]);
    assert_eq!(
        m.transmit(0, 0x07a0, 0),
        Reply::Value(100),
        "colour changed master brightness"
    );
    for (selector, high, low) in [(128, 0, 200), (129, 0, 153), (130, 1, 144), (131, 1, 244)] {
        assert_eq!(value(&mut m, selector), Reply::Value(high));
        assert_eq!(
            m.transmit(0, 0x0798, 0),
            Reply::Value(low),
            "Tc limit low byte mismatch"
        );
    }
}

#[test]
fn emulator_rgbwaf_masks_and_inactive_queries_preserve_native_channels() {
    let mut m = model(physical());
    assert_eq!(value(&mut m, 9), Reply::Value(255)); // Inactive RGB is not recalculated from Tc.
    send(
        &mut m,
        &[
            0xa3fe, 0xc300, 0xc57f, 0xc108, 0x07eb, 0xa3ff, 0xc305, 0xc5ff, 0xc108, 0x07ec,
        ],
    );
    assert_eq!(query(&mut m, 0x07f8), Reply::Value(0x20));
    send(&mut m, &[0xc108, 0x07e2]);
    for (selector, expected) in [
        (9, 254),
        (10, 0),
        (11, 127),
        (12, 4),
        (13, 5),
        (14, 6),
        (15, 128),
    ] {
        assert_eq!(
            value(&mut m, selector),
            Reply::Value(expected),
            "RGBWAF activation changed a MASK channel or byte width"
        );
    }
    assert_eq!(value(&mut m, 2), Reply::Value(255));
    assert_eq!(
        m.transmit(0, 0x0798, 0),
        Reply::Value(255),
        "inactive word was not full MASK"
    );
    assert_eq!(query(&mut m, 0x07fb), Reply::Value(128));
    send(&mut m, &[0xc108, 0x07e2]);
    assert_eq!(
        value(&mut m, 9),
        Reply::Value(254),
        "ACTIVATE replayed stale staged channels"
    );
    send(
        &mut m,
        &[
            0xa355, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xc108,
            0x07e2,
        ],
    );
    assert_eq!(
        value(&mut m, 9),
        Reply::Value(254),
        "MASK retained an earlier staged RGB value"
    );
    assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(100));
    assert_eq!(query(&mut m, 0x09f9), Reply::Value(2));
    send(
        &mut m,
        &[0xa3fe, 0xc3fe, 0xc5fe, 0xc108, 0x09eb, 0xc108, 0x09e2],
    );
    assert_eq!(
        query(&mut m, 0x09f8),
        Reply::Value(0x20),
        "unsupported RGB setter changed a Tc-only fixture"
    );
    send(
        &mut m,
        &[
            0xa35e, 0xc301, 0xc108, 0x09e7, 0xa3fe, 0xc3fe, 0xc5fe, 0xc108, 0x09eb, 0xc108, 0x09e2,
        ],
    );
    send(&mut m, &[0xa302, 0xc108]);
    assert_eq!(m.transmit(0, 0x09fa, 0), Reply::Value(1));
    assert_eq!(
        m.transmit(0, 0x0998, 0),
        Reply::Value(94),
        "unsupported RGB setter erased a supported pending Tc"
    );
}

#[test]
fn emulator_xy_words_group_selection_and_duplicate_replies_are_independent() {
    let mut m = model(physical());
    send(
        &mut m,
        &[
            0xa300, 0xc3ff, 0xc108, 0x07e0, 0xa334, 0xc312, 0xc108, 0x07e1, 0xc108, 0x07e2,
        ],
    );
    assert_eq!(
        value(&mut m, 0),
        Reply::Value(255),
        "xy query did not return the literal MSB"
    );
    assert_eq!(
        m.transmit(0, 0x0798, 0),
        Reply::Value(0),
        "valid FF00 xy word became MASK"
    );
    assert_eq!(value(&mut m, 1), Reply::Value(0x12));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(0x34));
    send(&mut m, &[0xa32c, 0xc301, 0xc108, 0x81e7, 0xc108, 0x81e2]); // Group 0 Tc300.
    assert_eq!(query(&mut m, 0x07f8), Reply::Value(0x20));
    assert_eq!(query(&mut m, 0x09f8), Reply::Value(0x20));
    assert_eq!(m.transmit(0, 0x0ba0, 0), Reply::Value(42));
    send(&mut m, &[0xa35e, 0xc301, 0xc108, 0xffe7, 0xc108, 0xffe2]); // Broadcast Tc350.
    assert_eq!(value(&mut m, 2), Reply::Value(1));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(94));
    send(&mut m, &[0xa302, 0xc108]);
    assert_eq!(m.transmit(0, 0x09fa, 0), Reply::Value(1));
    assert_eq!(m.transmit(0, 0x0998, 0), Reply::Value(94));
    let mut duplicate = physical();
    duplicate["buses"][0]["outputs"][1]["short_address"] = json!(3);
    let mut duplicate = model(duplicate);
    assert_eq!(
        query(&mut duplicate, 0x07f9),
        Reply::Collision,
        "different DT8 replies were not a collision"
    );
}

#[test]
fn emulator_rgb_only_fixture_masks_missing_channels_without_word_side_effects() {
    let mut profile = physical();
    profile["buses"][0]["outputs"][0]["colour"] = json!({"active":"rgbwaf","rgbwaf":[10,20,30]});
    let mut m = model(profile);
    assert_eq!(query(&mut m, 0x07f9), Reply::Value(0x60));
    assert_eq!(
        value(&mut m, 12),
        Reply::Value(255),
        "missing white channel fabricated a value"
    );
    assert_eq!(
        m.transmit(0, 0x0798, 0),
        Reply::Value(12),
        "byte value query overwrote DTR0 as a word low byte"
    );
    send(&mut m, &[0xa32c, 0xc301, 0xc108, 0x07e7, 0xc108, 0x07e2]);
    assert_eq!(query(&mut m, 0x07f8), Reply::Value(0x80));
    assert_eq!(value(&mut m, 9), Reply::Value(10));
    assert_eq!(value(&mut m, 2), Reply::Value(255));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(255));
}

#[test]
fn emulator_colour_queries_update_both_word_registers_but_preserve_byte_registers() {
    let mut m = model(physical());
    send(&mut m, &[0xc3aa, 0xa302, 0xc108]);
    assert_eq!(m.transmit(0, 0x07fa, 0), Reply::Value(0));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(250));
    assert_eq!(
        m.transmit(0, 0x079c, 0),
        Reply::Value(0),
        "word query left a stale MSB in DTR1"
    );
    send(&mut m, &[0xc3aa, 0xa300, 0xc108]); // Inactive xy = FFFF.
    assert_eq!(m.transmit(0, 0x07fa, 0), Reply::Value(255));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(255));
    assert_eq!(
        m.transmit(0, 0x079c, 0),
        Reply::Value(255),
        "word MASK did not fill DTR1"
    );
    send(&mut m, &[0xc3aa, 0xa309, 0xc108]); // Inactive RGB byte = FF, registers untouched.
    assert_eq!(m.transmit(0, 0x07fa, 0), Reply::Value(255));
    assert_eq!(m.transmit(0, 0x0798, 0), Reply::Value(9));
    assert_eq!(
        m.transmit(0, 0x079c, 0),
        Reply::Value(0xaa),
        "byte query overwrote DTR1"
    );
    let mut rgb_profile = physical();
    rgb_profile["buses"][0]["outputs"][0]["colour"]["active"] = json!("rgbwaf");
    let mut rgb = model(rgb_profile);
    send(&mut rgb, &[0xc3aa, 0xa309, 0xc108]);
    assert_eq!(rgb.transmit(0, 0x07fa, 0), Reply::Value(1));
    assert_eq!(rgb.transmit(0, 0x0798, 0), Reply::Value(9));
    assert_eq!(
        rgb.transmit(0, 0x079c, 0),
        Reply::Value(0xaa),
        "known byte query overwrote DTR1"
    );
}
