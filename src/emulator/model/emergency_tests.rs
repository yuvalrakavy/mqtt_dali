//! Independent literal DT1 traffic, never built with the bridge codec.
use super::*;
use serde_json::json;
fn scenario(profile: serde_json::Value) -> Scenario {
    serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[
        {"id":"emergency","short_address":0,"device_type":1,"level":91,"groups":2,"emergency":profile},
        {"id":"plain","short_address":1,"level":80}
    ]}]})).unwrap()
}
fn fixture(profile: serde_json::Value) -> Model {
    Model::new(scenario(profile)).unwrap()
}
fn query(m: &mut Model, frame: u16) -> Reply {
    let at = m.now_ms;
    m.transmit(0, 0xc101, at);
    m.transmit(0, frame, at)
}
fn setting(m: &mut Model, opcode: u16, raw: u8) {
    let at = m.now_ms;
    for frame in [
        0xa300 | u16::from(raw),
        0xc101,
        0x0100 | opcode,
        0x0100 | opcode,
    ] {
        m.transmit(0, frame, at);
    }
}
fn control(m: &mut Model, opcode: u16) {
    let at = m.now_ms;
    for frame in [0xc101, 0x0100 | opcode, 0x0100 | opcode] {
        m.transmit(0, frame, at);
    }
}
fn timing(m: &mut Model, selector: u8) -> Reply {
    let at = m.now_ms;
    m.transmit(0, 0xa300 | u16::from(selector), at);
    query(m, 0x01f2)
}
#[test]
fn literal_emergency_queries_settings_and_atomic_delay_keep_other_native_state() {
    let mut m = fixture(json!({}));
    let before = m.snapshot();
    for (frame, value) in [
        (0x01f1, 254),
        (0x01f6, 254),
        (0x01f7, 1),
        (0x01f8, 254),
        (0x01f9, 90),
        (0x01fa, 2),
        (0x01fb, 27),
        (0x01fc, 0),
        (0x01fd, 0),
    ] {
        assert_eq!(query(&mut m, frame), Reply::Value(value));
    }
    setting(&mut m, 0xe9, 123);
    assert_eq!(query(&mut m, 0x01f6), Reply::Value(123));
    setting(&mut m, 0xea, 0x12);
    setting(&mut m, 0xeb, 0x34);
    setting(&mut m, 0xec, 7);
    assert_eq!(timing(&mut m, 0), Reply::Value(0x12));
    assert_eq!(m.transmit(0, 0x019c, m.now_ms), Reply::Value(0x34));
    setting(&mut m, 0xea, 0x23);
    setting(&mut m, 0xeb, 0x45);
    setting(&mut m, 0xed, 8);
    assert_eq!(timing(&mut m, 2), Reply::Value(0x23));
    assert_eq!(m.transmit(0, 0x019c, m.now_ms), Reply::Value(0x45));
    assert_eq!(timing(&mut m, 0), Reply::Value(0x12));
    assert_eq!(m.transmit(0, 0x019c, m.now_ms), Reply::Value(0x34));
    for (set, selector, raw) in [(0xee, 6, 255), (0xef, 7, 255)] {
        setting(&mut m, set, raw);
        assert_eq!(timing(&mut m, selector), Reply::Value(raw));
    }
    assert_eq!(m.transmit(0, 0x01a0, m.now_ms), Reply::Value(91));
    let after = m.snapshot();
    for index in 0..2 {
        for field in [
            "short_address",
            "level",
            "groups",
            "scenes",
            "min_level",
            "max_level",
            "fade",
            "lamp_failure",
        ] {
            assert_eq!(
                after["buses"][0]["outputs"][index][field],
                before["buses"][0]["outputs"][index][field]
            );
        }
    }
    assert_eq!(query(&mut m, 0x03f1), Reply::None);
}
#[test]
fn literal_emergency_test_success_failure_and_stop_use_existing_clock() {
    let mut m = fixture(
        json!({"function_test_seconds":10,"duration_test_seconds":240,"duration_test_passes":false}),
    );
    control(&mut m, 0xe3);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(16));
    m.advance_to(9999);
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(0));
    m.advance_to(10000);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(2));
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(2));
    assert_eq!(query(&mut m, 0x01fc), Reply::Value(0));
    control(&mut m, 0xe4);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(32));
    m.advance_to(250000);
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(6));
    assert_eq!(query(&mut m, 0x01fc), Reply::Value(128));
    assert_eq!(query(&mut m, 0x01f3), Reply::Value(2));
    control(&mut m, 0xe3);
    control(&mut m, 0xe5);
    m.advance_to(999999);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(2));
    assert_eq!(
        query(&mut m, 0x01fd),
        Reply::Value(6),
        "Stop reset previous native result-valid flags"
    );
}
#[test]
fn literal_emergency_pending_timeout_flags_failure_without_cancelling() {
    for (timeout, threshold) in [(0, 900000u64), (1, 86400000)] {
        let mut m = fixture(json!({"test_ready":false,"execution_timeout_days":timeout}));
        control(&mut m, 0xe3);
        control(&mut m, 0xe4);
        assert_eq!(query(&mut m, 0x01fd), Reply::Value(48));
        m.advance_to(threshold - 1);
        assert_eq!(query(&mut m, 0x01fc), Reply::Value(0));
        m.advance_to(threshold);
        assert_eq!(query(&mut m, 0x01fc), Reply::Value(48));
        assert_eq!(
            query(&mut m, 0x01fd),
            Reply::Value(48),
            "timeout cancelled pending native tests"
        );
        control(&mut m, 0xe5);
        assert_eq!(query(&mut m, 0x01fd), Reply::Value(0));
        assert_eq!(query(&mut m, 0x01fc), Reply::Value(48));
    }
}
#[test]
fn literal_emergency_repeated_enable_authority_and_optional_features_are_independent() {
    for interruption in [None, Some(0x01a0), Some(0xa301), Some(0xc101), Some(0xc108)] {
        let mut m = fixture(json!({}));
        for frame in [0xa363, 0xc101, 0x01e9] {
            m.transmit(0, frame, 0);
        }
        if let Some(frame) = interruption {
            m.transmit(0, frame, 1);
        }
        m.transmit(0, 0x01e9, 2);
        assert_eq!(
            query(&mut m, 0x01f6),
            Reply::Value(if interruption.is_none() { 99 } else { 254 })
        );
    }
    let mut m = fixture(json!({"features":3}));
    setting(&mut m, 0xe9, 99);
    setting(&mut m, 0xea, 1);
    setting(&mut m, 0xeb, 2);
    setting(&mut m, 0xec, 3);
    assert_eq!(query(&mut m, 0x01f6), Reply::Value(254));
    assert_eq!(timing(&mut m, 0), Reply::Value(255));
    assert_eq!(m.transmit(0, 0x019c, 0), Reply::Value(255));
    assert_eq!(timing(&mut m, 4), Reply::Value(0));
    for frame in [0xa363, 0x01e9, 0x01e9] {
        m.transmit(0, frame, 0);
    }
    assert_eq!(query(&mut m, 0x01f6), Reply::Value(254));
    assert_eq!(
        m.transmit(0, 0x01f6, 0),
        Reply::None,
        "query reused consumed enable"
    );
}
#[test]
fn emergency_capture_excludes_active_timer_overlays_and_schedule_jump_is_bounded() {
    let source = scenario(
        json!({"function_interval_days":1,"function_delay_quarters":1,"duration_interval_weeks":1,"duration_delay_quarters":1,"function_test_seconds":10}),
    );
    let mut m = Model::new(source.clone()).unwrap();
    m.advance_to(900000);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(16));
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(32));
    let captured = m.scenario(&source);
    captured.validate().unwrap();
    let mut rebooted = Model::new(captured).unwrap();
    assert_eq!(query(&mut rebooted, 0x01fa), Reply::Value(2));
    assert_eq!(query(&mut rebooted, 0x01fd), Reply::Value(0));
    m.advance_to(u64::MAX - 1000000);
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(16));
    assert_eq!(m.snapshot()["buses"][0]["outputs"][0]["level"], 91);
    let mut original = Model::new(source).unwrap();
    assert_eq!(timing(&mut original, 0), Reply::Value(0));
    assert_eq!(original.transmit(0, 0x019c, 0), Reply::Value(1));
}
#[test]
fn emergency_profiles_are_optional_strict_and_require_dt1() {
    for profile in [
        json!({"minimum_level":0}),
        json!({"minimum_level":200,"maximum_level":100}),
        json!({"emergency_level":255}),
        json!({"duration_interval_weeks":98}),
        json!({"function_delay_quarters":65535}),
        json!({"function_test_seconds":0}),
    ] {
        assert!(scenario(profile).validate().is_err());
    }
    let mut wrong = scenario(json!({}));
    wrong.buses[0].outputs[0].device_type = 6;
    assert!(wrong.validate().is_err());
    assert!(serde_json::from_value::<Scenario>(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"a","short_address":0,"device_type":1,"emergency":{"surprise":true}}]}]})).is_err());
}
#[test]
fn emergency_pending_timeout_can_still_start_and_live_stimuli_are_captured() {
    let source = scenario(json!({"test_ready":false}));
    let mut m = Model::new(source.clone()).unwrap();
    control(&mut m, 0xe3);
    m.advance_to(900000);
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(16));
    m.set_emergency("emergency",serde_json::from_value(json!({"test_ready":true,"battery_charge":255,"failures":20,"function_test_passes":false})).unwrap()).unwrap();
    assert_eq!(query(&mut m, 0x01fa), Reply::Value(16));
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(0));
    assert_eq!(query(&mut m, 0x01f1), Reply::Value(255));
    m.advance_to(910000);
    assert_eq!(query(&mut m, 0x01fc), Reply::Value(84));
    assert_eq!(query(&mut m, 0x01fd), Reply::Value(2));
    let capture = m.scenario(&source);
    capture.validate().unwrap();
    let mut reset = Model::new(capture).unwrap();
    assert_eq!(query(&mut reset, 0x01fc), Reply::Value(84));
    assert_eq!(query(&mut reset, 0x01f1), Reply::Value(255));
    assert!(m.set_emergency("plain", Default::default()).is_err());
}
#[test]
fn emergency_added_after_clock_advance_starts_its_schedule_at_installation() {
    let source = scenario(json!({}));
    let mut m = Model::new(source.clone()).unwrap();
    m.advance_to(2000000);
    let output=serde_json::from_value(json!({"id":"later","short_address":2,"device_type":1,"emergency":{"function_interval_days":1,"function_delay_quarters":1}})).unwrap();
    m.add_output(0, output, &source).unwrap();
    m.advance_to(2000000);
    assert_eq!(
        query(&mut m, 0x05fa),
        Reply::Value(2),
        "new fixture inherited time before installation"
    );
    m.advance_to(2900000);
    assert_eq!(query(&mut m, 0x05fa), Reply::Value(16));
}
