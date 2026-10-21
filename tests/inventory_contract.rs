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
