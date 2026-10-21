#[path = "support/inventory_consumer.rs"]
mod inventory_consumer;
use inventory_consumer::Consumer;
use mqtt_dali::config::Inventory;
use mqtt_dali::inventory::{Edit, Model};
use serde_json::{json, Value};

fn model() -> Model {
    let inventory: Inventory = serde_json::from_value(json!({
        "schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef",
        "name":"HouseController","buses":[{"bus":0,"name":"Bus1","enabled":true,
        "outputs":[{"short_address":0,"name":"Light1"},{"short_address":1,"name":"Light2"}],
        "groups":[{"group_address":0,"name":"All","members":[0,1]}]}]
    }))
    .unwrap();
    Model::new(inventory).unwrap()
}
fn renamed(model: &Model, address: u8, name: &str) -> Model {
    model
        .edit(
            model.inventory().revision,
            &[Edit::RenameOutput {
                bus: 0,
                address,
                name: name.into(),
            }],
        )
        .unwrap()
}

#[test]
fn a_consumer_rejects_split_bus_history_before_changing_any_identity() {
    let initial = model();
    let renamed = initial
        .edit(
            1,
            &[Edit::RenameBus {
                bus: 0,
                name: "Upper".into(),
            }],
        )
        .unwrap();
    let mut invalid: Value = serde_json::from_slice(&renamed.state_bytes().unwrap()).unwrap();
    invalid["Inventory"]["revision"] = 3.into();
    let batches = invalid["Inventory"]["identity_ledger"]["batches"]
        .as_array_mut()
        .unwrap();
    let mut later = batches[0]["transitions"]
        .as_array_mut()
        .unwrap()
        .split_off(1);
    for edge in &mut later {
        edge["to"]["binding_revision"] = 3.into();
    }
    batches.push(json!({"revision":3,"transitions":later}));
    for item in invalid["Inventory"]["buses"][0]["outputs"]
        .as_array_mut()
        .unwrap()
    {
        item["binding_revision"] = 3.into();
    }
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    assert!(
        consumer
            .accept(&serde_json::to_vec(&invalid).unwrap())
            .is_err(),
        "accepted split bus ancestor history"
    );
    assert_eq!(consumer.revision, 1);
    assert_eq!(consumer.by_target("Bus1.Outputs.Light1").unwrap().id, id);
    // The complete ancestor rename remains valid.
    consumer.accept(&renamed.state_bytes().unwrap()).unwrap();
    assert_eq!(consumer.by_target("Upper.Outputs.Light1").unwrap().id, id);
}

#[test]
fn a_consumer_validates_required_metadata_even_for_empty_buses() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/inventory-v1-rename.json")).unwrap();
    let initial = model();
    let empty = json!({"bus":1,"name":"Unused","enabled":false,"outputs":[],"groups":[]});
    for case in 0..15 {
        let mut invalid = fixture.clone();
        match case {
            0 => {
                invalid["Inventory"].as_object_mut().unwrap().remove("name");
            }
            1 => {
                invalid["Inventory"]["buses"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("enabled");
            }
            2 => {
                let mut bus = empty.clone();
                bus["bus"] = 255.into();
                invalid["Inventory"]["buses"]
                    .as_array_mut()
                    .unwrap()
                    .push(bus);
            }
            3 => invalid["Inventory"]["name"] = "Bad/#".into(),
            4 => {
                let mut bus = empty.clone();
                bus["name"] = "Bus1".into();
                invalid["Inventory"]["buses"]
                    .as_array_mut()
                    .unwrap()
                    .push(bus);
            }
            5 => {
                let mut bus = empty.clone();
                bus["name"] = "..".into();
                invalid["Inventory"]["buses"]
                    .as_array_mut()
                    .unwrap()
                    .push(bus);
            }
            6 => invalid["Inventory"]["description"] = 42.into(),
            7 => invalid["Inventory"]["buses"][0]["description"] = json!({}),
            8 => invalid["Inventory"]["buses"][0]["enabled"] = "true".into(),
            9 => invalid["Inventory"]["buses"][0]["outputs"][0]["description"] = false.into(),
            10 => invalid["Inventory"]["buses"][0]["groups"][0]["description"] = json!([]),
            11 => {
                let mut bus = empty.clone();
                bus.as_object_mut().unwrap().remove("enabled");
                invalid["Inventory"]["buses"]
                    .as_array_mut()
                    .unwrap()
                    .push(bus);
            }
            12 => invalid["Inventory"]["name"] = "_3_Sorted".into(),
            13 => invalid["Inventory"]["name"] = "Controller2_".into(),
            _ => invalid["Inventory"]["name"] = "Controller@".into(),
        }
        let mut consumer = Consumer::default();
        consumer.accept(&initial.state_bytes().unwrap()).unwrap();
        let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
        consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.5}));
        assert!(
            consumer
                .accept(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "accepted incomplete snapshot metadata case {case}"
        );
        assert_eq!(consumer.revision, 1);
        let device = consumer.by_target("Bus1.Outputs.Light1").unwrap();
        assert_eq!(device.id, id);
        assert_eq!(device.intent, Some(json!({"Brightness":0.5})));
        assert!(consumer.retired.is_empty());
    }
    let mut valid = fixture;
    valid["Inventory"]["buses"]
        .as_array_mut()
        .unwrap()
        .push(empty);
    Consumer::default()
        .accept(&serde_json::to_vec(&valid).unwrap())
        .unwrap();
}
#[test]
fn an_offline_consumer_preserves_binding_ids_through_renames_but_never_moves_intent_to_a_replacement(
) {
    let initial = model();
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    let original_id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.4}));
    // A logical Home object keeps this id throughout the hardware reconciliation.
    let logical_binding = original_id;
    let first = renamed(&initial, 0, "Kitchen");
    let returned = renamed(&first, 0, "Light1");
    consumer.accept(&returned.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().id,
        logical_binding
    );
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().intent,
        Some(json!({"Brightness":0.4}))
    );
    assert!(!consumer.accepts_state("Bus1.Outputs.Light1", "0123456789abcdef0123456789abcdef.1"));
    assert!(consumer.accepts_state("Bus1.Outputs.Light1", "0123456789abcdef0123456789abcdef.3"));
    let replacement = returned
        .edit(
            3,
            &[Edit::ReplaceOutput {
                bus: 0,
                output: returned.inventory().buses[0].outputs[0].clone(),
            }],
        )
        .unwrap();
    let final_name = renamed(&replacement, 0, "Kitchen");
    consumer.accept(&final_name.state_bytes().unwrap()).unwrap();
    let device = consumer.by_target("Bus1.Outputs.Kitchen").unwrap();
    assert_ne!(
        device.id, logical_binding,
        "address/name reuse stole the old logical binding"
    );
    assert!(
        device.intent.is_none(),
        "old desired intent was retokened onto a replacement"
    );
    assert!(consumer.retired.contains(&logical_binding));
    assert!(!consumer.accepts_state("Bus1.Outputs.Light1", "0123456789abcdef0123456789abcdef.3"));
    let mut fresh = Consumer::default();
    fresh.accept(&final_name.state_bytes().unwrap()).unwrap();
    assert!(
        fresh.devices.values().all(|d| d.intent.is_none()),
        "a new consumer adopted arbitrary retained intent"
    );
}

