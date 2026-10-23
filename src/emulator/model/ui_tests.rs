use super::*;
use crate::emulator::input::physical::Stimulus;
fn installation() -> Scenario {
    serde_json::from_value(serde_json::json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[{"id":"lamp","short_address":3,"level":100}],"inputs":[{"id":"panel","short_address":0,"instances":[1]},{"id":"sensor","short_address":2,"instances":[2,3,4],"values":[{"instance":0,"resolution":10,"bytes":[50,50]}]}]}]})).unwrap()
}
fn button(model: &mut Model, pressed: bool) -> Vec<(u8, u32)> {
    model
        .stimulate("panel", 0, Stimulus::Button { pressed })
        .unwrap();
    model.drain_events()
}
fn write_input(model: &mut Model, frame: u32, value: u8, now: u64) {
    model.transmit_input(0, 0xc13000 | u32::from(value), now);
    model.transmit_input(0, frame, now);
    model.transmit_input(0, frame, now);
}
#[test]
fn semantic_button_clock_enable_filter_and_source_scheme_are_native() {
    let mut model = Model::new(installation()).unwrap();
    assert_eq!(button(&mut model, true), vec![(0, 0x008001)]);
    model.advance_to(499);
    assert!(model.drain_events().is_empty());
    model.advance_to(500);
    assert_eq!(model.drain_events(), vec![(0, 0x008009)]);
    model.advance_to(660);
    assert_eq!(model.drain_events(), vec![(0, 0x00800b)]);
    assert_eq!(
        button(&mut model, false),
        vec![(0, 0x008000), (0, 0x00800c)]
    );
    write_input(&mut model, 0x010063, 0, 660);
    assert!(button(&mut model, true).is_empty());
    model.advance_to(1160);
    assert!(model.drain_events().is_empty());
    assert!(button(&mut model, false).is_empty());
    write_input(&mut model, 0x010062, 0, 1160);
    write_input(&mut model, 0x010067, 0, 1160);
    assert_eq!(button(&mut model, true), vec![(0, 0x828001)]);
    button(&mut model, false);
    write_input(&mut model, 0x010068, 0, 1160);
    assert!(button(&mut model, true).is_empty());
}
#[test]
fn semantic_sensor_initial_value_deadtime_and_disabled_reports() {
    let mut model = Model::new(installation()).unwrap();
    assert_eq!(
        model.snapshot()["buses"][0]["inputs"][1]["physical"][0]["raw_10"],
        200
    );
    model
        .stimulate("sensor", 0, Stimulus::Value { raw_10: 300 })
        .unwrap();
    assert_eq!(model.drain_events(), vec![(0, 0x04812c)]);
    model.advance_to(50);
    model
        .stimulate("sensor", 0, Stimulus::Value { raw_10: 400 })
        .unwrap();
    assert!(model.drain_events().is_empty());
    model.advance_to(99);
    assert!(model.drain_events().is_empty());
    model.advance_to(100);
    assert_eq!(model.drain_events(), vec![(0, 0x048190)]);
    write_input(&mut model, 0x050010, 1, 100); // Report every second.
    model.advance_to(200);
    model.drain_events();
    write_input(&mut model, 0x050063, 0, 200); // Disable before timer matures.
    model.advance_to(1200);
    assert!(model.drain_events().is_empty());
}

