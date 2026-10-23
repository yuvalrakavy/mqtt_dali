//! Literal feedback scopes and replies, independent of the production codec.
use super::*;
use serde_json::json;

fn physical() -> Model {
    let mut kinds = vec![1; 32];
    kinds[0] = 2;
    Model::new(serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":99}],"inputs":[
            {"id":"panel","short_address":0,"instances":kinds,"feedback":[{"instance":0},{"instance":31},{"instance":null}],"values":[{"instance":0,"resolution":16,"bytes":[171,205]}]},
            {"id":"edge","short_address":63,"instances":vec![1;32],"feedback":[{"instance":31}]},
            {"id":"new","short_address":null,"instances":[1],"feedback":[{"instance":0}]}
        ]},
        {"number":1,"inputs":[{"id":"other-bus","short_address":0,"instances":[1],"feedback":[{"instance":0}]}]}
    ]})).expect("feedback scenario profile is unsupported")).unwrap()
}

#[test]
fn literal_feedback_single_commands_queries_and_sensor_namespace() {
    let mut m = physical();
    assert_eq!(
        m.transmit_input(0, 0x01008e, 0),
        Reply::Value(32),
        "instance feature capability query missing"
    );
    assert_eq!(m.transmit_input(0, 0x01fe8e, 0), Reply::Value(32));
    assert_eq!(
        m.transmit_input(0, 0x01018e, 0),
        Reply::Value(254),
        "no feature must be FE"
    );
    assert_eq!(
        m.transmit_input(0, 0x01008f, 0),
        Reply::None,
        "singleton feature invented an enumeration continuation"
    );
    assert_eq!(m.transmit_input(0, 0x01204f, 0), Reply::Value(1));
    assert_eq!(m.transmit_input(0, 0x01204e, 0), Reply::None);
    m.transmit_input(0, 0x012010, 0);
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::Value(255),
        "single feedback activate did not apply"
    );
    assert_eq!(
        m.transmit_input(0, 0x01001e, 0),
        Reply::Value(0),
        "feedback activation changed absolute report timer"
    );
    // Ordinary absolute setter10 is repeated, unlike feature activate10.
    m.transmit_input(0, 0xc13007, 0);
    m.transmit_input(0, 0x010010, 0);
    assert_eq!(m.transmit_input(0, 0x01001e, 0), Reply::Value(0));
    m.transmit_input(0, 0x010010, 0);
    m.transmit_input(0, 0x010010, 0);
    assert_eq!(m.transmit_input(0, 0x01001e, 0), Reply::Value(7));
    assert_eq!(m.transmit_input(0, 0x01204e, 0), Reply::Value(255));
    m.transmit_input(0, 0x012010, 0);
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::Value(255),
        "repeated activation toggled feedback off"
    );
    m.transmit_input(0, 0x012011, 0);
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::None,
        "single feedback stop did not apply"
    );
    m.transmit_input(0, 0x012011, 0);
    assert_eq!(m.transmit_input(0, 0x01204e, 0), Reply::None);
    assert_eq!(
        m.transmit_input(0, 0x01001d, 0),
        Reply::Value(2),
        "feedback stop changed absolute deadtime"
    );
    m.transmit_input(0, 0x010063, 0);
    m.transmit_input(0, 0x010063, 0);
    assert_eq!(m.transmit_input(0, 0x010086, 0), Reply::None);
    m.transmit_input(0, 0x012010, 0);
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::Value(255),
        "disabled event input disabled independent feedback"
    );
    m.transmit_input(0, 0x01fc10, 0);
    assert_eq!(m.transmit_input(0, 0x01fc4f, 0), Reply::Value(1));
    assert_eq!(m.transmit_input(0, 0x01fc4e, 0), Reply::Value(255));
    m.transmit_input(0, 0x01fc11, 0);
    assert_eq!(m.transmit_input(0, 0x01fc4e, 0), Reply::None);
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::Value(255),
        "device feedback changed instance feedback"
    );
    assert_eq!(m.transmit_input(0, 0x01fe30, 0), Reply::Value(0));
    assert_eq!(
        m.transmit_input(0, 0x01fe4f, 0),
        Reply::None,
        "device selector aliased feature-device selector"
    );
    assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["dtr0"], 7);
}

