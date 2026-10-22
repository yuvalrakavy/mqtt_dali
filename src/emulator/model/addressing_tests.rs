//! Literal Part 102/103 address writes; independent of production codecs.
use super::*;
use serde_json::json;

fn physical() -> Model {
    Model::new(serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":99,"groups":5,"scenes":[42,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]}],
         "inputs":[{"id":"panel","short_address":0,"instances":[2],"values":[{"instance":0,"resolution":10,"bytes":[171,192]}]},
                   {"id":"used","short_address":0,"instances":[1]},
                   {"id":"new","short_address":null,"instances":[1]}]},
        {"number":1,"outputs":[{"id":"other-light","short_address":0}],"inputs":[{"id":"other-panel","short_address":0,"instances":[1]}]}
    ]})).unwrap()).unwrap()
}

#[test]
fn input_unaddressed_broadcast_queries_exclude_addressed_gear_and_keep_instance_types() {
    let mut m = physical();
    assert_eq!(
        m.transmit_input(0, 0xfdfe35, 0),
        Reply::Value(1),
        "unaddressed input broadcast count was not answered"
    );
    assert_eq!(
        m.transmit_input(0, 0xfd0080, 0),
        Reply::Value(1),
        "unaddressed input broadcast included an addressed module"
    );
    assert_eq!(
        m.transmit_input(0, 0xfd008c, 0),
        Reply::None,
        "unaddressed input query used another module's value"
    );
    m.set_address("new", Some(3)).unwrap();
    assert_eq!(
        m.transmit_input(0, 0xfdfe35, 0),
        Reply::None,
        "addressed module kept answering broadcast-unaddressed"
    );
    assert_eq!(m.transmit_input(0, 0x07fe35, 0), Reply::Value(1));
}

#[test]
fn input_address_write_requires_repeat_raw_address_and_preserves_values_latches() {
    let mut m = physical();
    let before = m.snapshot();
    assert_eq!(m.transmit_input(0, 0x01008c, 0), Reply::Value(171));
    m.transmit_input(0, 0xc1303f, 1); // DTR0 is raw 63, unlike Part 102.
    m.transmit_input(0, 0x01fe14, 2);
    assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["short_address"], 0);
    m.transmit_input(0, 0x01fe14, 103); // A late pair does not program.
    assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["short_address"], 0);
    m.transmit_input(0, 0x01fe14, 104);
    assert_eq!(
        m.snapshot()["buses"][0]["inputs"][0]["short_address"],
        63,
        "ordinary input SetShortAddress did not program raw DTR0"
    );
    assert_eq!(
        m.transmit_input(0, 0x7f008d, 105),
        Reply::Value(192),
        "address write reset the value latch"
    );
    assert_eq!(m.transmit_input(0, 0x010080, 106), Reply::None);
    assert_eq!(
        m.snapshot()["buses"][0]["inputs"][2]["short_address"],
        serde_json::Value::Null,
        "short destination changed unaddressed peer"
    );
    assert_eq!(
        m.snapshot()["buses"][0]["outputs"],
        before["buses"][0]["outputs"]
    );
    assert_eq!(m.snapshot()["buses"][1], before["buses"][1]);
    // Values 64..254 are invalid input addresses, not shifted output codes.
    for value in [64u32, 127, 254] {
        m.transmit_input(0, 0xc13000 | value, 110);
        m.transmit_input(0, 0x7ffe14, 111);
        m.transmit_input(0, 0x7ffe14, 112);
        assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["short_address"], 63);
    }
    // An output packet between two input writes breaks the pair.
    m.transmit_input(0, 0xc130ff, 120);
    m.transmit_input(0, 0xfffe14, 121);
    m.transmit(0, 0x01a0, 122);
    m.transmit_input(0, 0xfffe14, 123);
    assert_eq!(m.snapshot()["buses"][0]["inputs"][0]["short_address"], 63);
    m.transmit_input(0, 0xfffe14, 124);
    assert!(
        m.snapshot()["buses"][0]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["short_address"].is_null()),
        "broadcast did not clear every addressed/unaddressed peer"
    );
    assert_eq!(
        m.snapshot()["buses"][0]["outputs"],
        before["buses"][0]["outputs"]
    );
}

#[test]
fn output_address_clear_preserves_live_groups_scenes_and_other_spaces() {
    let mut m = physical();
    let before = m.snapshot();
    for frame in [0xa3ff, 0xff80] {
        m.transmit(0, frame, 0);
    }
    assert_eq!(m.transmit(0, 0x0190, 0), Reply::Value(4)); // Intervening query breaks pair.
    m.transmit(0, 0xff80, 0);
    assert_eq!(m.snapshot()["buses"][0]["outputs"][0]["short_address"], 0);
    m.transmit(0, 0xff80, 1);
    let after = m.snapshot();
    assert!(after["buses"][0]["outputs"][0]["short_address"].is_null());
    for key in ["level", "groups", "scenes", "colour_state"] {
        assert_eq!(
            after["buses"][0]["outputs"][0][key],
            before["buses"][0]["outputs"][0][key]
        );
    }
    assert_eq!(after["buses"][0]["inputs"], before["buses"][0]["inputs"]);
    assert_eq!(after["buses"][1], before["buses"][1]);
}

#[test]
fn broadcast_clear_covers_all_64_short_addresses_in_each_independent_space() {
    let outputs: Vec<_> = (0..64)
        .map(|a| json!({"id":format!("light-{a}"),"short_address":a,"level":a+1}))
        .collect();
    let inputs: Vec<_> = (0..64)
        .map(|a| json!({"id":format!("input-{a}"),"short_address":a,"instances":[1]}))
        .collect();
    let mut m = Model::new(
        serde_json::from_value(
            json!({"version":1,"buses":[{"number":0,"outputs":outputs,"inputs":inputs}]}),
        )
        .unwrap(),
    )
    .unwrap();
    for frame in [0xa3ff, 0xff80, 0xff80] {
        m.transmit(0, frame, 0);
    }
    for address in 0..64 {
        assert_eq!(
            m.transmit(0, ((address * 2 + 1) << 8) | 0x90, 0),
            Reply::None
        );
    }
    assert_eq!(m.transmit_input(0, 0x7ffe30, 0), Reply::Value(0));
    for frame in [0xc130ff, 0xfffe14, 0xfffe14] {
        m.transmit_input(0, frame, 0);
    }
    for address in 0..64 {
        assert_eq!(
            m.transmit_input(0, ((address * 2 + 1) << 16) | 0xfe30, 0),
            Reply::None
        );
    }
    assert!(m.snapshot()["buses"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| g["level"].as_u64().unwrap() > 0));
}