#[test]
fn semantic_pending_value_return_and_report_disable_retire_old_reports() {
    let mut model = Model::new(installation()).unwrap();
    model
        .stimulate("sensor", 0, Stimulus::Value { raw_10: 300 })
        .unwrap();
    model.drain_events();
    model.advance_to(50);
    model
        .stimulate("sensor", 0, Stimulus::Value { raw_10: 400 })
        .unwrap();
    model.advance_to(75);
    model
        .stimulate("sensor", 0, Stimulus::Value { raw_10: 300 })
        .unwrap();
    model.advance_to(100);
    assert!(
        model.drain_events().is_empty(),
        "old pending400 was reported after physical returned300"
    );
    write_input(&mut model, 0x050010, 1, 100);
    model.advance_to(101);
    write_input(&mut model, 0x050010, 0, 200);
    model.advance_to(1200);
    assert!(
        model.drain_events().is_empty(),
        "disabled report timer emitted its former scheduled event"
    );
}
#[test]
fn capture_roundtrip_preserves_native_settings_and_live_add_does_not_reset_them() {
    let start = installation();
    let mut model = Model::new(start.clone()).unwrap();
    model.transmit(0, 0xa32a, 0);
    model.transmit(0, 0x072b, 0);
    model.transmit(0, 0x072b, 0);
    write_input(&mut model, 0x010067, 1, 0);
    write_input(&mut model, 0x010000, 30, 0);
    write_input(&mut model, 0x050010, 12, 0);
    let capture = model.scenario(&start);
    capture.validate().unwrap();
    let encoded = serde_json::to_vec(&capture).unwrap();
    let capture: Scenario = serde_json::from_slice(&encoded).unwrap();
    let mut restored = Model::new(capture).unwrap();
    assert_eq!(restored.transmit(0, 0x07a2, 0), Reply::Value(42));
    assert_eq!(restored.transmit_input(0, 0x01008b, 0), Reply::Value(1));
    assert_eq!(restored.transmit_input(0, 0x01000a, 0), Reply::Value(30));
    assert_eq!(restored.transmit_input(0, 0x05001e, 0), Reply::Value(12));
    let add =
        serde_json::from_value(serde_json::json!({"id":"added","short_address":null})).unwrap();
    restored.add_output(0, add, &start).unwrap();
    assert_eq!(restored.transmit_input(0, 0x01000a, 0), Reply::Value(30));
    let snapshot = restored.snapshot();
    assert_ne!(
        snapshot["buses"][0]["outputs"][0]["random_address"],
        snapshot["buses"][0]["outputs"][1]["random_address"]
    );
}

#[test]
fn capture_keeps_physical_values_without_inventing_queried_resolution() {
    let start = installation();
    let mut model = Model::new(start.clone()).unwrap();
    model
        .stimulate("sensor", 2, Stimulus::Value { raw_10: 513 })
        .unwrap();
    let saved = model.scenario(&start);
    assert!(saved.buses[0].inputs[1]
        .values
        .iter()
        .all(|v| v.instance != 2));
    let restored = Model::new(saved).unwrap();
    assert_eq!(
        restored.snapshot()["buses"][0]["inputs"][1]["physical"][2]["raw_10"],
        513
    );
}

#[test]
fn capture_colour_actual_and_native_scene_memory_survive_roundtrip() {
    let mut start: Scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/colour-i4.json")).unwrap();
    start.buses[0].outputs[0]
        .colour
        .as_mut()
        .unwrap()
        .auto_activation = true;
    let mut model = Model::new(start.clone()).unwrap();
    // Stage350 mirek and store it with level60 in native scene0.
    for frame in [0xa35e, 0xc301, 0xc108, 0x07e7, 0xa33c, 0x0740, 0x0740] {
        model.transmit(0, frame, 0);
    }
    // Change actual Tc to300, leaving scene350 intact.
    for frame in [0xa32c, 0xc301, 0xc108, 0x07e7, 0xc108, 0x07e2] {
        model.transmit(0, frame, 0);
    }
    let capture = model.scenario(&start);
    capture.validate().unwrap();
    let serialized = serde_json::to_vec(&capture).unwrap();
    let mut restored = Model::new(serde_json::from_slice(&serialized).unwrap()).unwrap();
    assert_eq!(
        restored.snapshot()["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]
            ["value"],
        300
    );
    restored.transmit(0, 0x0710, 0);
    assert_eq!(
        restored.snapshot()["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]
            ["value"],
        350
    );
    assert_eq!(restored.transmit(0, 0x07a0, 0), Reply::Value(60));
}

#[test]
fn occupancy_hold_updates_physical_and_queried_state_on_one_clock() {
    let mut start = installation();
    start.buses[0].inputs[1].values.push(
        serde_json::from_value(serde_json::json!({"instance":1,"resolution":2,"bytes":[0]}))
            .unwrap(),
    );
    let mut model = Model::new(start).unwrap();
    write_input(&mut model, 0x050121, 0, 0);
    model
        .stimulate(
            "sensor",
            1,
            Stimulus::Occupancy {
                occupied: true,
                movement: true,
            },
        )
        .unwrap();
    model.drain_events();
    model.advance_to(100);
    model
        .stimulate(
            "sensor",
            1,
            Stimulus::Occupancy {
                occupied: false,
                movement: false,
            },
        )
        .unwrap();
    model.drain_events();
    assert_eq!(model.transmit_input(0, 0x05018c, 100), Reply::Value(255));
    model.advance_to(1099);
    assert_eq!(
        model.snapshot()["buses"][0]["inputs"][1]["physical"][1]["occupied"],
        true
    );
    model.advance_to(1100);
    assert_eq!(
        model.snapshot()["buses"][0]["inputs"][1]["physical"][1]["occupied"],
        false
    );
    assert_eq!(model.transmit_input(0, 0x05018c, 1100), Reply::Value(0));
}
