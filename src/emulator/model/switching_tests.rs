//! Literal Part208 examples, independent of bridge frame constructors.
use super::*;
use serde_json::json;

fn fixed() -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
            {"id":"relay","short_address":0,"device_type":7,"level":0},
            {"id":"dimmer","short_address":1,"level":90}
        ]}]}))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn literal_dt7_actual_level_is_binary_and_default_limits_are_switching_limits() {
    let mut m = fixed();
    m.transmit(0, 0x0064, 0);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(254),
        "DT7 actual readback was a virtual dimmer level"
    );
    assert_eq!(m.transmit(0, 0x019a, 0), Reply::Value(254));
    assert_eq!(m.transmit(0, 0x01a2, 0), Reply::Value(254));
    assert_eq!(m.transmit(0, 0x0190, 0), Reply::Value(4));
    m.transmit(0, 0x00ff, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(254));
    m.transmit(0, 0x0100, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(0));
    assert_eq!(m.transmit(0, 0x0190, 0), Reply::Value(0));
    assert_eq!(
        m.transmit(0, 0x03a0, 0),
        Reply::Value(90),
        "DT7 behavior changed the ordinary dimmer"
    );
}

fn adjustable(thresholds: [u8; 4], level: u8) -> Model {
    Model::new(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
            {"id":"relay","short_address":0,"device_type":7,"level":level,
             "switching":{"adjustable_thresholds":true,"thresholds":thresholds}}
        ]}]}))
        .unwrap(),
    )
    .unwrap()
}
fn query(m: &mut Model, bus: u8, frame: u16) -> Reply {
    let at = m.now_ms;
    m.transmit(bus, 0xc107, at);
    m.transmit(bus, frame, at)
}
fn setting(m: &mut Model, destination: u16, opcode: u8, raw: u8) {
    let at = m.now_ms;
    for frame in [
        0xa300 | u16::from(raw),
        0xc107,
        destination | u16::from(opcode),
        destination | u16::from(opcode),
    ] {
        m.transmit(0, frame, at);
    }
}
fn base_setting(m: &mut Model, opcode: u8, raw: u8) {
    let at = m.now_ms;
    for frame in [
        0xa300 | u16::from(raw),
        0x0100 | u16::from(opcode),
        0x0100 | u16::from(opcode),
    ] {
        m.transmit(0, frame, at);
    }
}
fn drive(m: &mut Model, raw: u8) -> Reply {
    let at = m.now_ms;
    m.transmit(0, u16::from(raw), at);
    m.transmit(0, 0x01a0, at)
}

#[test]
fn literal_dt7_hysteresis_and_store_actual_use_distinct_virtual_and_actual_values() {
    let mut m = adjustable([30, 255, 255, 20], 0);
    base_setting(&mut m, 0x2b, 1);
    assert_eq!(m.transmit(0, 0x01a2, 0), Reply::Value(1));
    assert_eq!(m.transmit(0, 0x019a, 0), Reply::Value(254));
    for (value, actual) in [(1, 0), (40, 254), (25, 254), (20, 0), (25, 0), (30, 254)] {
        assert_eq!(
            drive(&mut m, value),
            Reply::Value(actual),
            "DT7 hysteresis used virtual level as contact state"
        );
        m.transmit(0, 0x0121, 0);
        m.transmit(0, 0x0121, 0);
        assert_eq!(
            m.transmit(0, 0x0198, 0),
            Reply::Value(value),
            "DT7 STORE ACTUAL lost the virtual control level"
        );
        assert_eq!(
            m.transmit(0, 0x0190, 0),
            Reply::Value(if actual == 0 { 0 } else { 4 })
        );
    }
    assert_eq!(query(&mut m, 0, 0x01f0), Reply::Value(8));
    assert_eq!(query(&mut m, 0, 0x01f7), Reply::Value(2));
    assert_eq!(query(&mut m, 0, 0x01ff), Reply::Value(1));
    assert_eq!(query(&mut m, 0, 0x01f3), Reply::Value(255));
}

