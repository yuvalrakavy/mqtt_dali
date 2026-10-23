//! Independent literal Part102 memory-latch behavior; no bridge codec imports.
use super::*;
use serde_json::json;
fn scenario() -> Scenario {
    serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[
            {"id":"meter","short_address":0,"level":87,"memory":{"banks":[{"bank":202,"latch":true,"bytes":[15,0,255,1,253,0,0,0,1,226,64,255,0,0,1,169]}]}},
            {"id":"other","short_address":1,"level":42,"memory":{"banks":[{"bank":202,"latch":true,"bytes":[15,0,255,1,0,0,0,0,0,0,99,0,0,0,0,7]}]}}
        ]},
        {"number":1,"outputs":[{"id":"another-bus","short_address":0,"memory":{"banks":[{"bank":202,"latch":true,"bytes":[15,0,255,1,0,0,0,0,0,0,77,0,0,0,0,5]}]}}]}
    ]})).expect("energy latch scenario profile was rejected")
}
fn send(model: &mut Model, frame: u16) -> Reply {
    model.transmit(0, frame, 0)
}
fn enable(model: &mut Model, address: u8) {
    for _ in 0..2 {
        assert_eq!(send(model, (u16::from(address) << 9) | 0x0181), Reply::None);
    }
}
fn read(model: &mut Model, address: u8, offset: u8) -> Reply {
    send(model, 0xa300 | u16::from(offset));
    send(model, (u16::from(address) << 9) | 0x01c5)
}
fn write_latch(model: &mut Model, address: u8, value: u8) {
    enable(model, address);
    send(model, 0xa302);
    assert_eq!(send(model, 0xc900 | u16::from(value)), Reply::None);
}
#[test]
fn energy_model_latch_requires_pair_and_freezes_payload_until_release() {
    let s = scenario();
    s.validate().unwrap();
    let mut model = Model::new(s.clone()).unwrap();
    let profile = serde_json::to_value(model.scenario(&s)).unwrap();
    send(&mut model, 0xc3ca);
    send(&mut model, 0xa302);
    send(&mut model, 0xc9aa);
    assert_eq!(
        read(&mut model, 0, 2),
        Reply::Value(255),
        "disabled memory write latched data"
    );
    send(&mut model, 0x0181);
    send(&mut model, 0xa302);
    send(&mut model, 0xc9aa);
    assert_eq!(
        read(&mut model, 0, 2),
        Reply::Value(255),
        "single ENABLE WRITE MEMORY was accepted"
    );
    write_latch(&mut model, 0, 0xaa);
    assert_eq!(
        read(&mut model, 0, 2),
        Reply::Value(170),
        "repeated enable/latch did not freeze bank"
    );
    assert_eq!(
        read(&mut model, 1, 2),
        Reply::Value(255),
        "latch reached another fixture"
    );
    let bytes = &mut model.buses[0].outputs[0]
        .config
        .memory
        .as_mut()
        .unwrap()
        .banks[0]
        .bytes;
    bytes[10] = Some(65);
    bytes[15] = Some(170);
    assert_eq!(
        read(&mut model, 0, 10),
        Reply::Value(64),
        "energy read leaked newer live payload"
    );
    assert_eq!(
        read(&mut model, 0, 15),
        Reply::Value(169),
        "power read leaked newer live payload"
    );
    assert_eq!(read(&mut model, 1, 10), Reply::Value(99));
    write_latch(&mut model, 0, 0xff);
    assert_eq!(read(&mut model, 0, 2), Reply::Value(255));
    assert_eq!(
        read(&mut model, 0, 10),
        Reply::Value(65),
        "unlatch retained old snapshot"
    );
    assert_eq!(read(&mut model, 0, 15), Reply::Value(170));
    assert_eq!(send(&mut model, 0x01a0), Reply::Value(87));
    let captured = serde_json::to_value(model.scenario(&s)).unwrap();
    assert_eq!(
        captured["buses"][0]["outputs"][0]["memory"]["banks"][0]["bytes"][2], 255,
        "latch programmed profile bytes"
    );
    assert_eq!(
        captured["buses"][0]["outputs"][1]["memory"],
        profile["buses"][0]["outputs"][1]["memory"]
    );
    model.transmit(1, 0xc3ca, 0);
    model.transmit(1, 0xa30a, 0);
    assert_eq!(
        model.transmit(1, 0x01c5, 0),
        Reply::Value(77),
        "latch crossed bus boundary"
    );
}
#[test]
fn energy_model_write_enable_reset_precedes_repeat_return_and_preserves_dtr_exceptions() {
    let mut model = Model::new(scenario()).unwrap();
    send(&mut model, 0xc3ca);
    for unrelated in [0x01a0, 0x01c5, 0x0120] {
        enable(&mut model, 0);
        send(&mut model, unrelated);
        send(&mut model, 0xa302);
        send(&mut model, 0xc9aa);
        assert_eq!(
            read(&mut model, 0, 2),
            Reply::Value(255),
            "unrelated/repeat-first frame retained write enable: {unrelated:04X}"
        );
    }
    enable(&mut model, 0);
    send(&mut model, 0xa302);
    send(&mut model, 0xc3ca);
    send(&mut model, 0xc500);
    assert_eq!(send(&mut model, 0x0198), Reply::Value(2));
    assert_eq!(send(&mut model, 0x019c), Reply::Value(202));
    assert_eq!(send(&mut model, 0x019d), Reply::Value(0));
    send(&mut model, 0xc9aa);
    assert_eq!(
        read(&mut model, 0, 2),
        Reply::Value(170),
        "DTR exceptions disabled memory writes"
    );
    write_latch(&mut model, 0, 0xff);
    // Unsupported manufacturer writes never alter raw profile data.
    let before = serde_json::to_value(&model.buses[0].outputs[0].config.memory).unwrap();
    enable(&mut model, 0);
    send(&mut model, 0xa302);
    send(&mut model, 0xc955);
    enable(&mut model, 0);
    send(&mut model, 0xa303);
    send(&mut model, 0xc902);
    assert_eq!(
        serde_json::to_value(&model.buses[0].outputs[0].config.memory).unwrap(),
        before,
        "snapshot support enabled arbitrary memory programming"
    );
}
#[test]
fn energy_model_latch_profile_is_opt_in_bounded_and_output_only() {
    let s = scenario();
    let original = serde_json::to_value(&s).unwrap();
    for (bank, bytes) in [
        (1, vec![Some(0u8); 16]),
        (205, vec![Some(0); 16]),
        (202, vec![Some(0); 17]),
    ] {
        let mut j = original.clone();
        j["buses"][0]["outputs"][0]["memory"]["banks"][0] =
            json!({"bank":bank,"latch":true,"bytes":bytes});
        assert!(
            serde_json::from_value::<Scenario>(j).is_ok_and(|s| s.validate().is_err()),
            "unsupported latch profile accepted"
        );
    }
    let mut j = original.clone();
    j["buses"][0]["inputs"] = json!([{"id":"input","short_address":0,"instances":[1],"memory":j["buses"][0]["outputs"][0]["memory"]}]);
    assert!(
        serde_json::from_value::<Scenario>(j).is_ok_and(|s| s.validate().is_err()),
        "input module accepted output snapshot profile"
    );
    let mut j = original.clone();
    j["buses"][0]["outputs"][0]["memory"]["banks"][0]
        .as_object_mut()
        .unwrap()
        .remove("latch");
    let old: Scenario = serde_json::from_value(j).unwrap();
    old.validate().unwrap();
    let mut model = Model::new(old.clone()).unwrap();
    send(&mut model, 0xc3ca);
    write_latch(&mut model, 0, 0xaa);
    assert_eq!(
        read(&mut model, 0, 2),
        Reply::Value(255),
        "old raw profile became writable"
    );
    assert!(
        serde_json::to_value(model.scenario(&old)).unwrap()["buses"][0]["outputs"][0]["memory"]
            ["banks"][0]
            .get("latch")
            .is_none()
    );
}
