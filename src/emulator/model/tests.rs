use super::*;

fn scenario() -> Scenario {
    serde_json::from_str(
        r#"{"version":1,"seed":9,"clock":"frozen","buses":[
        {"number":0,"outputs":[{"id":"A","short_address":3,"random_address":256,"level":100},
        {"id":"B","short_address":null,"random_address":512,"level":90}]},
        {"number":1,"outputs":[{"id":"C","short_address":3,"level":20}]}]}"#,
    )
    .unwrap()
}

#[test]
fn literal_output_queries_address_spaces_and_mask_are_independent() {
    let mut model = Model::new(scenario()).unwrap();
    assert_eq!(
        model.transmit(0, 0x07a0, 0),
        Reply::Value(100),
        "short address level query did not reach physical fixture"
    );
    assert_eq!(model.transmit(1, 0x07a0, 1), Reply::Value(20));
    model.transmit(0, 0x062a, 2);
    model.transmit(0, 0x06ff, 3);
    assert_eq!(
        model.transmit(0, 0x07a0, 4),
        Reply::Value(42),
        "MASK overwrote level instead of retaining it"
    );
    assert_eq!(model.transmit(0, 0x0991, 5), Reply::None);
    assert_eq!(model.transmit(1, 0x07a0, 6), Reply::Value(20));
}

#[test]
fn configuration_requires_an_adjacent_repeat_within_the_total_window() {
    let mut model = Model::new(scenario()).unwrap();
    model.transmit(0, 0x0762, 0);
    assert_eq!(
        model.transmit(0, 0x07c0, 1),
        Reply::Value(0),
        "single configuration frame changed groups"
    );
    model.transmit(0, 0x0762, 10);
    model.transmit(0, 0x0762, 110);
    assert_eq!(model.transmit(0, 0x07c0, 111), Reply::Value(4));
    model.transmit(0, 0x0772, 120);
    model.transmit(0, 0x0772, 221);
    assert_eq!(model.transmit(0, 0x07c0, 222), Reply::Value(4));
    model.transmit(0, 0xa32a, 223);
    model.transmit(0, 0x0741, 224);
    model.transmit(0, 0x0741, 225);
    model.transmit(0, 0x0600, 226);
    model.transmit(0, 0x0711, 227);
    assert_eq!(model.transmit(0, 0x07a0, 228), Reply::Value(42));
    assert_eq!(model.transmit(0, 0x07b1, 229), Reply::Value(42));
}

#[test]
fn addressing_compares_a_range_but_only_programs_exact_eligible_matches() {
    let mut model = Model::new(scenario()).unwrap();
    model.transmit(0, 0xa5ff, 0);
    model.transmit(0, 0xa5ff, 1); // Only unaddressed B.
    model.transmit(0, 0xb100, 2);
    model.transmit(0, 0xb302, 3);
    model.transmit(0, 0xb501, 4); // Search 0x201; B=0x200 <= search.
    assert_eq!(model.transmit(0, 0xa900, 5), Reply::Value(255));
    model.transmit(0, 0xb70b, 6); // Not an exact match: must not program.
    assert_eq!(model.transmit(0, 0x0b91, 7), Reply::None);
    model.transmit(0, 0xb500, 8);
    model.transmit(0, 0xb70b, 9);
    assert_eq!(
        model.transmit(0, 0x0b91, 10),
        Reply::Value(255),
        "exact selected fixture did not receive its short address"
    );
    model.transmit(0, 0xab00, 11);
    assert_eq!(model.transmit(0, 0xa900, 12), Reply::None);
    assert_eq!(model.transmit(0, 0x0791, 13), Reply::Value(255));
}

#[test]
fn duplicate_addresses_update_every_fixture_and_conflicting_replies_collide() {
    let mut scenario = scenario();
    scenario.buses[0].outputs[1].short_address = Some(3);
    let mut model = Model::new(scenario).unwrap();
    assert_eq!(model.transmit(0, 0x07a0, 0), Reply::Collision);
    model.transmit(0, 0x0633, 1);
    assert_eq!(model.transmit(0, 0x07a0, 2), Reply::Value(0x33));
}

