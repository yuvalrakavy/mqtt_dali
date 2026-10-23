use super::*;
use serde_json::json;

fn scenario() -> Scenario {
    serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"gear","short_address":0,"level":87,"memory":{"version":1,"banks":[{"bank":0,"bytes":[14,255,0,0,1,2,3,4,5,6,7,8,9,10,11]},{"bank":255,"bytes":[255,null,0,127]}]}}
    ],"inputs":[{"id":"input","short_address":0,"instances":[1],"memory":{"version":8,"banks":[{"bank":0,"bytes":[26,0,1,4,3,2,1,0,255,1,2,255]}]}}]}]})).expect("read-only memory scenario profile is unsupported")
}

#[test]
fn memory_native_model_preserves_ff_holes_space_and_saturated_pointer() {
    let scenario = scenario();
    scenario.validate().unwrap();
    let mut model = Model::new(scenario).unwrap();
    assert_eq!(model.transmit(0, 0x0197, 0), Reply::Value(1));
    assert_eq!(model.transmit(0, 0xa300, 0), Reply::None);
    assert_eq!(model.transmit(0, 0xc3ff, 0), Reply::None);
    assert_eq!(
        model.transmit(0, 0x01c5, 0),
        Reply::Value(255),
        "memory FF was mistaken for silence"
    );
    assert_eq!(model.transmit(0, 0x01c5, 0), Reply::None);
    assert_eq!(
        model.transmit(0, 0x01c5, 0),
        Reply::Value(0),
        "unreadable memory location did not advance its implemented-bank pointer"
    );
    assert_eq!(model.transmit(0, 0x01c5, 0), Reply::Value(127));
    assert_eq!(model.transmit(0, 0x0198, 0), Reply::Value(4));
    model.transmit(0, 0xa3fe, 0);
    assert_eq!(model.transmit(0, 0x01c5, 0), Reply::None);
    assert_eq!(model.transmit(0, 0x0198, 0), Reply::Value(255));
    model.transmit(0, 0x01c5, 0);
    assert_eq!(model.transmit(0, 0x0198, 0), Reply::Value(255));
    model.transmit(0, 0xa305, 0);
    model.transmit(0, 0xc301, 0);
    assert_eq!(model.transmit(0, 0x01c5, 0), Reply::None);
    assert_eq!(
        model.transmit(0, 0x0198, 0),
        Reply::Value(5),
        "absent bank executed a read"
    );
    assert_eq!(model.transmit_input(0, 0x01fe34, 0), Reply::Value(8));
    model.transmit_input(0, 0xc13100, 0);
    model.transmit_input(0, 0xc13003, 0);
    assert_eq!(model.transmit_input(0, 0x01fe3c, 0), Reply::Value(4));
    assert_eq!(
        model.transmit(0, 0x0198, 0),
        Reply::Value(5),
        "input memory changed output DTR0"
    );
    assert_eq!(
        model.transmit(0, 0x01a0, 0),
        Reply::Value(87),
        "memory read changed light level"
    );
}

#[test]
fn memory_profiles_are_optional_strict_and_survive_native_reads_and_capture() {
    let mut value = serde_json::to_value(scenario()).unwrap();
    let scenario: Scenario = serde_json::from_value(value.clone()).unwrap();
    let mut model = Model::new(scenario.clone()).unwrap();
    model.transmit(0, 0xa300, 0);
    model.transmit(0, 0xc300, 0);
    model.transmit(0, 0x01c5, 0);
    assert_eq!(
        serde_json::to_value(model.scenario(&scenario)).unwrap()["buses"][0]["outputs"][0]
            ["memory"],
        value["buses"][0]["outputs"][0]["memory"],
        "capture changed read-only memory"
    );
    let profile = value["buses"][0]["outputs"][0]["memory"].clone();
    for memory in [
        json!({"version":8,"banks":[{"bank":0,"bytes":[0]},{"bank":0,"bytes":[1]}]}),
        json!({"banks":[{"bank":1,"bytes":[]}]}),
        json!({"banks":[{"bank":1,"bytes":vec![Some(0u8);256]}]}),
    ] {
        value["buses"][0]["outputs"][0]["memory"] = memory;
        assert!(serde_json::from_value::<Scenario>(value.clone())
            .is_ok_and(|scenario| scenario.validate().is_err()));
    }
    value["buses"][0]["outputs"][0]["memory"] = profile;
    for space in ["outputs", "inputs"] {
        value["buses"][0][space][0]
            .as_object_mut()
            .unwrap()
            .remove("memory");
    }
    let scenario: Scenario = serde_json::from_value(value).unwrap();
    let mut model = Model::new(scenario.clone()).unwrap();
    assert_eq!(model.transmit(0, 0x0197, 0), Reply::Value(8));
    assert_eq!(model.transmit_input(0, 0x01fe34, 0), Reply::None);
    assert_eq!(model.transmit(0, 0x01c5, 0), Reply::None);
    assert_eq!(model.transmit_input(0, 0x01fe3c, 0), Reply::None);
    let captured = serde_json::to_value(model.scenario(&scenario)).unwrap();
    assert!(captured["buses"][0]["outputs"][0].get("memory").is_none());
    assert!(captured["buses"][0]["inputs"][0].get("memory").is_none());
}
