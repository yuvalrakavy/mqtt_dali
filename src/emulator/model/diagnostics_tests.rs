//! Literal Part253 snapshot/model checks; no bridge decoder or frame helpers.
use super::*;
use serde_json::json;
fn scenario() -> Scenario {
    serde_json::from_str(include_str!(
        "../../../examples/emulator/diagnostics-i4.json"
    ))
    .expect("diagnostic scenario grammar")
}
fn read(model: &mut Model, offset: u8) -> Reply {
    model.transmit(0, 0xa300 | u16::from(offset), 0);
    model.transmit(0, 0x01c5, 0)
}
fn latch(model: &mut Model, value: u8) {
    for _ in 0..2 {
        model.transmit(0, 0x0181, 0);
    }
    model.transmit(0, 0xa302, 0);
    model.transmit(0, 0xc900 | u16::from(value), 0);
}
#[test]
fn diagnostics_model_full_banks_freeze_independently_and_export_only_configured_profiles() {
    let s = scenario();
    s.validate().expect("Part253 latch profile refused");
    for (bank, last, index) in [(205, 28, 0), (206, 32, 1)] {
        let mut model = Model::new(s.clone()).unwrap();
        let before = serde_json::to_value(model.scenario(&s)).unwrap();
        model.transmit(0, 0xc300 | bank, 0);
        let original = read(&mut model, last);
        latch(&mut model, 170);
        assert_eq!(read(&mut model, 2), Reply::Value(170));
        assert_eq!(
            serde_json::to_value(model.scenario(&s)).unwrap(),
            before,
            "runtime diagnostic latch leaked into scenario export"
        );
        model.buses[0].outputs[0]
            .config
            .memory
            .as_mut()
            .unwrap()
            .banks[index]
            .bytes[usize::from(last)] = Some(99);
        assert_eq!(
            read(&mut model, last),
            original,
            "diagnostic bank read leaked newer live payload"
        );
        let other = if bank == 205 { 206 } else { 205 };
        model.transmit(0, 0xc300 | other, 0);
        assert_eq!(
            read(&mut model, 2),
            Reply::Value(255),
            "diagnostic latch crossed banks"
        );
        model.transmit(0, 0xc300 | bank, 0);
        assert_eq!(read(&mut model, last), original);
        latch(&mut model, 255);
        assert_eq!(read(&mut model, 2), Reply::Value(255));
        assert_eq!(
            read(&mut model, last),
            Reply::Value(99),
            "diagnostic release retained frozen payload"
        );
        assert_eq!(model.transmit(0, 0x01a0, 0), Reply::Value(91));
        assert_eq!(
            model.buses[0].outputs[0]
                .config
                .memory
                .as_ref()
                .unwrap()
                .banks[index]
                .bytes[2],
            Some(255)
        );
    }
    let mut model = Model::new(s).unwrap();
    model.transmit(0, 0xc3cf, 0);
    latch(&mut model, 170);
    assert_eq!(
        read(&mut model, 2),
        Reply::Value(255),
        "static207 accepted a snapshot latch"
    );
}
#[test]
fn diagnostics_model_profile_bounds_are_bank_specific_and_input_scope_cannot_latch() {
    let s = scenario();
    s.validate().expect("valid diagnostic banks refused");
    let original = serde_json::to_value(s).unwrap();
    for (bank, length) in [(205, 30), (206, 34), (207, 8), (201, 16)] {
        let mut j = original.clone();
        j["buses"][0]["outputs"][0]["memory"]["banks"][0] =
            json!({"bank":bank,"latch":true,"bytes":vec![Some(0u8);length]});
        assert!(
            serde_json::from_value::<Scenario>(j).is_ok_and(|s| s.validate().is_err()),
            "unsupported diagnostic latch profile accepted"
        );
    }
    let mut j = original;
    j["buses"][0]["inputs"][0]["memory"] = j["buses"][0]["outputs"][0]["memory"].clone();
    assert!(
        serde_json::from_value::<Scenario>(j).is_ok_and(|s| s.validate().is_err()),
        "input scope accepted diagnostic latch"
    );
}
