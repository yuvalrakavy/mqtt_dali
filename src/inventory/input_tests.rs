use super::*;
use serde_json::{json, Value};

fn document() -> Value {
    json!({"schema_version":2,"revision":1,
        "inventory_epoch":"0123456789abcdef0123456789abcdef",
        "name":"Controller","buses":[{"bus":0,"name":"Bus1","enabled":true,
        "outputs":[{"short_address":0,"name":"Panel"}],"groups":[],
        "inputs":[{"short_address":0,"name":"Panel","instances":[
            {"number":0,"name":"Key","instance_type":1},
            {"number":31,"name":"Sensor","instance_type":255}]}]}]})
}
fn model() -> Model {
    Model::new(serde_json::from_value(document()).expect("named inputs must deserialize")).unwrap()
}

#[test]
fn input_inventory_has_distinct_addresses_and_complete_nested_projections() {
    let model = model();
    assert_eq!(
        model
            .projections()
            .into_iter()
            .find(|p| p.target.ends_with(".Sensor"))
            .unwrap()
            .instance_type,
        Some(255),
        "unknown input type was lost from its identity projection"
    );
    let actual = model
        .projections()
        .into_iter()
        .map(|p| (p.target, p.binding_revision))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            ("Bus1.Inputs.Panel".into(), 1),
            ("Bus1.Inputs.Panel.Instances.Key".into(), 1),
            ("Bus1.Inputs.Panel.Instances.Sensor".into(), 1),
            ("Bus1.Outputs.Panel".into(), 1),
        ],
        "input module/instance projections are incomplete"
    );
    let state: Value = serde_json::from_slice(&model.state_bytes().unwrap()).unwrap();
    let input = &state["Inventory"]["buses"][0]["inputs"][0];
    assert_eq!(input["target"], "Bus1.Inputs.Panel");
    assert_eq!(
        input["instances"][0]["target"],
        "Bus1.Inputs.Panel.Instances.Key"
    );
    assert_eq!(input["instances"][1]["instance_type"], 255);
    assert_eq!(input["instances"][1]["binding_revision"], 1);
    let round_trip: crate::config::Inventory =
        serde_json::from_slice(&serde_json::to_vec(model.inventory()).unwrap()).unwrap();
    assert_eq!(
        Model::new(round_trip).unwrap().projections(),
        model.projections()
    );
}

#[test]
fn input_inventory_rejects_duplicate_or_invalid_modules_instances_and_names() {
    let original = document();
    let mut invalid = Vec::new();
    for address in [64, 255] {
        let mut v = original.clone();
        v["buses"][0]["inputs"][0]["short_address"] = json!(address);
        invalid.push(v);
    }
    let mut v = original.clone();
    v["buses"][0]["inputs"] = Value::Null;
    invalid.push(v);
    for field in ["number", "name"] {
        let mut v = original.clone();
        v["buses"][0]["inputs"][0]["instances"][1][field] =
            v["buses"][0]["inputs"][0]["instances"][0][field].clone();
        invalid.push(v);
    }
    for (field, value) in [
        ("number", json!(32)),
        ("name", json!("bad/name")),
        ("name", json!("💡".repeat(500))),
        ("instance_type", json!(256)),
    ] {
        let mut v = original.clone();
        v["buses"][0]["inputs"][0]["instances"][0][field] = value;
        invalid.push(v);
    }
    for (address, name) in [(0, "Other"), (1, "Panel")] {
        let mut v = original.clone();
        v["buses"][0]["inputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"short_address":address,"name":name,"instances":[]}));
        invalid.push(v);
    }
    for value in invalid {
        let accepted = serde_json::from_value::<crate::config::Inventory>(value.clone())
            .is_ok_and(|v| Model::new(v).is_ok());
        assert!(!accepted, "invalid input inventory was accepted: {value}");
    }
    let mut old = original;
    old["buses"][0].as_object_mut().unwrap().remove("inputs");
    let old = Model::new(serde_json::from_value(old).unwrap()).unwrap();
    assert_eq!(old.projections().len(), 1);
    let state: Value = serde_json::from_slice(&old.state_bytes().unwrap()).unwrap();
    assert!(
        state["Inventory"]["buses"][0].get("inputs").is_none(),
        "empty inputs changed legacy output inventory wire shape"
    );
}

#[test]
fn input_rename_preserves_lifetimes_and_rotates_every_affected_descendant() {
    let first = model();
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
    let batch = &renamed.ledger().batches[0];
    assert_eq!(
        batch.transitions.len(),
        3,
        "module rename omitted descendant retirement"
    );
    for edge in &batch.transitions {
        assert!(edge
            .from
            .as_ref()
            .unwrap()
            .target
            .starts_with("Bus1.Inputs.Panel"));
        assert!(edge
            .to
            .as_ref()
            .unwrap()
            .target
            .starts_with("Bus1.Inputs.Wall"));
        assert_eq!(edge.to.as_ref().unwrap().binding_revision, 2);
    }
    let key = renamed
        .edit(
            2,
            &[Edit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Door".into(),
            }],
        )
        .unwrap();
    assert_eq!(key.ledger().batches[1].transitions.len(), 1);
    let upper = key
        .edit(
            3,
            &[Edit::RenameBus {
                bus: 0,
                name: "Upper".into(),
            }],
        )
        .unwrap();
    assert_eq!(upper.ledger().batches[2].transitions.len(), 4);
    assert!(upper.projections().iter().all(|p| p.binding_revision == 4));
    let replayed = ledger::replay(upper.ledger(), 4).unwrap();
    assert_eq!(
        replayed["Upper.Inputs.Wall.Instances.Door"].origin,
        Origin {
            target: "Bus1.Inputs.Panel.Instances.Key".into(),
            revision: 1
        }
    );
    assert_eq!(
        replayed["Upper.Inputs.Wall"].origin,
        Origin {
            target: "Bus1.Inputs.Panel".into(),
            revision: 1
        }
    );
    assert!(first
        .edit(
            0,
            &[Edit::RenameInput {
                bus: 0,
                address: 0,
                name: "Bad".into()
            }]
        )
        .is_err());
}

