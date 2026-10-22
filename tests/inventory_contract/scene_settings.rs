use super::*;

fn document(setting: Value) -> Value {
    let mut value = serde_json::to_value(model().inventory()).unwrap();
    value["buses"][0]["scenes"] = json!([{
        "slot":0,"name":"Evening","contents":[{
            "output_origin":{"target":"Bus1.Outputs.Light1","revision":1},
            "setting":setting
        }]
    }]);
    value
}

fn setting(color: Value) -> Value {
    json!({"level":{"kind":"value","value":0},"color":color})
}

#[test]
fn scene_settings_persist_native_masks_but_never_enter_public_inventory() {
    for color in [
        json!({"kind":"mask"}),
        json!({"kind":"temperature","value":null}),
        json!({"kind":"temperature","value":65534}),
        json!({"kind":"xy","x":0,"y":null}),
        json!({"kind":"rgbwaf","channels":[0,null,254,null,null,null],"control":128}),
        json!({"kind":"rgbwaf","channels":[null,null,null,null,null,null],"control":null}),
    ] {
        let value = document(setting(color));
        let inventory: Inventory =
            serde_json::from_value(value.clone()).expect("valid private scene row was rejected");
        let model = Model::new(inventory).unwrap();
        let reopened = Model::new(
            serde_json::from_slice(&serde_json::to_vec(model.inventory()).unwrap()).unwrap(),
        )
        .unwrap();
        let disk = serde_json::to_value(reopened.inventory()).unwrap();
        assert_eq!(disk["buses"][0]["scenes"], value["buses"][0]["scenes"]);
        let public: Value = serde_json::from_slice(&reopened.state_bytes().unwrap()).unwrap();
        assert!(
            public["Inventory"]["buses"][0]["scenes"][0]
                .get("contents")
                .is_none(),
            "private scene contents leaked into public Inventory"
        );
        Consumer::default()
            .accept(&reopened.state_bytes().unwrap())
            .expect("opaque scene snapshot broke independent consumer");
        let history = serde_json::to_string(reopened.ledger()).unwrap();
        assert!(!history.contains("setting") && !history.contains("contents"));
    }
}

#[test]
fn scene_settings_reject_missing_mask_attributes_reserved_values_and_invalid_references() {
    let valid = setting(json!({"kind":"temperature","value":null}));
    // Positive control ensures rejection is testing the setting, not an unknown contents field.
    let good: Inventory = serde_json::from_value(document(valid.clone()))
        .expect("valid private scene row was rejected");
    Model::new(good).unwrap();
    for invalid in [
        json!({"level":{"kind":"value","value":255},"color":{"kind":"mask"}}),
        json!({"level":{"kind":"mask"}}),
        setting(json!({"kind":"temperature"})),
        setting(json!({"kind":"temperature","value":0})),
        setting(json!({"kind":"temperature","value":65535})),
        setting(json!({"kind":"xy","x":null})),
        setting(json!({"kind":"xy","x":65535,"y":null})),
        setting(json!({"kind":"rgbwaf","channels":[null,null,null,null,null,null]})),
        setting(json!({"kind":"rgbwaf","channels":[255,null,null,null,null,null],"control":null})),
        setting(json!({"kind":"rgbwaf","channels":[null,null,null,null,null,null],"control":192})),
        setting(json!({"kind":"mask","value":null})),
    ] {
        assert!(
            serde_json::from_value::<Inventory>(document(invalid.clone())).is_err(),
            "invalid native scene setting accepted: {invalid}"
        );
    }
    for origin in [
        json!({"target":"Bus1.Outputs.Light1","revision":2}),
        json!({"target":"Bus1.Groups.All","revision":1}),
        json!({"target":"Bus1.Outputs.Missing","revision":1}),
    ] {
        let mut value = document(valid.clone());
        value["buses"][0]["scenes"][0]["contents"][0]["output_origin"] = origin;
        let parsed: Inventory = serde_json::from_value(value).unwrap();
        assert!(
            Model::new(parsed).is_err(),
            "invalid scene Origin was accepted"
        );
    }
    let mut value = document(valid.clone());
    let row = value["buses"][0]["scenes"][0]["contents"][0].clone();
    value["buses"][0]["scenes"][0]["contents"]
        .as_array_mut()
        .unwrap()
        .push(row);
    assert!(
        Model::new(serde_json::from_value(value).unwrap()).is_err(),
        "duplicate scene row accepted"
    );
    let mut value = document(valid);
    value["buses"][0]["scenes"][0]["contents"] = Value::Null;
    assert!(serde_json::from_value::<Inventory>(value).is_err());
}

