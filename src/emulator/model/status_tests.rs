use super::*;
use serde_json::json;
fn scenario() -> Scenario {
    serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":3,"level":100,"groups":5},{"id":"other","short_address":4,"level":50}],
        "inputs":[{"id":"panel","short_address":0,"instances":[1]}]}]})).unwrap()
}
#[test]
fn fixture_status_literal_queries_isolate_lamp_failure_without_changing_native_state() {
    let start = scenario();
    let mut model = Model::new(start.clone()).unwrap();
    let before = serde_json::to_value(model.scenario(&start)).unwrap();
    assert_eq!(model.transmit(0, 0x0790, 0), Reply::Value(4));
    model.set_lamp_failure("lamp", true).unwrap();
    assert_eq!(
        model.transmit(0, 0x0790, 0),
        Reply::Value(6),
        "native status omitted lamp failure bit"
    );
    assert_eq!(model.transmit(0, 0x07a0, 0), Reply::Value(100));
    assert_eq!(
        model.transmit(0, 0x0990, 0),
        Reply::Value(4),
        "lamp failure affected another fixture"
    );
    let mut after = serde_json::to_value(model.scenario(&start)).unwrap();
    assert_eq!(after["buses"][0]["outputs"][0]["lamp_failure"], true);
    after["buses"][0]["outputs"][0]
        .as_object_mut()
        .unwrap()
        .remove("lamp_failure");
    assert_eq!(
        after, before,
        "lamp failure changed unrelated native configuration"
    );
    for id in ["panel", "missing"] {
        assert!(model.set_lamp_failure(id, true).is_err());
    }
    assert_eq!(
        serde_json::to_value(model.scenario(&start)).unwrap()["buses"][0]["outputs"][1],
        before["buses"][0]["outputs"][1]
    );
    model.transmit(0, 0x0700, 0);
    assert_eq!(
        model.transmit(0, 0x0790, 0),
        Reply::Value(2),
        "failure while off was hidden"
    );
    let encoded = serde_json::to_vec(&model.scenario(&start)).unwrap();
    let mut restored = Model::new(serde_json::from_slice(&encoded).unwrap()).unwrap();
    assert_eq!(restored.transmit(0, 0x0790, 0), Reply::Value(2));
    restored.set_lamp_failure("lamp", false).unwrap();
    assert_eq!(restored.transmit(0, 0x0790, 0), Reply::Value(0));
    assert!(
        serde_json::to_value(restored.scenario(&start)).unwrap()["buses"][0]["outputs"][0]
            .get("lamp_failure")
            .is_none()
    );
}
#[test]
fn fixture_status_optional_scenario_boolean_preserves_old_files_and_rejects_invalid_types() {
    let encoded = serde_json::to_value(scenario()).unwrap();
    assert!(encoded["buses"][0]["outputs"][0]
        .get("lamp_failure")
        .is_none());
    for value in [json!(null), json!("true"), json!(1)] {
        let mut invalid = encoded.clone();
        invalid["buses"][0]["outputs"][0]["lamp_failure"] = value;
        assert!(serde_json::from_value::<Scenario>(invalid).is_err());
    }
}