#[test]
fn literal_dt7_direction_reversal_ties_and_disabled_thresholds_are_independent() {
    let mut m = adjustable([50, 100, 150, 200], 0);
    base_setting(&mut m, 0x2b, 1);
    for (level, actual, status) in [(254, 0, 4), (140, 254, 8), (141, 0, 4)] {
        assert_eq!(
            drive(&mut m, level),
            Reply::Value(actual),
            "DT7 direction reversal or last applicable threshold was ignored"
        );
        assert_eq!(query(&mut m, 0, 0x01f1), Reply::Value(status));
    }
    let mut tied = adjustable([100, 100, 100, 100], 0);
    base_setting(&mut tied, 0x2b, 1);
    assert_eq!(
        drive(&mut tied, 150),
        Reply::Value(254),
        "DT7 equal thresholds did not prefer On"
    );
    assert_eq!(
        drive(&mut tied, 50),
        Reply::Value(254),
        "DT7 downward equal thresholds did not prefer On"
    );
    assert_eq!(query(&mut tied, 0, 0x01f1), Reply::Value(8));
    for initial in [0, 254] {
        let mut disabled = adjustable([255; 4], initial);
        base_setting(&mut disabled, 0x2b, 1);
        for level in [254, 100, 0, 50, 255] {
            assert_eq!(
                drive(&mut disabled, level),
                Reply::Value(initial),
                "DT7 disabled thresholds operated the contact"
            );
        }
    }
}

#[test]
fn literal_dt7_programming_is_nonactivating_and_reset_preserves_status_evidence() {
    let mut m = adjustable([100, 255, 255, 50], 0);
    base_setting(&mut m, 0x2b, 1);
    assert_eq!(drive(&mut m, 80), Reply::Value(0));
    setting(&mut m, 0x0100, 0xe1, 50);
    assert_eq!(query(&mut m, 0, 0x01f2), Reply::Value(50));
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(0),
        "DT7 threshold programming activated the contact"
    );
    assert_eq!(drive(&mut m, 81), Reply::Value(254));
    assert_eq!(drive(&mut m, 40), Reply::Value(0));
    assert_eq!(query(&mut m, 0, 0x01f1), Reply::Value(12));
    m.transmit(0, 0x0120, 0);
    m.transmit(0, 0x0120, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(254));
    assert_eq!(m.transmit(0, 0x01a2, 0), Reply::Value(254));
    assert_eq!(
        query(&mut m, 0, 0x01f1),
        Reply::Value(12),
        "DT7 RESET erased previous raw switch status"
    );
    for (opcode, value) in [(0xf2, 1), (0xf3, 255), (0xf4, 255), (0xf5, 0)] {
        assert_eq!(query(&mut m, 0, 0x0100 | opcode), Reply::Value(value));
    }
}

#[test]
fn literal_dt7_fixed_profiles_ignore_optional_setters_and_adjustable_limits_operate_virtual_level()
{
    let mut m = fixed();
    assert_eq!(query(&mut m, 0, 0x01f0), Reply::Value(0));
    for (setter, value, read, original) in [
        (0xe1, 99, 0xf2, 1),
        (0xe2, 99, 0xf3, 255),
        (0xe3, 99, 0xf4, 255),
        (0xe4, 99, 0xf5, 0),
    ] {
        setting(&mut m, 0x0100, setter, value);
        assert_eq!(
            query(&mut m, 0, 0x0100 | read),
            Reply::Value(original),
            "DT7 fixed profile accepted a threshold setter"
        );
    }
    base_setting(&mut m, 0x2b, 1);
    base_setting(&mut m, 0x2a, 100);
    assert_eq!(
        m.transmit(0, 0x01a2, 0),
        Reply::Value(254),
        "DT7 fixed profile changed virtual limits"
    );
    assert_eq!(m.transmit(0, 0x01a1, 0), Reply::Value(254));
    let mut m = adjustable([100, 255, 255, 50], 0);
    base_setting(&mut m, 0x2b, 1);
    assert_eq!(drive(&mut m, 80), Reply::Value(0));
    base_setting(&mut m, 0x2b, 120);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(254),
        "DT7 minimum change did not operate virtual level"
    );
    base_setting(&mut m, 0x2b, 1);
    base_setting(&mut m, 0x2a, 40);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(0),
        "DT7 maximum change did not operate virtual level"
    );
    setting(&mut m, 0x0100, 0xe1, 0);
    assert_eq!(
        query(&mut m, 0, 0x01f2),
        Reply::Value(1),
        "DT7 raw upward zero was not clamped to one"
    );
}

