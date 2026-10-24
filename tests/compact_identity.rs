#[allow(dead_code)]
#[path = "support/inventory_consumer.rs"]
mod inventory_consumer;

use inventory_consumer::Consumer;
use mqtt_dali::config::Inventory;
use mqtt_dali::inventory::{Edit, Model};
use serde_json::{json, Value};

fn initial() -> Model {
    let inventory: Inventory = serde_json::from_value(json!({
        "schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef",
        "name":"HouseController","buses":[{"bus":0,"name":"Bus1","enabled":true,
        "outputs":[{"short_address":0,"name":"Light1"},{"short_address":1,"name":"Light2"}],
        "groups":[{"group_address":0,"name":"All","members":[0,1]}]}]
    }))
    .unwrap();
    Model::new(inventory).unwrap()
}

fn rename(model: Model, name: &str) -> Model {
    model
        .edit(
            model.inventory().revision,
            &[Edit::RenameOutput {
                bus: 0,
                address: 0,
                name: name.into(),
            }],
        )
        .unwrap()
}

#[test]
fn twelve_hundred_real_edits_have_bounded_storage_and_preserve_offline_bindings() {
    let mut model = initial();
    let mut consumer = Consumer::default();
    consumer.accept(&model.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.4}));
    for index in 0..1200 {
        model = rename(model, if index % 2 == 0 { "Kitchen" } else { "Light1" });
        assert!(
            model.ledger().batches.len() <= mqtt_dali::inventory::MAX_HISTORY_BATCHES,
            "diagnostic history exceeded its batch budget"
        );
        if index % 47 == 0 {
            model = Model::new(
                serde_json::from_slice(&serde_json::to_vec(model.inventory()).unwrap()).unwrap(),
            )
            .unwrap();
        }
    }
    let state = model.state_bytes().unwrap();
    let document: Value = serde_json::from_slice(&state).unwrap();
    assert_eq!(
        document["Inventory"]["schema_version"], 2,
        "producer still publishes replay-dependent inventory"
    );
    assert!(
        document["Inventory"].get("identity_ledger").is_none(),
        "MQTT inventory contains past renames"
    );
    assert_eq!(
        document["Inventory"]["identity_snapshot"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        state.len() < 8 * 1024,
        "current snapshot grew with past renames: {}",
        state.len()
    );
    let config = serde_json::to_vec(model.inventory()).unwrap();
    assert!(
        config.len() < 64 * 1024,
        "saved diagnostic history is unbounded: {}",
        config.len()
    );
    consumer.accept(&state).unwrap();
    let current = consumer.by_target("Bus1.Outputs.Light1").unwrap();
    assert_eq!(
        current.id, id,
        "offline device was recreated after history compaction"
    );
    assert_eq!(
        current.intent,
        Some(json!({"Brightness":0.4})),
        "ordinary rename-back lost compatible intent"
    );
}

#[test]
fn incompatible_change_and_back_remains_revoked_after_compaction_and_restart() {
    let mut model = initial();
    let mut consumer = Consumer::default();
    consumer.accept(&model.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.4}));
    for representation in [Some(mqtt_dali::config::ColorRepresentation::Rgbwaf), None] {
        model = model
            .edit(
                model.inventory().revision,
                &[Edit::OutputColorRepresentation {
                    bus: 0,
                    address: 0,
                    representation,
                }],
            )
            .unwrap();
    }
    for index in 0..80 {
        model = rename(model, if index % 2 == 0 { "Kitchen" } else { "Light1" });
    }
    let restored = Model::new(
        serde_json::from_slice(&serde_json::to_vec(model.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    consumer.accept(&restored.state_bytes().unwrap()).unwrap();
    let current = consumer.by_target("Bus1.Outputs.Light1").unwrap();
    assert_eq!(current.id, id);
    assert_eq!(
        current.intent, None,
        "forgotten color change resurrected incompatible control intent"
    );
}

#[test]
fn compact_snapshot_validation_is_atomic_and_rejects_identity_or_barrier_corruption() {
    let model = initial();
    let first = model.state_bytes().unwrap();
    let next = rename(model, "Kitchen");
    let mut consumer = Consumer::default();
    consumer.accept(&first).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    let document: Value = serde_json::from_slice(&next.state_bytes().unwrap()).unwrap();
    assert!(document["Inventory"]["identity_snapshot"].is_array());
    for variant in 0..3 {
        let mut bad = document.clone();
        let identities = bad["Inventory"]["identity_snapshot"]
            .as_array_mut()
            .unwrap();
        match variant {
            0 => identities[1]["origin"] = identities[0]["origin"].clone(),
            1 => identities[0]["intent_revision"] = json!(0),
            _ => identities[0]["projection"]["binding_revision"] = json!(99),
        }
        assert!(
            consumer.accept(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "accepted malformed snapshot {variant}"
        );
        assert_eq!(consumer.revision, 1);
        assert_eq!(consumer.by_target("Bus1.Outputs.Light1").unwrap().id, id);
    }
}

#[test]
fn a_checkpoint_cannot_reparent_an_existing_button_to_a_replacement_module() {
    let mut inventory = initial().inventory().clone();
    inventory.buses[0].inputs = serde_json::from_value(json!([{
        "short_address":0,"name":"Panel","instances":[{"number":0,"name":"Key","instance_type":1}]
    }]))
    .unwrap();
    let mut model = Model::new(inventory).unwrap();
    for index in 0..34 {
        model = rename(model, if index % 2 == 0 { "Kitchen" } else { "Light1" });
    }
    let mut saved = serde_json::to_value(model.inventory()).unwrap();
    let checkpoint = &mut saved["identity_ledger"]["checkpoint"];
    let revision = checkpoint["revision"].as_u64().unwrap();
    let parent = checkpoint["records"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record["projection"]["location"]["kind"] == "input")
        .unwrap();
    parent["origin"]["revision"] = json!(revision);
    parent["projection"]["binding_revision"] = json!(revision);
    parent["intent_revision"] = json!(revision);
    assert!(
        Model::new(serde_json::from_value(saved).unwrap()).is_err(),
        "checkpoint attached old button to replacement parent"
    );
}

#[test]
fn diagnostic_history_is_bounded_by_bytes_as_well_as_batch_count() {
    let mut model = initial();
    for index in 0..12 {
        model = model
            .edit(
                model.inventory().revision,
                &[Edit::RenameBus {
                    bus: 0,
                    name: format!("Bus{index}{}", "x".repeat(900)),
                }],
            )
            .unwrap();
        let ledger = model.inventory().identity_ledger.as_ref().unwrap();
        assert!(ledger.batches.len() <= mqtt_dali::inventory::MAX_HISTORY_BATCHES);
        assert!(
            serde_json::to_vec(&ledger.batches).unwrap().len()
                <= mqtt_dali::inventory::MAX_HISTORY_BYTES
        );
    }
    assert!(
        model.ledger().checkpoint.is_some(),
        "byte budget never checkpointed a short, large transition tail"
    );
}
