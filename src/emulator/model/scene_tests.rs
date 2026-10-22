//! Literal native commands, independent of the bridge's scene codec.
use super::*;
use serde_json::{json, Value};

fn physical(automatic: bool) -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[
            {"number":0,"outputs":[
                {"id":"colour","short_address":3,"groups":1,"level":100,"device_type":8,
                 "colour":{"active":"temperature","auto_activation":automatic,
                    "xy":[4660,9029],"rgbwaf":[1,2,3,4,5,6],
                    "temperature":{"value":250,"coolest":200,"warmest":400,
                                   "physical_coolest":153,"physical_warmest":500}}},
                {"id":"plain","short_address":4,"groups":1,"level":42}
            ]}
        ]}))
        .unwrap(),
    )
    .unwrap()
}
fn send(m: &mut Model, frames: &[u16]) {
    for &frame in frames {
        assert_eq!(
            m.transmit(0, frame, 0),
            Reply::None,
            "write fabricated a reply"
        );
    }
}
fn value(m: &mut Model, selector: u8) -> u8 {
    send(m, &[0xa300 | u16::from(selector), 0xc108]);
    let Reply::Value(value) = m.transmit(0, 0x07fa, 0) else {
        panic!("missing colour reply")
    };
    value
}
fn word(m: &mut Model, selector: u8) -> u16 {
    let high = value(m, selector);
    let Reply::Value(low) = m.transmit(0, 0x0798, 0) else {
        panic!("missing word LSB")
    };
    u16::from_be_bytes([high, low])
}
fn tc(m: &Model) -> Value {
    m.snapshot()["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]["value"].clone()
}
fn stage_tc(m: &mut Model) {
    send(m, &[0xa35e, 0xc301, 0xc108, 0x07e7]); // 350 mirek.
}
fn stage_masked(m: &mut Model, family: u8) {
    let frames: &[u16] = match family {
        0x10 => &[0xa3ff, 0xc3ff, 0xc108, 0x07e0, 0xc108, 0x07e1],
        0x20 => &[0xa3ff, 0xc3ff, 0xc108, 0x07e7],
        0x80 => &[
            0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xc108, 0x07ec, 0xc108, 0x07ed,
        ],
        _ => panic!("invalid fixture family"),
    };
    send(m, frames);
}
fn store(m: &mut Model, slot: u16, level: u8) {
    let frame = 0x0740 | slot;
    send(m, &[0xa300 | u16::from(level), frame, frame]);
}

#[test]
fn scene_store_requires_repeat_and_retains_stored_report_without_activation() {
    let mut m = physical(true);
    stage_tc(&mut m);
    send(&mut m, &[0xa300, 0x0740]);
    assert_eq!(m.transmit(0, 0x07b0, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        255,
        "single scene setting stored colour"
    );
    store(&mut m, 0, 0);
    assert_eq!(tc(&m), 250, "scene storage activated colour");
    assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(100));
    assert_eq!(m.transmit(0, 0x07b0, 0), Reply::Value(0));
    assert_eq!(value(&mut m, 240), 0x20, "scene colour type was not stored");
    assert_eq!(word(&mut m, 226), 350, "scene report lost stored Tc");
    assert_eq!(
        value(&mut m, 208),
        255,
        "scene Store retained temporary colour"
    );
    assert_eq!(m.transmit(0, 0x07b1, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        255,
        "storing one scene changed another slot"
    );
    assert_eq!(m.transmit(0, 0x09b0, 0), Reply::Value(255));
}

#[test]
fn scene_masked_temporary_type_preserves_colour_while_remove_clears_both() {
    let mut m = physical(true);
    stage_tc(&mut m);
    store(&mut m, 15, 30);
    store(&mut m, 15, 255); // Temporary type was consumed by the first Store.
    assert_eq!(m.transmit(0, 0x07bf, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        0x20,
        "masked temporary type erased stored colour"
    );
    assert_eq!(word(&mut m, 226), 350);
    send(&mut m, &[0x075f]);
    assert_eq!(m.transmit(0, 0x07bf, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        0x20,
        "single Remove erased scene colour"
    );
    send(&mut m, &[0x075f, 0x075f]);
    assert_eq!(m.transmit(0, 0x07bf, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        255,
        "Remove kept scene colour participation"
    );
    assert_eq!(word(&mut m, 226), 65535);
    assert_eq!(tc(&m), 250);
}

#[test]
fn scene_recall_colour_only_respects_automatic_and_manual_activation() {
    for automatic in [true, false] {
        let mut m = physical(automatic);
        stage_tc(&mut m);
        store(&mut m, 2, 255);
        send(&mut m, &[0x0712]);
        assert_eq!(
            m.transmit(0, 0x07a0, 0),
            Reply::Value(100),
            "colour-only scene changed brightness"
        );
        assert_eq!(
            tc(&m),
            if automatic { 350 } else { 250 },
            "scene recall ignored activation mode"
        );
        if !automatic {
            assert_eq!(
                word(&mut m, 194),
                350,
                "manual scene did not load temporary colour"
            );
            send(&mut m, &[0xc108, 0x07e2]);
            assert_eq!(tc(&m), 350);
        }
        assert_eq!(
            value(&mut m, 208),
            255,
            "scene activation retained temporary type"
        );
    }
}

#[test]
fn scene_ignored_entry_cannot_activate_previously_pending_colour() {
    let mut m = physical(true);
    stage_tc(&mut m);
    send(&mut m, &[0xff10]); // Ignored on both fixtures.
    assert_eq!(
        tc(&m),
        250,
        "ignored scene activated unrelated pending colour"
    );
    assert_eq!(word(&mut m, 194), 350);
    // A level-only scene replaces that pending colour with its masked colour.
    send(&mut m, &[0xc108, 0x07e2]);
    store(&mut m, 1, 0);
    send(&mut m, &[0xa32c, 0xc301, 0xc108, 0x07e7]); // Pending 300 differs from actual 350.
    send(&mut m, &[0x8111]); // Group 0, Off is a participating level.
    assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(0));
    assert_eq!(m.transmit(0, 0x09a0, 0), Reply::Value(42));
    assert_eq!(
        tc(&m),
        350,
        "level-only scene activated earlier pending colour"
    );
    assert_eq!(value(&mut m, 208), 255);
}

#[test]
fn scene_report_actual_and_temporary_are_separate_and_copy_does_not_activate() {
    let mut m = physical(false);
    stage_tc(&mut m);
    store(&mut m, 3, 90);
    send(&mut m, &[0xa32c, 0xc301, 0xc108, 0x07e7]); // Pending 300, actual 250.
    assert_eq!(m.transmit(0, 0x07b3, 0), Reply::Value(90));
    assert_eq!(
        word(&mut m, 226),
        350,
        "scene query did not select stored report"
    );
    assert_eq!(
        word(&mut m, 194),
        300,
        "report query changed pending colour"
    );
    assert_eq!(tc(&m), 250);
    send(&mut m, &[0xc108, 0x07ee]);
    assert_eq!(tc(&m), 250, "copying scene report activated colour");
    assert_eq!(
        word(&mut m, 194),
        350,
        "CopyReportToTemporary did not copy scene report"
    );
    assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(100));
    assert_eq!(
        word(&mut m, 226),
        250,
        "actual query retained stale scene report"
    );
    assert_eq!(word(&mut m, 194), 350);
    // Copying a masked report clears all pending attributes and type explicitly.
    assert_eq!(m.transmit(0, 0x07b4, 0), Reply::Value(255));
    send(&mut m, &[0xc108, 0x07ee]);
    assert_eq!(
        value(&mut m, 208),
        255,
        "masked report retained pending type"
    );
    assert_eq!(word(&mut m, 194), 65535);
}

#[test]
fn scene_rgb_masks_control_and_xy_words_survive_store_recall_and_reset() {
    let mut m = physical(true);
    send(
        &mut m,
        &[
            0xa312, 0xc3ff, 0xc556, 0xc108, 0x07eb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07ec, 0xa341,
            0xc108, 0x07ed,
        ],
    );
    store(&mut m, 5, 255);
    assert_eq!(m.transmit(0, 0x07b5, 0), Reply::Value(255));
    assert_eq!(value(&mut m, 240), 0x80, "RGB scene type missing");
    assert_eq!(value(&mut m, 233), 0x12);
    assert_eq!(value(&mut m, 234), 255, "stored RGB MASK changed");
    assert_eq!(value(&mut m, 239), 0x41, "stored RGB control changed");
    send(&mut m, &[0x0715]);
    assert_eq!(value(&mut m, 9), 0x12);
    assert_eq!(
        value(&mut m, 10),
        2,
        "scene recall overwrote a masked RGB channel"
    );
    assert_eq!(value(&mut m, 11), 0x56);
    send(&mut m, &[0xc108]);
    assert_eq!(
        m.transmit(0, 0x07fb, 0),
        Reply::Value(0x41),
        "RGB actual control ignored scene"
    );
    // A scene can select RGB while preserving every channel and its mode.
    send(
        &mut m,
        &[
            0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07eb, 0xc108, 0x07ec, 0xc108, 0x07ed,
        ],
    );
    store(&mut m, 8, 255);
    send(&mut m, &[0x0718, 0xc108]);
    assert_eq!(
        m.transmit(0, 0x07fb, 0),
        Reply::Value(0x41),
        "masked scene control overwrote actual mode"
    );
    assert_eq!(value(&mut m, 9), 0x12);
    assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(100));
    assert_eq!(
        value(&mut m, 239),
        0x41,
        "actual RGB report lost recalled control"
    );
    // Explicitly selected all-MASK xy is different from a MASK colour type.
    send(&mut m, &[0xa3ff, 0xc3ff, 0xc108, 0x07e0, 0xc108, 0x07e1]);
    store(&mut m, 6, 255);
    assert_eq!(m.transmit(0, 0x07b6, 0), Reply::Value(255));
    assert_eq!(
        value(&mut m, 240),
        0x10,
        "all-MASK attributes erased selected family"
    );
    assert_eq!(word(&mut m, 224), 65535);
    send(
        &mut m,
        &[
            0xa3ab, 0xc312, 0xc108, 0x07e0, 0xa3ff, 0xc3ff, 0xc108, 0x07e1,
        ],
    );
    store(&mut m, 7, 0);
    send(&mut m, &[0x0717]);
    assert_eq!(word(&mut m, 0), 0x12ab);
    assert_eq!(word(&mut m, 1), 0x2345, "xy recall overwrote masked Y");
    send(&mut m, &[0xc108]);
    assert_eq!(
        m.transmit(0, 0x07fb, 0),
        Reply::Value(0x40),
        "xy activation retained linked RGB channels"
    );
    // Put data in every slot, then check Reset clears every colour record.
    for slot in 0..16 {
        stage_tc(&mut m);
        store(&mut m, slot, 25);
    }
    send(&mut m, &[0x0720, 0x0720]);
    for slot in 0..16 {
        assert_eq!(m.transmit(0, 0x07b0 | slot, 0), Reply::Value(255));
        assert_eq!(
            value(&mut m, 240),
            255,
            "Reset retained a stored scene colour"
        );
    }
}

#[test]
fn scene_temporary_rgb_control_selects_family_and_clears_prior_temperature() {
    let mut m = physical(false);
    stage_tc(&mut m);
    send(&mut m, &[0xa341, 0xc108, 0x07ed]);
    assert_eq!(
        value(&mut m, 208),
        0x80,
        "temporary RGB control did not select RGB family"
    );
    assert_eq!(word(&mut m, 194), 65535, "RGB control retained pending Tc");
    store(&mut m, 9, 255);
    assert_eq!(m.transmit(0, 0x07b9, 0), Reply::Value(255));
    assert_eq!(value(&mut m, 240), 0x80);
    assert_eq!(
        word(&mut m, 226),
        65535,
        "RGB report exposed a different family's value"
    );
    assert_eq!(value(&mut m, 233), 255);
    assert_eq!(value(&mut m, 239), 0x41);
    assert_eq!(tc(&m), 250, "storing RGB control activated colour");
    send(&mut m, &[0x0719, 0xc108, 0x07e2, 0xc108]);
    assert_eq!(m.transmit(0, 0x07fb, 0), Reply::Value(0x41));
    assert_eq!(
        value(&mut m, 9),
        1,
        "control-only scene changed a masked channel"
    );
    // MASK control still selects its family, but a later Tc setter masks it.
    send(&mut m, &[0xa3ff, 0xc108, 0x07ed]);
    assert_eq!(value(&mut m, 208), 0x80);
    stage_tc(&mut m);
    assert_eq!(value(&mut m, 208), 0x20);
    assert_eq!(value(&mut m, 207), 255);
}

#[test]
fn scene_all_masked_selected_families_switch_type_in_auto_and_manual_modes() {
    for automatic in [true, false] {
        for original in [0x10, 0x80, 0x20] {
            for selected in [0x20, 0x10, 0x80] {
                if original == selected {
                    continue;
                }
                let mut m = physical(automatic);
                stage_masked(&mut m, original);
                send(&mut m, &[0xc108, 0x07e2]);
                stage_masked(&mut m, selected);
                store(&mut m, 10, 255);
                assert_eq!(m.transmit(0, 0x07ba, 0), Reply::Value(255));
                assert_eq!(value(&mut m, 240), selected);
                assert_eq!(tc(&m), 250, "storing MASK changed numeric Tc");
                send(&mut m, &[0xc108]);
                assert_eq!(m.transmit(0, 0x07f8, 0), Reply::Value(original));
                send(&mut m, &[0x071a]);
                if !automatic {
                    send(&mut m, &[0xc108]);
                    assert_eq!(
                        m.transmit(0, 0x07f8, 0),
                        Reply::Value(original),
                        "manual MASK scene activated early"
                    );
                    send(&mut m, &[0xc108, 0x07e2]);
                }
                send(&mut m, &[0xc108]);
                assert_eq!(
                    m.transmit(0, 0x07f8, 0),
                    Reply::Value(selected),
                    "scene MASK value prevented selected-family activation"
                );
                assert_eq!(tc(&m), 250, "MASK Tc was clamped instead of preserved");
                let actual = &m.snapshot()["buses"][0]["outputs"][0]["colour_state"]["actual"];
                assert_eq!(actual["xy"], json!([4660, 9029]));
                assert_eq!(actual["rgbwaf"], json!([1, 2, 3, 4, 5, 6]));
                assert_eq!(value(&mut m, 208), 255);
                assert_eq!(m.transmit(0, 0x07a0, 0), Reply::Value(100));
            }
        }
    }
}