#[test]
fn literal_dt7_one_enable_and_two_setters_require_original_pair_authority() {
    for interruption in [0u16, 0x01a0, 0x012b, 0xa301, 0xc107, 0xc108] {
        let mut m = adjustable([1, 255, 255, 0], 0);
        m.transmit(0, 0xa363, 0);
        m.transmit(0, 0xc107, 0);
        m.transmit(0, 0x01e1, 1);
        assert_eq!(
            m.snapshot()["buses"][0]["outputs"][0]["switching_state"]["thresholds"][0],
            1
        );
        if interruption != 0 {
            m.transmit(0, interruption, 2);
        }
        m.transmit(0, 0x01e1, 3);
        assert_eq!(
            query(&mut m, 0, 0x01f2),
            Reply::Value(if interruption == 0 { 99 } else { 1 }),
            "DT7 pair enable/intervening addressed command guard failed"
        );
    }
    for second_at in [101, 102] {
        let mut m = adjustable([1, 255, 255, 0], 0);
        m.transmit(0, 0xa363, 0);
        m.transmit(0, 0xc107, 0);
        m.transmit(0, 0x01e1, 1);
        m.transmit(0, 0x01e1, second_at);
        assert_eq!(
            query(&mut m, 0, 0x01f2),
            Reply::Value(if second_at == 101 { 99 } else { 1 }),
            "DT7 repeat window boundary changed"
        );
    }
    let mut m = adjustable([1, 255, 255, 0], 0);
    for frame in [0xa363, 0x01e1, 0x01e1] {
        m.transmit(0, frame, 0);
    }
    assert_eq!(
        query(&mut m, 0, 0x01f2),
        Reply::Value(1),
        "DT7 setter executed without enable"
    );
    assert_eq!(
        m.transmit(0, 0x01f2, 0),
        Reply::None,
        "DT7 query did not consume its enable"
    );
}

#[test]
fn literal_dt7_scope_unrelated_traffic_and_non_dt7_snapshots_are_preserved() {
    let mut m=Model::new(serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[
            {"id":"a","short_address":0,"device_type":7,"groups":32768,"switching":{"adjustable_thresholds":true}},
            {"id":"b","short_address":1,"device_type":7,"groups":32768,"switching":{"adjustable_thresholds":true}},
            {"id":"plain","short_address":2,"groups":32768,"level":90,"random_address":123},
            {"id":"outside","short_address":3,"device_type":7,"switching":{"adjustable_thresholds":true}},
            {"id":"fixed","short_address":4,"device_type":7}]},
        {"number":1,"outputs":[{"id":"other-bus","short_address":0,"device_type":7,"switching":{"adjustable_thresholds":true}}]}
    ]})).unwrap()).unwrap();
    let mut plain = m.snapshot()["buses"][0]["outputs"][2].clone();
    assert!(plain.get("switching").is_none() && plain.get("switching_state").is_none());
    for (frame, at) in [
        (0xa363, 0),
        (0xc107, 0),
        (0x01e1, 1),
        (0x03a0, 2),
        (0x01e1, 3),
    ] {
        m.transmit(0, frame, at);
    }
    assert_eq!(
        query(&mut m, 0, 0x01f2),
        Reply::Value(99),
        "DT7 traffic to another address canceled the pending pair"
    );
    setting(&mut m, 0x9f00, 0xe1, 88);
    for (bus, frame, value) in [
        (0, 0x01f2, 88),
        (0, 0x03f2, 88),
        (0, 0x07f2, 1),
        (0, 0x09f2, 1),
        (1, 0x01f2, 1),
    ] {
        assert_eq!(
            query(&mut m, bus, frame),
            Reply::Value(value),
            "DT7 setting escaped its group/bus selection"
        );
    }
    plain["dtr"][0] = json!(88);
    assert_eq!(
        m.snapshot()["buses"][0]["outputs"][2],
        plain,
        "DT7 work changed ordinary dimmer data beyond broadcast DTR0"
    );
    assert_eq!(
        query(&mut m, 0, 0xfff0),
        Reply::Collision,
        "DT7 distinct features did not collide"
    );
    assert_eq!(query(&mut m, 0, 0x01fe), Reply::None);
    m.transmit(0, 0xc108, 3);
    assert_eq!(
        m.transmit(0, 0x01f2, 3),
        Reply::None,
        "DT7 query accepted the wrong enabled device type"
    );
}

