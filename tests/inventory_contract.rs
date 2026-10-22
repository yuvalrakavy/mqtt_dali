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

fn input_model() -> Model {
    let mut inventory = model().inventory().clone();
    inventory.buses[0].inputs = serde_json::from_value(json!([
        {"short_address":0,"name":"Panel","instances":[
            {"number":0,"name":"Key","instance_type":1}]}]))
    .unwrap();
    Model::new(inventory).unwrap()
}

#[test]
fn removal_consumer_retires_only_selected_lifetimes_and_never_revives_old_intent() {
    let first = input_model();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let output = "Bus1.Outputs.Light1";
    let other = "Bus1.Outputs.Light2";
    let module = "Bus1.Inputs.Panel";
    let key = "Bus1.Inputs.Panel.Instances.Key";
    let ids =
        [output, other, module, key, "Bus1.Groups.All"].map(|t| consumer.by_target(t).unwrap().id);
    consumer.set_intent(output, json!({"Brightness":0.6}));
    consumer.set_intent(key, json!({"Status":true}));
    let removed = first
        .edit(
            1,
            &[
                Edit::RemoveOutput { bus: 0, address: 0 },
                Edit::RemoveInput { bus: 0, address: 0 },
            ],
        )
        .unwrap();
    consumer.accept(&removed.state_bytes().unwrap()).unwrap();
    for (target, id) in [(output, ids[0]), (module, ids[2]), (key, ids[3])] {
        assert!(consumer.by_target(target).is_none());
        assert!(consumer.retired.contains(&id));
    }
    assert_eq!(consumer.by_target(other).unwrap().id, ids[1]);
    assert_eq!(consumer.by_target("Bus1.Groups.All").unwrap().id, ids[4]);
    let recreated = removed
        .edit(
            2,
            &[
                Edit::AddOutput {
                    bus: 0,
                    output: first.inventory().buses[0].outputs[0].clone(),
                },
                Edit::AddInput {
                    bus: 0,
                    input: first.inventory().buses[0].inputs[0].clone(),
                },
            ],
        )
        .unwrap();
    let reopened = Model::new(
        serde_json::from_slice(&serde_json::to_vec(recreated.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    consumer.accept(&reopened.state_bytes().unwrap()).unwrap();
    assert_ne!(
        consumer.by_target(output).unwrap().id,
        ids[0],
        "rediscovery reused a removed consumer object"
    );
    assert_ne!(consumer.by_target(module).unwrap().id, ids[2]);
    assert_ne!(consumer.by_target(key).unwrap().id, ids[3]);
    assert!(
        consumer.by_target(output).unwrap().intent.is_none(),
        "old output intent reached a new lifetime"
    );
    assert!(
        consumer.by_target(key).unwrap().intent.is_none(),
        "old input intent reached a new lifetime"
    );
    assert!(!consumer.accepts_state(output, "0123456789abcdef0123456789abcdef.1"));
    assert!(consumer.accepts_state(output, "0123456789abcdef0123456789abcdef.3"));
}

#[test]
fn bus_setup_consumer_preserves_objects_across_disable_enable_and_bus_rename() {
    let first = input_model();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let output = "Bus1.Outputs.Light1";
    let child = "Bus1.Inputs.Panel.Instances.Key";
    let output_id = consumer.by_target(output).unwrap().id;
    let child_id = consumer.by_target(child).unwrap().id;
    consumer.set_intent(output, json!({"Brightness":0.5}));
    let disabled = first
        .edit(
            1,
            &[Edit::ConfigureBus {
                bus: 0,
                name: "Bus1".into(),
                enabled: false,
            }],
        )
        .unwrap();
    consumer.accept(&disabled.state_bytes().unwrap()).unwrap();
    assert_eq!(consumer.by_target(output).unwrap().id, output_id);
    assert_eq!(consumer.by_target(child).unwrap().id, child_id);
    assert!(consumer.by_target(output).unwrap().intent.is_some());
    assert!(consumer.accepts_state(output, "0123456789abcdef0123456789abcdef.1"));
    let renamed = disabled
        .edit(
            2,
            &[Edit::ConfigureBus {
                bus: 0,
                name: "Upper".into(),
                enabled: true,
            }],
        )
        .unwrap();
    let reopened = Model::new(
        serde_json::from_slice(&serde_json::to_vec(renamed.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    consumer.accept(&reopened.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Upper.Outputs.Light1").unwrap().id,
        output_id,
        "bus rename replaced fixture identity"
    );
    assert_eq!(
        consumer
            .by_target("Upper.Inputs.Panel.Instances.Key")
            .unwrap()
            .id,
        child_id
    );
    assert!(consumer.by_target(output).is_none());
    assert!(consumer
        .by_target("Upper.Outputs.Light1")
        .unwrap()
        .intent
        .is_some());
    assert!(!consumer.accepts_state("Upper.Outputs.Light1", "0123456789abcdef0123456789abcdef.1"));
}

#[test]
fn commissioning_reservation_consumer_adds_unknown_item_without_replacing_existing_bindings() {
    let first = model();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let old_id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.4}));
    let reserved = first
        .edit(
            1,
            &[Edit::AddOutput {
                bus: 0,
                output: serde_json::from_value(json!({"short_address":2,"name":"Light3"})).unwrap(),
            }],
        )
        .unwrap();
    // Reconstruct from the durable document, as after a process restart.
    let reopened = Model::new(
        serde_json::from_slice(&serde_json::to_vec(reserved.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    consumer.accept(&reopened.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().id,
        old_id
    );
    assert!(consumer
        .by_target("Bus1.Outputs.Light1")
        .unwrap()
        .intent
        .is_some());
    let added = consumer.by_target("Bus1.Outputs.Light3").unwrap();
    assert!(
        added.observed.is_none(),
        "address reservation fabricated current consumer state"
    );
    assert!(consumer.accepts_state("Bus1.Outputs.Light3", "0123456789abcdef0123456789abcdef.2"));
    assert!(!consumer.accepts_state("Bus1.Outputs.Light3", "0123456789abcdef0123456789abcdef.1"));
}

#[test]
fn input_expansion_consumer_keeps_parent_child_and_intent_continuity() {
    let first = input_model();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let parent = "Bus1.Inputs.Panel";
    let child = "Bus1.Inputs.Panel.Instances.Key";
    let parent_id = consumer.by_target(parent).unwrap().id;
    let child_id = consumer.by_target(child).unwrap().id;
    consumer.set_intent(child, json!({"Status":true}));
    let expanded = first
        .edit(
            1,
            &[Edit::AddInputInstance {
                bus: 0,
                address: 0,
                instance: serde_json::from_value(
                    json!({"number":1,"name":"Contact","instance_type":2}),
                )
                .unwrap(),
            }],
        )
        .unwrap();
    consumer.accept(&expanded.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target(parent).unwrap().id,
        parent_id,
        "input expansion replaced the parent identity"
    );
    assert_eq!(
        consumer.by_target(child).unwrap().id,
        child_id,
        "input expansion replaced the existing child identity"
    );
    assert!(
        consumer.by_target(child).unwrap().intent.is_some(),
        "input expansion retired continuing child intent"
    );
    let added = "Bus1.Inputs.Panel.Instances.Contact";
    assert!(
        consumer.by_target(added).is_some(),
        "expanded child did not reach the independent consumer"
    );
    assert!(consumer.accepts_state(child, "0123456789abcdef0123456789abcdef.1"));
    assert!(consumer.accepts_state(added, "0123456789abcdef0123456789abcdef.2"));
    assert!(!consumer.accepts_state(added, "0123456789abcdef0123456789abcdef.1"));
    let repeated = expanded.edit(
        2,
        &[Edit::AddInputInstance {
            bus: 0,
            address: 0,
            instance: serde_json::from_value(json!({"number":1,"name":"Other","instance_type":2}))
                .unwrap(),
        }],
    );
    assert!(
        repeated.is_err(),
        "duplicate input instance overwrote a reserved child"
    );
}

// Deliberately construct the new wire contract without producer edit/replay helpers.
fn color_document() -> Value {
    let mut value: Value = serde_json::from_slice(&model().state_bytes().unwrap()).unwrap();
    let current = &mut value["Inventory"];
    current["revision"] = json!(2);
    current["buses"][0]["outputs"][0]["binding_revision"] = json!(2);
    current["buses"][0]["outputs"][0]["color_representation"] = json!("temperature");
    let from = current["identity_ledger"]["baseline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["target"] == "Bus1.Outputs.Light1")
        .unwrap()
        .clone();
    let mut to = from.clone();
    to["binding_revision"] = json!(2);
    to["color_representation"] = json!("temperature");
    current["identity_ledger"]["batches"] =
        json!([{"revision":2,"transitions":[{"from":from,"to":to}]}]);
    value
}

#[test]
fn color_selection_consumer_keeps_identity_but_retires_old_intent_and_observation() {
    let first = model();
    let target = "Bus1.Outputs.Light1";
    let group = "Bus1.Groups.All";
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let id = consumer.by_target(target).unwrap().id;
    let group_before = consumer.by_target(group).unwrap().record.clone();
    consumer.set_intent(target, json!({"Brightness":0.2}));
    consumer
        .observe(
            target,
            "0123456789abcdef0123456789abcdef.1",
            br#"{"Brightness":0.2}"#,
        )
        .unwrap();
    consumer
        .accept(&serde_json::to_vec(&color_document()).unwrap())
        .expect("valid color selection continuity rejected");
    let fixture = consumer.by_target(target).unwrap();
    assert_eq!(fixture.id, id, "color selection replaced physical identity");
    assert!(
        fixture.intent.is_none(),
        "color selection preserved old contract intent"
    );
    assert!(
        fixture.observed.is_none(),
        "color selection preserved old observation"
    );
    assert_eq!(
        consumer.by_target(group).unwrap().record,
        group_before,
        "color selection changed group membership identity"
    );
    assert!(!consumer.accepts_state(target, "0123456789abcdef0123456789abcdef.1"));
    assert!(consumer.accepts_state(target, "0123456789abcdef0123456789abcdef.2"));
}

#[test]
fn color_selection_consumer_rejects_unrecorded_or_wrong_space_metadata() {
    let valid = color_document();
    let mut malformed = Vec::new();
    let mut disagreement = valid.clone();
    disagreement["Inventory"]["buses"][0]["outputs"][0]["color_representation"] = json!("xy");
    malformed.push(disagreement);
    let mut enum_object = valid.clone();
    enum_object["Inventory"]["identity_ledger"]["batches"][0]["transitions"][0]["to"]
        ["color_representation"] = json!({"temperature":null});
    malformed.push(enum_object);
    let mut unrelated = valid.clone();
    let group = unrelated["Inventory"]["identity_ledger"]["baseline"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["target"] == "Bus1.Groups.All")
        .unwrap();
    group["color_representation"] = json!("xy");
    malformed.push(unrelated);
    for document in malformed {
        let mut consumer = Consumer::default();
        consumer.accept(&model().state_bytes().unwrap()).unwrap();
        let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
        assert!(
            consumer
                .accept(&serde_json::to_vec(&document).unwrap())
                .is_err(),
            "malformed color history accepted"
        );
        assert_eq!(
            consumer.revision, 1,
            "failed color acceptance partially committed"
        );
        assert_eq!(consumer.by_target("Bus1.Outputs.Light1").unwrap().id, id);
    }
}

#[test]
fn color_selection_consumer_drops_intent_even_when_fixture_is_also_renamed() {
    let mut value = color_document();
    let current = &mut value["Inventory"];
    current["buses"][0]["outputs"][0]["name"] = json!("Kitchen");
    current["buses"][0]["outputs"][0]["target"] = json!("Bus1.Outputs.Kitchen");
    current["identity_ledger"]["batches"][0]["transitions"][0]["to"]["target"] =
        json!("Bus1.Outputs.Kitchen");
    let mut consumer = Consumer::default();
    consumer.accept(&model().state_bytes().unwrap()).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"ColorTemperature":3000}));
    consumer
        .accept(&serde_json::to_vec(&value).unwrap())
        .expect("valid renamed color selection rejected");
    assert_eq!(consumer.by_target("Bus1.Outputs.Kitchen").unwrap().id, id);
    assert!(
        consumer
            .by_target("Bus1.Outputs.Kitchen")
            .unwrap()
            .intent
            .is_none(),
        "rename carried incompatible color intent"
    );
}

#[test]
fn color_selection_consumer_rejects_wrong_space_even_in_past_history() {
    let mut document = color_document();
    let current = &mut document["Inventory"];
    current["buses"][0]["groups"][0]["binding_revision"] = json!(2);
    let group = current["identity_ledger"]["baseline"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["target"] == "Bus1.Groups.All")
        .unwrap();
    let mut to = group.clone();
    to["binding_revision"] = json!(2);
    group["color_representation"] = json!("xy");
    let edge = json!({"from":group.clone(),"to":to});
    current["identity_ledger"]["batches"][0]["transitions"]
        .as_array_mut()
        .unwrap()
        .push(edge);
    let mut consumer = Consumer::default();
    assert!(
        consumer
            .accept(&serde_json::to_vec(&document).unwrap())
            .is_err(),
        "consumer accepted past non-output color projection"
    );
    assert_eq!(consumer.revision, 0);
}

#[test]
fn color_selection_consumer_rejects_current_input_and_instance_metadata() {
    let first = input_model();
    for instance in [false, true] {
        let mut value: Value = serde_json::from_slice(&first.state_bytes().unwrap()).unwrap();
        value["Inventory"]["revision"] = json!(2);
        let input = &mut value["Inventory"]["buses"][0]["inputs"][0];
        if instance {
            input["instances"][0]["color_representation"] = json!("xy");
        } else {
            input["color_representation"] = json!("xy");
        }
        let mut consumer = Consumer::default();
        consumer.accept(&first.state_bytes().unwrap()).unwrap();
        let id = consumer
            .by_target("Bus1.Inputs.Panel.Instances.Key")
            .unwrap()
            .id;
        assert!(
            consumer
                .accept(&serde_json::to_vec(&value).unwrap())
                .is_err(),
            "non-output current color selection accepted, instance={instance}"
        );
        assert_eq!(
            consumer
                .by_target("Bus1.Inputs.Panel.Instances.Key")
                .unwrap()
                .id,
            id
        );
        assert_eq!(consumer.revision, 1);
    }
}

#[test]
fn input_consumer_preserves_rename_identity_and_retires_same_text_replacement() {
    let first = input_model();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    let old = "Bus1.Inputs.Panel.Instances.Key";
    let id = consumer.by_target(old).unwrap().id;
    consumer.set_intent(old, json!({"Binding":"Entrance"}));
    let renamed = first
        .edit(
            1,
            &[Edit::RenameInput {
                bus: 0,
                address: 0,
                name: "Wall".into(),
            }],
        )
        .unwrap();
    consumer.accept(&renamed.state_bytes().unwrap()).unwrap();
    let target = "Bus1.Inputs.Wall.Instances.Key";
    let child = consumer.by_target(target).unwrap();
    assert_eq!(child.id, id, "module rename replaced child identity");
    assert_eq!(child.intent, Some(json!({"Binding":"Entrance"})));
    assert!(!consumer.accepts_state(target, "0123456789abcdef0123456789abcdef.1"));
    assert!(consumer.accepts_state(target, "0123456789abcdef0123456789abcdef.2"));
    let replacement = renamed
        .edit(
            2,
            &[Edit::ReplaceInput {
                bus: 0,
                input: renamed.inventory().buses[0].inputs[0].clone(),
            }],
        )
        .unwrap();
    consumer
        .accept(&replacement.state_bytes().unwrap())
        .unwrap();
    assert_ne!(
        consumer.by_target(target).unwrap().id,
        id,
        "same-text input replacement retained old child identity"
    );
    assert_eq!(consumer.by_target(target).unwrap().intent, None);
}

#[test]
fn input_consumer_rejects_partial_parent_rename_or_replacement_before_mutation() {
    let first = input_model();
    let renamed = first
        .edit(
            1,
            &[Edit::RenameInput {
                bus: 0,
                address: 0,
                name: "Wall".into(),
            }],
        )
        .unwrap();
    let replacement = first
        .edit(
            1,
            &[Edit::ReplaceInput {
                bus: 0,
                input: first.inventory().buses[0].inputs[0].clone(),
            }],
        )
        .unwrap();
    for (next, reason) in [
        (renamed, "split input parent rename"),
        (replacement, "parent-only replacement"),
    ] {
        let mut value: Value = serde_json::from_slice(&next.state_bytes().unwrap()).unwrap();
        value["Inventory"]["identity_ledger"]["batches"][0]["transitions"]
            .as_array_mut()
            .unwrap()
            .retain(|edge| {
                edge["from"]["target"] == "Bus1.Inputs.Panel"
                    || edge["to"]["target"] == "Bus1.Inputs.Panel"
                    || edge["to"]["target"] == "Bus1.Inputs.Wall"
            });
        if reason == "parent-only replacement" {
            // Make the final current document agree with the surviving child;
            // only the cross-batch parent-lifetime rule can reject this history.
            value["Inventory"]["buses"][0]["inputs"][0]["instances"][0]["binding_revision"] =
                json!(1);
        }
        let mut consumer = Consumer::default();
        consumer.accept(&first.state_bytes().unwrap()).unwrap();
        let id = consumer
            .by_target("Bus1.Inputs.Panel.Instances.Key")
            .unwrap()
            .id;
        assert!(
            consumer
                .accept(&serde_json::to_vec(&value).unwrap())
                .is_err(),
            "consumer accepted {reason}"
        );
        assert_eq!(consumer.revision, 1);
        assert_eq!(
            consumer
                .by_target("Bus1.Inputs.Panel.Instances.Key")
                .unwrap()
                .id,
            id
        );
    }
    let renamed = first
        .edit(
            1,
            &[Edit::RenameInput {
                bus: 0,
                address: 0,
                name: "Wall".into(),
            }],
        )
        .unwrap();
    let mut value: Value = serde_json::from_slice(&renamed.state_bytes().unwrap()).unwrap();
    let transitions = value["Inventory"]["identity_ledger"]["batches"][0]["transitions"]
        .as_array_mut()
        .unwrap();
    let child_index = transitions
        .iter()
        .position(|e| e["from"]["target"] == "Bus1.Inputs.Panel.Instances.Key")
        .unwrap();
    let mut child = transitions.remove(child_index);
    child["to"]["binding_revision"] = json!(3);
    value["Inventory"]["identity_ledger"]["batches"]
        .as_array_mut()
        .unwrap()
        .push(json!({"revision":3,"transitions":[child]}));
    value["Inventory"]["revision"] = json!(3);
    value["Inventory"]["buses"][0]["inputs"][0]["instances"][0]["binding_revision"] = json!(3);
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    assert!(
        consumer
            .accept(&serde_json::to_vec(&value).unwrap())
            .is_err(),
        "consumer accepted an impossible intermediate input hierarchy"
    );
    assert_eq!(consumer.revision, 1);
}

#[test]
fn input_consumer_does_not_carry_binding_intent_through_rename_with_type_change() {
    let first = input_model();
    let renamed = first
        .edit(
            1,
            &[Edit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Sensor".into(),
            }],
        )
        .unwrap();
    let mut inventory = renamed.inventory().clone();
    inventory.buses[0].inputs[0].instances[0].instance_type = 3;
    for edge in &mut inventory.identity_ledger.as_mut().unwrap().batches[0].transitions {
        edge.to.as_mut().unwrap().instance_type = Some(3);
    }
    let changed = Model::new(inventory).unwrap();
    let mut consumer = Consumer::default();
    consumer.accept(&first.state_bytes().unwrap()).unwrap();
    consumer.set_intent(
        "Bus1.Inputs.Panel.Instances.Key",
        json!({"Binding":"Button"}),
    );
    consumer.accept(&changed.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer
            .by_target("Bus1.Inputs.Panel.Instances.Sensor")
            .unwrap()
            .intent,
        None,
        "type change carried old button binding intent"
    );
    let mut malformed: Value = serde_json::from_slice(&changed.state_bytes().unwrap()).unwrap();
    malformed["Inventory"]["buses"][0]["inputs"][0]["instances"][0]["instance_type"] = json!(1);
    let mut fresh = Consumer::default();
    assert!(
        fresh
            .accept(&serde_json::to_vec(&malformed).unwrap())
            .is_err(),
        "consumer ignored instance type disagreement"
    );
}

#[test]
fn state_before_inventory_is_buffered_without_creating_a_device_or_intent() {
    let mut consumer = Consumer::default();
    let token = "0123456789abcdef0123456789abcdef.1";
    consumer
        .observe(
            "Bus1.Outputs.Light1",
            token,
            br#"{"PowerState":true,"Brightness":0.25}"#,
        )
        .unwrap();
    assert!(
        consumer.devices.is_empty(),
        "unproven State created a device"
    );
    consumer.accept(&model().state_bytes().unwrap()).unwrap();
    let device = consumer.by_target("Bus1.Outputs.Light1").unwrap();
    assert_eq!(
        device.observed,
        Some(json!({"PowerState":true,"Brightness":0.25})),
        "matching State-before-Inventory was lost"
    );
    assert_eq!(device.intent, None);
}

#[test]
fn observations_follow_exact_mapping_versions_and_old_retractions_cannot_clear_new_state() {
    let initial = model();
    let next = renamed(&initial, 0, "Kitchen");
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    consumer
        .observe(
            "Bus1.Outputs.Light1",
            "0123456789abcdef0123456789abcdef.1",
            br#"{"Brightness":0.2}"#,
        )
        .unwrap();
    consumer
        .observe(
            "Bus1.Outputs.Kitchen",
            "0123456789abcdef0123456789abcdef.2",
            br#"{"Brightness":0.8}"#,
        )
        .unwrap();
    consumer.accept(&next.state_bytes().unwrap()).unwrap();
    consumer
        .observe(
            "Bus1.Outputs.Kitchen",
            "0123456789abcdef0123456789abcdef.1",
            b"",
        )
        .unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Kitchen").unwrap().observed,
        Some(json!({"Brightness":0.8})),
        "old retraction erased fresh observed state"
    );
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Kitchen").unwrap().observed,
        Some(json!({"Brightness":0.8}))
    );
    let later = renamed(&next, 0, "Final");
    consumer.accept(&later.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Final").unwrap().observed,
        None,
        "observation was migrated to another mapping"
    );
}

#[test]
fn retractions_remove_pending_state_and_invalid_inventory_does_not_drain_it() {
    let initial = model();
    let next = renamed(&initial, 0, "Kitchen");
    let mut consumer = Consumer::default();
    consumer.accept(&initial.state_bytes().unwrap()).unwrap();
    let token = "0123456789abcdef0123456789abcdef.2";
    consumer
        .observe("Bus1.Outputs.Kitchen", token, br#"{"PowerState":true}"#)
        .unwrap();
    let mut malformed: Value = serde_json::from_slice(&next.state_bytes().unwrap()).unwrap();
    malformed["Inventory"]["name"] = json!(false);
    assert!(consumer
        .accept(&serde_json::to_vec(&malformed).unwrap())
        .is_err());
    assert!(consumer.by_target("Bus1.Outputs.Kitchen").is_none());
    consumer
        .observe("Bus1.Outputs.Kitchen", token, b"")
        .unwrap();
    consumer.accept(&next.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Kitchen").unwrap().observed,
        None,
        "pending retraction was ignored"
    );
}

#[test]
fn pending_observations_are_bounded_and_malformed_values_are_atomic() {
    let mut consumer = Consumer::default();
    let token = "0123456789abcdef0123456789abcdef.1";
    consumer
        .observe("Bus1.Outputs.Light1", token, br#"{"PowerState":true}"#)
        .unwrap();
    for n in 0..64 {
        consumer
            .observe(
                &format!("Bus1.Outputs.Unknown{n}"),
                token,
                br#"{"PowerState":true}"#,
            )
            .unwrap();
    }
    assert!(
        consumer.pending_len() <= 64,
        "unbounded pending observations"
    );
    consumer.accept(&model().state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().observed,
        None,
        "oldest observation was not evicted"
    );
    for (target, generation, bytes) in [
        ("Bus1.Outputs.Light1", token, b"[]".as_slice()),
        ("Bus1.Outputs.Light1", token, b"{\"assumed\":42}".as_slice()),
        ("Bus1.Outputs.Light1", "bad", b"{}".as_slice()),
        ("bad/target", token, b"{}".as_slice()),
        ("__inventory", token, b"{}".as_slice()),
    ] {
        assert!(
            consumer.observe(target, generation, bytes).is_err(),
            "malformed State was accepted"
        );
    }
    consumer
        .observe("Bus1.Outputs.Light1", token, br#"{"Brightness":0.4}"#)
        .unwrap();
    let foreign = "fedcba9876543210fedcba9876543210.1";
    consumer
        .observe("Bus1.Outputs.Light1", foreign, br#"{"Brightness":0.9}"#)
        .unwrap();
    assert_eq!(
        consumer.by_target("Bus1.Outputs.Light1").unwrap().observed,
        Some(json!({"Brightness":0.4})),
        "foreign epoch changed current observation"
    );
}

#[test]
fn observation_budget_includes_keys_and_payloads_and_inventory_outcomes_are_explicit() {
    use inventory_consumer::Acceptance;
    let mut consumer = Consumer::default();
    let token = "0123456789abcdef0123456789abcdef.1";
    let payload = serde_json::to_vec(&json!({"Description":"x".repeat(4000)})).unwrap();
    for n in 0..20 {
        consumer
            .observe(
                &format!("Bus1.Outputs.N{n}{}", "x".repeat(2000)),
                token,
                &payload,
            )
            .unwrap();
    }
    assert!(
        consumer.pending_len() < 12,
        "cache budget omitted target keys or payload bytes"
    );
    let initial = model();
    assert_eq!(
        consumer.accept(&initial.state_bytes().unwrap()).unwrap(),
        Acceptance::Applied
    );
    assert_eq!(
        consumer.accept(&initial.state_bytes().unwrap()).unwrap(),
        Acceptance::Unchanged
    );
    let next = renamed(&initial, 0, "Kitchen");
    consumer.accept(&next.state_bytes().unwrap()).unwrap();
    assert_eq!(
        consumer.accept(&initial.state_bytes().unwrap()).unwrap(),
        Acceptance::Ignored
    );
}

#[tokio::test]
async fn an_independent_consumer_accepts_actual_mqtt_inventory_and_preserves_its_own_binding_and_intent(
) {
    use mqtt_bridge_kit::{Bridge, RequestContext};
    use mqtt_dali::{
        config::ConfigPaths,
        service::{DaliService, MetadataEdit, Phase},
    };
    use mqtt_test_broker::FakeBroker;
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
    use tokio::time::Instant;
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "dali-consumer-wire-{:032x}",
        rand::random::<u128>()
    )));
    fs::create_dir(&directory.0).unwrap();
    let paths = ConfigPaths::in_directory(&directory.0);
    fs::write(
        &paths.current,
        serde_json::to_vec(model().inventory()).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&paths.current, fs::Permissions::from_mode(0o600)).unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = DaliService::new(paths, "HouseController".into());
    let running = Bridge::new("DALI")
        .instance("HouseController")
        .broker(&broker.address())
        .start_managed(service)
        .await
        .unwrap();
    let topic = "DALI/State/HouseController/__inventory";
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(topic).is_empty())
            .await
    );
    let first = broker.received_on(topic).pop().unwrap();
    let mut consumer = Consumer::default();
    consumer.accept(&first.payload).unwrap();
    let id = consumer.by_target("Bus1.Outputs.Light1").unwrap().id;
    consumer.set_intent("Bus1.Outputs.Light1", json!({"Brightness":0.3}));
    let saved = handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            },
            RequestContext::new(first.user_property("target-generation").unwrap()).unwrap(),
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert_eq!(saved.phase, Phase::Ready);
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(topic)
                .iter()
                .any(|m| serde_json::from_slice::<Value>(&m.payload)
                    .is_ok_and(|v| v["Inventory"]["revision"] == 2)))
            .await
    );
    let next = broker
        .received_on(topic)
        .into_iter()
        .find(|m| {
            serde_json::from_slice::<Value>(&m.payload)
                .is_ok_and(|v| v["Inventory"]["revision"] == 2)
        })
        .unwrap();
    consumer.accept(&next.payload).unwrap();
    assert_eq!(
        next.user_property("target-generation"),
        Some("0123456789abcdef0123456789abcdef.2")
    );
    let renamed = consumer.by_target("Bus1.Outputs.Kitchen").unwrap();
    assert_eq!(renamed.id, id);
    assert_eq!(
        renamed.intent,
        Some(json!({"Brightness":0.3})),
        "MQTT inventory rename lost consumer-owned intent"
    );
    assert!(consumer.by_target("Bus1.Outputs.Light1").is_none());
    assert!(running.stop().await);
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