#[test]
fn replacing_or_removing_input_module_retires_all_descendants_even_with_same_text() {
    let first = model();
    let replacement = first
        .edit(
            1,
            &[Edit::ReplaceInput {
                bus: 0,
                input: first.inventory().buses[0].inputs[0].clone(),
            }],
        )
        .unwrap();
    assert_eq!(
        replacement.ledger().batches[0].transitions.len(),
        6,
        "same-text replacement reused input lifetimes"
    );
    let records = ledger::replay(replacement.ledger(), 2).unwrap();
    for (target, record) in records.iter().filter(|(t, _)| t.contains(".Inputs.")) {
        assert_eq!(
            record.origin,
            Origin {
                target: target.clone(),
                revision: 2
            },
            "same-text replacement reused a descendant lifetime"
        );
        assert_eq!(record.projection.binding_revision, 2);
    }
    let removed = replacement
        .edit(2, &[Edit::RemoveInput { bus: 0, address: 0 }])
        .unwrap();
    assert_eq!(removed.projections().len(), 1);
    assert_eq!(removed.ledger().batches[1].transitions.len(), 3);
    let added = removed
        .edit(
            3,
            &[Edit::AddInput {
                bus: 0,
                input: first.inventory().buses[0].inputs[0].clone(),
            }],
        )
        .unwrap();
    assert_eq!(
        added
            .projections()
            .iter()
            .filter(|p| p.binding_revision == 4)
            .count(),
        3
    );
}

#[test]
fn input_history_rejects_orphans_partial_renames_and_parent_only_replacements() {
    let first = model();
    let mut orphan = first.ledger().clone();
    orphan.baseline.retain(|p| p.target != "Bus1.Inputs.Panel");
    assert!(
        ledger::replay(&orphan, 1).is_err(),
        "orphan input instance was accepted"
    );
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
    let mut split = renamed.ledger().clone();
    split.batches[0]
        .transitions
        .retain(|e| e.from.as_ref().unwrap().target == "Bus1.Inputs.Panel");
    assert!(
        ledger::replay(&split, 2).is_err(),
        "split module rename was accepted"
    );
    let replacement = first
        .edit(
            1,
            &[Edit::ReplaceInput {
                bus: 0,
                input: first.inventory().buses[0].inputs[0].clone(),
            }],
        )
        .unwrap();
    let mut parent_only = replacement.ledger().clone();
    parent_only.batches[0]
        .transitions
        .retain(|e| e.from.as_ref().or(e.to.as_ref()).unwrap().target == "Bus1.Inputs.Panel");
    assert!(
        ledger::replay(&parent_only, 2).is_err(),
        "parent-only replacement carried old child lifetimes"
    );
    let mut wrong_type = first.inventory().clone();
    wrong_type.identity_ledger = Some(first.ledger().clone());
    wrong_type.buses[0].inputs[0].instances[0].instance_type = 3;
    assert!(
        Model::new(wrong_type).is_err(),
        "instance type changed without mapping retirement"
    );
}