#[test]
fn literal_dt7_scenes_and_steps_calculate_from_virtual_level() {
    let mut m = adjustable([100, 255, 255, 50], 0);
    base_setting(&mut m, 0x2b, 1);
    base_setting(&mut m, 0x40, 80);
    base_setting(&mut m, 0x41, 40);
    assert_eq!(drive(&mut m, 150), Reply::Value(254));
    m.transmit(0, 0x0110, 0);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(254),
        "DT7 scene lost hysteresis state"
    );
    m.transmit(0, 0x0121, 0);
    m.transmit(0, 0x0121, 0);
    assert_eq!(
        m.transmit(0, 0x0198, 0),
        Reply::Value(80),
        "DT7 scene changed actual instead of virtual level"
    );
    m.transmit(0, 0x0111, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(0));
    m.transmit(0, 0x0112, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(0));
    assert_eq!(drive(&mut m, 99), Reply::Value(0));
    m.transmit(0, 0x0103, 0);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(254),
        "DT7 step-up used actual contact level"
    );
    assert_eq!(drive(&mut m, 51), Reply::Value(254));
    m.transmit(0, 0x0104, 0);
    assert_eq!(
        m.transmit(0, 0x01a0, 0),
        Reply::Value(0),
        "DT7 step-down used actual contact level"
    );
    m.transmit(0, 0x0105, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(254));
    m.transmit(0, 0x0106, 0);
    assert_eq!(m.transmit(0, 0x01a0, 0), Reply::Value(0));
    m.transmit(0, 0x0107, 0);
    m.transmit(0, 0x0108, 0);
    m.transmit(0, 0x0121, 0);
    m.transmit(0, 0x0121, 0);
    assert_eq!(m.transmit(0, 0x0198, 0), Reply::Value(1));
}

#[test]
fn dt7_scenario_profile_validation_is_strict_and_optional() {
    let valid = json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"relay","short_address":0,"device_type":7,"level":0}]}]});
    for (field, value) in [
        ("level", json!(1)),
        ("level", json!(255)),
        ("switching", json!({"thresholds":[0,255,255,0]})),
        ("switching", json!({"thresholds":[2,255,255,0]})),
        (
            "switching",
            json!({"adjustable_thresholds":true,"thresholds":[1,0,255,0]}),
        ),
    ] {
        let mut invalid = valid.clone();
        invalid["buses"][0]["outputs"][0][field] = value;
        let scenario: Scenario = serde_json::from_value(invalid).unwrap();
        assert!(
            scenario.validate().is_err(),
            "invalid DT7 profile was accepted"
        );
    }
    let mut invalid = valid.clone();
    invalid["buses"][0]["outputs"][0]["device_type"] = json!(6);
    invalid["buses"][0]["outputs"][0]["switching"] = json!({});
    assert!(serde_json::from_value::<Scenario>(invalid)
        .unwrap()
        .validate()
        .is_err());
    let mut invalid = valid.clone();
    invalid["buses"][0]["outputs"][0]["switching"] = json!({"unknown":true});
    assert!(serde_json::from_value::<Scenario>(invalid).is_err());
    let mut raw = valid;
    raw["buses"][0]["outputs"][0]["switching"] =
        json!({"adjustable_thresholds":true,"thresholds":[255,255,255,255],"gear_type":255});
    let mut m = Model::new(serde_json::from_value(raw).unwrap()).unwrap();
    assert_eq!(
        query(&mut m, 0, 0x01f7),
        Reply::Value(255),
        "raw DT7 gear type FF was lost"
    );
}