#[test]
fn literal_feedback_last_instance_scope_latch_readdress_and_bus_isolation() {
    let mut m = physical();
    let before = m.snapshot();
    assert_eq!(m.transmit_input(0, 0x01008c, 0), Reply::Value(171));
    m.set_input_value("panel", 0, vec![18, 52]).unwrap();
    m.transmit_input(0, 0x013f10, 0);
    assert_eq!(
        m.transmit_input(0, 0x013f4e, 0),
        Reply::Value(255),
        "last feedback instance was not activated"
    );
    assert_eq!(
        m.transmit_input(0, 0x01204e, 0),
        Reply::None,
        "last feedback instance activated instance zero"
    );
    assert_eq!(
        m.transmit_input(0, 0x01008d, 0),
        Reply::Value(205),
        "feedback changed captured input latch"
    );
    m.transmit_input(0, 0x7f3f10, 0);
    assert_eq!(m.transmit_input(0, 0x7f3f4e, 0), Reply::Value(255));
    m.transmit_input(0, 0xc130ff, 0);
    for frame in [
        0x012110, 0x014010, 0x01a010, 0x01fd10, 0x01fc12, 0x012012, 0x01fc14, 0x013f14,
    ] {
        m.transmit_input(0, frame, 0);
    }
    assert_eq!(
        m.transmit_input(0, 0x01fe30, 0),
        Reply::Value(0),
        "feature setter aliased device short-address setter"
    );
    assert_eq!(m.transmit_input(0, 0x01018e, 0), Reply::Value(254));
    assert_eq!(
        m.transmit_input(0, 0x01214f, 0),
        Reply::None,
        "unconfigured feature was invented"
    );
    assert_eq!(m.transmit_input(0, 0x01204e, 0), Reply::None);
    m.set_address("panel", Some(17)).unwrap();
    assert_eq!(m.transmit_input(0, 0x013f4e, 0), Reply::None);
    assert_eq!(
        m.transmit_input(0, 0x233f4e, 0),
        Reply::Value(255),
        "readdress lost feedback state"
    );
    m.transmit_input(0, 0xfd2010, 0);
    assert_eq!(m.transmit_input(0, 0xfd204e, 0), Reply::Value(255));
    assert_eq!(
        m.transmit_input(0, 0x23204e, 0),
        Reply::None,
        "unaddressed feedback write changed addressed input"
    );
    m.transmit_input(0, 0xff2010, 0);
    assert_eq!(m.transmit_input(0, 0x23204e, 0), Reply::Value(255));
    assert_eq!(m.transmit_input(1, 0x01204e, 0), Reply::None);
    let after = m.snapshot();
    assert_eq!(after["buses"][0]["outputs"], before["buses"][0]["outputs"]);
    assert_eq!(after["buses"][1], before["buses"][1]);
    assert_eq!(
        after["buses"][0]["inputs"][0]["settings"],
        before["buses"][0]["inputs"][0]["settings"]
    );
}

#[test]
fn feedback_scenario_rejects_invalid_duplicate_profiles_and_preserves_legacy_shape() {
    let value = json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"panel","short_address":0,"instances":[1]}]}]});
    let scenario: Scenario = serde_json::from_value(value.clone()).unwrap();
    assert!(
        serde_json::to_value(&scenario).unwrap()["buses"][0]["inputs"][0]
            .get("feedback")
            .is_none()
    );
    let mut m = Model::new(scenario).unwrap();
    assert!(m.snapshot()["buses"][0]["inputs"][0]
        .get("feedback_state")
        .is_none());
    assert_eq!(
        m.transmit_input(0, 0x01008e, 0),
        Reply::Value(254),
        "legacy instance must report no feature FE"
    );
    assert_eq!(m.transmit_input(0, 0x01fe8e, 0), Reply::Value(254));
    for profiles in [
        json!([{"instance":0},{"instance":0}]),
        json!([{"instance":null},{"instance":null}]),
        json!([{"instance":1}]),
        json!([{"instance":32}]),
        json!([{"instance":0,"unknown":true}]),
        json!(null),
    ] {
        let mut bad = value.clone();
        bad["buses"][0]["inputs"][0]["feedback"] = profiles;
        assert!(
            serde_json::from_value::<Scenario>(bad)
                .map(|s| s.validate())
                .map_or(true, |v| v.is_err()),
            "invalid feedback scenario was accepted"
        );
    }
    // Device feedback does not require an input instance.
    let empty = json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"controller","short_address":1,"instances":[],"feedback":[{"instance":null}]}]}]});
    let mut m = Model::new(serde_json::from_value(empty).unwrap()).unwrap();
    assert_eq!(m.transmit_input(0, 0x03fe8e, 0), Reply::Value(32));
    assert_eq!(m.transmit_input(0, 0x03008e, 0), Reply::None);
}