#[test]
fn a_consumer_rejects_partial_rewritten_or_new_epoch_documents_without_mutating_its_model() {
    let initial = model();
    let renamed = renamed(&initial, 0, "Kitchen");
    let mut consumer = Consumer::default();
    consumer.accept(&renamed.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Kitchen").unwrap().id;
    let newer = renamed
        .edit(
            2,
            &[Edit::Description {
                description: Some("upper floor".into()),
            }],
        )
        .unwrap();
    let valid: Value = serde_json::from_slice(&newer.state_bytes().unwrap()).unwrap();
    for kind in 0..5 {
        let mut invalid = valid.clone();
        match kind {
            0 => {
                invalid["Inventory"]["buses"][0]["outputs"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            1 => invalid["Inventory"]["identity_ledger"]["batches"] = json!([]),
            2 => {
                invalid["Inventory"]["inventory_epoch"] = "fedcba9876543210fedcba9876543210".into()
            }
            3 => {
                invalid["Inventory"]["identity_ledger"]["batches"][0]["transitions"][0]["from"]
                    ["binding_revision"] = 99.into()
            }
            _ => invalid["Inventory"]["revision"] = 2.into(),
        }
        assert!(
            consumer
                .accept(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "accepted invalid snapshot case {kind}"
        );
        assert_eq!(consumer.revision, 2);
        assert_eq!(consumer.by_target("Bus1.Outputs.Kitchen").unwrap().id, id);
    }
    consumer.accept(&newer.state_bytes().unwrap()).unwrap();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.revision, 3,
        "an older retained snapshot resurrected the old inventory"
    );
    consumer.accept(&newer.state_bytes().unwrap()).unwrap();
}

#[test]
fn atomic_swaps_retain_fixture_ids_and_explicit_removal_retires_only_that_device() {
    let initial = model();
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    let ids = [
        consumer.by_target("Bus1.Outputs.Light1").unwrap().id,
        consumer.by_target("Bus1.Outputs.Light2").unwrap().id,
    ];
    let group_id = consumer.by_target("Bus1.Groups.All").unwrap().id;
    let swapped = initial
        .edit(
            1,
            &[
                Edit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Light2".into(),
                },
                Edit::RenameOutput {
                    bus: 0,
                    address: 1,
                    name: "Light1".into(),
                },
            ],
        )
        .unwrap();
    consumer.accept(&swapped.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().id,
        ids[1]
    );
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light2").unwrap().id,
        ids[0]
    );
    let removed = swapped
        .edit(2, &[Edit::RemoveOutput { bus: 0, address: 0 }])
        .unwrap();
    consumer.accept(&removed.state_bytes().unwrap()).unwrap();
    assert!(consumer.by_target("Bus1.Outputs.Light2").is_none());
    assert!(consumer.retired.contains(&ids[0]));
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().id,
        ids[1]
    );
    assert_eq!(consumer.by_target("Bus1.Groups.All").unwrap().id, group_id);
    assert_eq!(consumer.devices.len(), 2);
}

#[test]
fn a_literal_wire_document_matches_the_producer_and_an_independent_consumers_rename() {
    let literal = include_bytes!("fixtures/inventory-v1-rename.json");
    let initial = model();
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"PowerState":true}));
    consumer.accept(literal).unwrap();
    assert_eq!(consumer.by_target("Bus1.Outputs.Kitchen").unwrap().id, id);
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Kitchen").unwrap().intent,
        Some(json!({"PowerState":true}))
    );
    let produced: Value =
        serde_json::from_slice(&renamed(&initial, 0, "Kitchen").state_bytes().unwrap()).unwrap();
    assert_eq!(produced, serde_json::from_slice::<Value>(literal).unwrap());
}
