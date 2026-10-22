use super::*;

fn with_scene(first: &Model) -> Model {
    first
        .edit(
            first.inventory().revision,
            &[Edit::AddScene {
                bus: 0,
                scene: mqtt_dali::config::Scene {
                    slot: 0,
                    name: "Evening".into(),
                    description: None,
                },
            }],
        )
        .unwrap()
}

#[test]
fn scene_metadata_consumer_preserves_continuity_and_distinguishes_recreated_slot() {
    let first = with_scene(&model());
    let mut consumer = Consumer::default();
    consumer
        .accept(&first.state_bytes().unwrap())
        .expect("scene snapshot was rejected");
    let id = consumer.by_target("Bus1.Scenes.Evening").unwrap().id;
    let output_id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.4}));
    let renamed = first
        .edit(
            2,
            &[Edit::RenameScene {
                bus: 0,
                slot: 0,
                name: "Movie".into(),
            }],
        )
        .unwrap();
    let moved = renamed
        .edit(
            3,
            &[Edit::RenameBus {
                bus: 0,
                name: "Lighting".into(),
            }],
        )
        .unwrap();
    consumer.accept(&moved.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Lighting.Scenes.Movie").unwrap().id,
        id,
        "scene rename replaced its continuing consumer object"
    );
    assert_eq!(
        consumer.by_target("Lighting.Outputs.Light1").unwrap().id,
        output_id
    );
    assert_eq!(
        consumer
            .by_target("Lighting.Outputs.Light1")
            .unwrap()
            .intent,
        Some(json!({"Brightness":0.4}))
    );
    let described = moved
        .edit(
            4,
            &[Edit::SceneDescription {
                bus: 0,
                slot: 0,
                description: Some("Opaque native scene".into()),
            }],
        )
        .unwrap();
    consumer.accept(&described.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer
            .by_target("Lighting.Scenes.Movie")
            .unwrap()
            .record
            .binding_revision,
        4,
        "description-only edit rotated scene mapping authority"
    );
    let public: Value = serde_json::from_slice(&described.state_bytes().unwrap()).unwrap();
    assert_eq!(
        public["Inventory"]["buses"][0]["scenes"][0]["description"],
        "Opaque native scene"
    );
    let noop = described
        .edit(
            5,
            &[Edit::RenameScene {
                bus: 0,
                slot: 0,
                name: "Movie".into(),
            }],
        )
        .unwrap();
    assert_eq!(
        noop.inventory().revision,
        5,
        "no-op scene rename advanced revision"
    );
    let removed = noop
        .edit(5, &[Edit::RemoveScene { bus: 0, slot: 0 }])
        .unwrap();
    consumer.accept(&removed.state_bytes().unwrap()).unwrap();
    assert!(consumer.by_target("Lighting.Scenes.Movie").is_none());
    assert!(consumer.retired.contains(&id));
    let mut offline = Consumer::default();
    offline
        .accept(&removed.state_bytes().unwrap())
        .expect("removed scene history could not be replayed");
    let recreated = removed
        .edit(
            6,
            &[Edit::AddScene {
                bus: 0,
                scene: mqtt_dali::config::Scene {
                    slot: 0,
                    name: "Movie".into(),
                    description: None,
                },
            }],
        )
        .unwrap();
    consumer.accept(&recreated.state_bytes().unwrap()).unwrap();
    let device = consumer.by_target("Lighting.Scenes.Movie").unwrap();
    assert_ne!(
        device.id, id,
        "recreated scene reused its removed consumer identity"
    );
    assert!(
        device.intent.is_none() && device.observed.is_none(),
        "scene metadata fabricated a trigger or observation"
    );
    assert!(recreated
        .edit(
            7,
            &[Edit::AddScene {
                bus: 0,
                scene: mqtt_dali::config::Scene {
                    slot: 0,
                    name: "Duplicate".into(),
                    description: None
                }
            }]
        )
        .is_err());
}

#[test]
fn scene_metadata_consumer_validates_current_scene_slot_and_completeness_atomically() {
    let first = model();
    let added = with_scene(&first);
    let good: Value = serde_json::from_slice(&added.state_bytes().unwrap()).unwrap();
    let mut invalid = Vec::new();
    let mut value = good.clone();
    value["Inventory"]["buses"][0]["scenes"][0]["slot"] = json!(1);
    invalid.push(value);
    let mut value = good.clone();
    value["Inventory"]["buses"][0]["scenes"] = Value::Null;
    invalid.push(value);
    let mut value = good.clone();
    value["Inventory"]["buses"][0]
        .as_object_mut()
        .unwrap()
        .remove("scenes");
    invalid.push(value);
    let mut value = good.clone();
    value["Inventory"]["buses"][0]["scenes"][0]["color_representation"] = json!("temperature");
    invalid.push(value);
    let mut value = good.clone();
    value["Inventory"]["buses"][0]["scenes"][0]["description"] = json!(true);
    invalid.push(value);
    for value in invalid {
        let mut consumer = Consumer::default();
        consumer.accept(&first.state_bytes().unwrap()).unwrap();
        let ids: Vec<_> = consumer.devices.values().map(|d| d.id).collect();
        assert!(
            consumer
                .accept(&serde_json::to_vec(&value).unwrap())
                .is_err(),
            "consumer accepted malformed current scene metadata"
        );
        assert_eq!(consumer.revision, 1);
        assert_eq!(
            consumer.devices.values().map(|d| d.id).collect::<Vec<_>>(),
            ids
        );
    }
}

fn contaminate_scene_history(value: &mut Value, field: &str, data: Value) {
    for batch in value["identity_ledger"]["batches"].as_array_mut().unwrap() {
        for edge in batch["transitions"].as_array_mut().unwrap() {
            for side in ["from", "to"] {
                if edge[side]["location"]["kind"] == "scene" {
                    edge[side][field] = data.clone();
                }
            }
        }
    }
}

#[test]
fn scene_metadata_historical_reader_rejects_foreign_fields_after_scene_removal() {
    let added = with_scene(&model());
    let removed = added
        .edit(2, &[Edit::RemoveScene { bus: 0, slot: 0 }])
        .unwrap();
    for (field, data) in [
        ("instance_type", json!(1)),
        ("color_representation", json!("temperature")),
        (
            "members",
            json!([{"target":"Bus1.Outputs.Light1","revision":1}]),
        ),
    ] {
        let mut private = serde_json::to_value(removed.inventory()).unwrap();
        contaminate_scene_history(&mut private, field, data.clone());
        let inventory: Inventory = serde_json::from_value(private).unwrap();
        assert!(
            Model::new(inventory).is_err(),
            "producer accepted foreign metadata in retired scene history"
        );
        let mut public: Value = serde_json::from_slice(&removed.state_bytes().unwrap()).unwrap();
        contaminate_scene_history(&mut public["Inventory"], field, data);
        assert!(
            Consumer::default()
                .accept(&serde_json::to_vec(&public).unwrap())
                .is_err(),
            "consumer accepted foreign metadata in retired scene history"
        );
    }
}