#[test]
fn scenario_validation_rejects_unsafe_ranges_and_unknown_fields() {
    let mut s = scenario();
    s.buses[0].outputs[0].short_address = Some(64);
    assert!(s.validate().is_err());
    s.buses[0].outputs[0].short_address = None;
    s.buses[0].outputs[0].scenes.pop();
    assert!(s.validate().is_err());
    assert!(serde_json::from_str::<Scenario>(r#"{"version":1,"buses":[],"invented":1}"#).is_err());
}

#[test]
fn binary_search_finds_each_fixture_without_using_the_bridge_inventory() {
    let mut s = scenario();
    s.buses[0].outputs[0].random_address = Some(0x801234);
    s.buses[0].outputs[1].random_address = Some(0xfffffe);
    let mut model = Model::new(s).unwrap();
    model.transmit(0, 0xa500, 0);
    model.transmit(0, 0xa500, 1);
    let mut time = 2;
    for (address, expected) in [(10, 0x801234), (11, 0xfffffe)] {
        let (mut low, mut high) = (0u32, 0xffffffu32);
        while low < high {
            let middle = low + (high - low) / 2;
            for frame in [
                0xb100 | ((middle >> 16) as u16),
                0xb300 | (((middle >> 8) & 255) as u16),
                0xb500 | ((middle & 255) as u16),
            ] {
                model.transmit(0, frame, time);
                time += 1;
            }
            if model.transmit(0, 0xa900, time) == Reply::Value(255) {
                high = middle;
            } else {
                low = middle + 1;
            }
            time += 1;
        }
        assert_eq!(
            low, expected,
            "COMPARE did not find the independent fixture's random address"
        );
        for frame in [
            0xb100 | ((low >> 16) as u16),
            0xb300 | (((low >> 8) & 255) as u16),
            0xb500 | ((low & 255) as u16),
            0xb700 | (address * 2 + 1),
            0xab00,
        ] {
            model.transmit(0, frame, time);
            time += 1;
        }
        assert_eq!(
            model.transmit(0, ((address * 2 + 1) << 8) | 0x91, time),
            Reply::Value(255)
        );
        time += 1;
    }
    assert_eq!(model.transmit(0, 0xa900, time), Reply::None);
}

#[test]
fn initialization_expires_on_model_time_and_randomization_is_seeded() {
    let mut a = Model::new(scenario()).unwrap();
    let mut b = Model::new(scenario()).unwrap();
    for model in [&mut a, &mut b] {
        model.transmit(0, 0xa500, 0);
        model.transmit(0, 0xa500, 1);
        model.transmit(0, 0xa700, 2);
        model.transmit(0, 0xa700, 3);
    }
    assert_eq!(a.snapshot(), b.snapshot());
    assert_eq!(a.transmit(0, 0xa900, 900000), Reply::Value(255));
    assert_eq!(
        a.transmit(0, 0xa900, 900001),
        Reply::None,
        "INITIALISE did not expire on model time"
    );
}

#[test]
fn step_up_does_not_ignite_and_reset_restores_all_modeled_persistent_defaults() {
    let mut model = Model::new(scenario()).unwrap();
    model.transmit(0, 0x0700, 0);
    model.transmit(0, 0x0703, 1);
    assert_eq!(
        model.transmit(0, 0x07a0, 2),
        Reply::Value(0),
        "STEP UP ignited an off fixture"
    );
    model.transmit(0, 0x0708, 3);
    assert_eq!(model.transmit(0, 0x07a0, 4), Reply::Value(1));
    model.transmit(0, 0xa32a, 5);
    for frame in [0x072c, 0x072c, 0x072d, 0x072d, 0xa303, 0x072f, 0x072f] {
        model.transmit(0, frame, 6);
    }
    assert_eq!(model.transmit(0, 0x07a5, 6), Reply::Value(3));
    assert_eq!(model.transmit(0, 0x07a3, 7), Reply::Value(42));
    model.transmit(0, 0x0720, 8);
    model.transmit(0, 0x0720, 9);
    assert_eq!(
        model.transmit(0, 0x07a3, 10),
        Reply::Value(254),
        "RESET retained programmed power-on level"
    );
    assert_eq!(
        model.transmit(0, 0x07a4, 11),
        Reply::Value(254),
        "RESET retained programmed failure level"
    );
    assert_eq!(model.transmit(0, 0x07a5, 12), Reply::Value(7));
    assert_eq!(model.transmit(0, 0x0791, 13), Reply::Value(255));
}

#[test]
fn min_max_scene_mask_and_unsupported_timed_ramps_have_explicit_effects() {
    let mut model = Model::new(scenario()).unwrap();
    for frame in [0xa314, 0x072a, 0x072a] {
        model.transmit(0, frame, 0);
    }
    assert_eq!(model.transmit(0, 0x07a0, 1), Reply::Value(20));
    for frame in [0xa3ff, 0x072a, 0x072a] {
        model.transmit(0, frame, 2);
    }
    assert_eq!(model.transmit(0, 0x07a1, 3), Reply::Value(254));
    for frame in [0xa30a, 0x072b, 0x072b, 0xa301, 0x0740, 0x0740, 0x0710] {
        model.transmit(0, frame, 4);
    }
    assert_eq!(model.transmit(0, 0x07a0, 5), Reply::Value(10));
    model.transmit(0, 0x0701, 6);
    model.transmit(0, 0x0702, 7);
    assert_eq!(
        model.transmit(0, 0x07a0, 8),
        Reply::Value(10),
        "unimplemented timed ramps were approximated as immediate steps"
    );
    assert_eq!(model.snapshot()["unsupported_commands"], 2);
    assert_eq!(model.transmit(0, 0x0797, 9), Reply::Value(8));
}

#[test]
fn targeted_reset_restores_defaults_preserves_dtrs_and_does_not_reset_another_fixture_search() {
    let mut s = scenario();
    s.buses[0].outputs[1].short_address = Some(4);
    let mut model = Model::new(s).unwrap();
    for frame in [
        0xa500, 0xa500, 0xb100, 0xb302, 0xb500, 0xa314, 0x072a, 0x072a, 0xa30a, 0x072b, 0x072b,
        0x0762, 0x0762, 0x0743, 0x0743, 0x072c, 0x072c, 0x072d, 0x072d, 0xa303, 0x072f, 0x072f,
        0xa337,
    ] {
        model.transmit(0, frame, 0);
    }
    let before = model.snapshot();
    model.transmit(0, 0x0720, 1);
    model.transmit(0, 0x0720, 2);
    let after = model.snapshot();
    let fixture = &after["buses"][0]["outputs"][0];
    for (key,expected) in serde_json::json!({"level":254,"random_address":0xffffff,"search":0xffffff,
        "short_address":3,"min_level":1,"max_level":254,"power_on_level":254,"failure_level":254,"fade":7,"groups":0,"dtr":[55,0,0]}).as_object().unwrap() {
        assert_eq!(&fixture[key],expected,"RESET violated the declared default/retention rule for {key}");
    }
    assert_eq!(fixture["scenes"], serde_json::json!(vec![255u8; 16]));
    assert_eq!(
        after["buses"][0]["outputs"][1], before["buses"][0]["outputs"][1],
        "targeted RESET changed another fixture's state/search"
    );
}