#[test]
fn scene_settings_follow_continuing_origins_and_retire_on_replacement_or_removal() {
    let initial =
        Model::new(serde_json::from_value(document(setting(json!({"kind":"mask"})))).unwrap())
            .unwrap();
    let origin = mqtt_dali::inventory::Origin {
        target: "Bus1.Outputs.Light1".into(),
        revision: 1,
    };
    let desired = serde_json::from_value(setting(json!({"kind":"xy","x":null,"y":65534}))).unwrap();
    let edit = Edit::SceneRow {
        bus: 0,
        slot: 0,
        output_origin: origin.clone(),
        setting: desired,
    };
    let changed = initial.edit(1, std::slice::from_ref(&edit)).unwrap();
    assert_eq!(changed.inventory().revision, 2);
    assert_eq!(
        changed.projections(),
        initial.projections(),
        "private setting changed binding generations"
    );
    assert_eq!(
        changed.ledger(),
        initial.ledger(),
        "private data appeared in public history"
    );
    assert_eq!(
        changed.edit(2, &[edit]).unwrap().inventory(),
        changed.inventory(),
        "identical desired row changed revision"
    );
    let moved = changed
        .edit(
            2,
            &[
                Edit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Kitchen".into(),
                },
                Edit::RenameBus {
                    bus: 0,
                    name: "Lighting".into(),
                },
                Edit::ReaddressOutputs {
                    bus: 0,
                    addresses: vec![(0, 1), (1, 0)],
                },
                Edit::RenameScene {
                    bus: 0,
                    slot: 0,
                    name: "Movie".into(),
                },
            ],
        )
        .unwrap();
    assert_eq!(
        moved.inventory().buses[0].scenes[0].contents[0].output_origin,
        origin
    );
    assert_eq!(
        moved.inventory().buses[0].scenes[0].contents[0].setting,
        desired
    );
    Model::new(serde_json::from_slice(&serde_json::to_vec(moved.inventory()).unwrap()).unwrap())
        .unwrap();
    let other_removed = moved
        .edit(3, &[Edit::RemoveOutput { bus: 0, address: 0 }])
        .unwrap();
    assert_eq!(
        other_removed.inventory().buses[0].scenes[0].contents.len(),
        1,
        "removing another fixture cleared selected expectation"
    );
    let replaced = moved
        .edit(
            3,
            &[Edit::ReplaceOutput {
                bus: 0,
                output: moved.inventory().buses[0]
                    .outputs
                    .iter()
                    .find(|o| o.short_address == 1)
                    .unwrap()
                    .clone(),
            }],
        )
        .unwrap();
    assert!(
        replaced.inventory().buses[0].scenes[0].contents.is_empty(),
        "replacement inherited retired fixture settings"
    );
    let removed = moved
        .edit(3, &[Edit::RemoveOutput { bus: 0, address: 1 }])
        .unwrap();
    assert!(removed.inventory().buses[0].scenes[0].contents.is_empty());
    assert!(
        removed
            .edit(
                4,
                &[Edit::SceneRow {
                    bus: 0,
                    slot: 0,
                    output_origin: origin,
                    setting: desired
                }]
            )
            .is_err(),
        "retired Origin was readmitted"
    );
    let scene_removed = moved
        .edit(3, &[Edit::RemoveScene { bus: 0, slot: 0 }])
        .unwrap();
    assert!(scene_removed.inventory().buses[0].scenes.is_empty());
}
