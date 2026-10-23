use super::*;
use serde_json::{json, Value};

#[test]
fn color_selection_rotates_only_contract_generation_and_survives_reload() {
    use crate::config::ColorRepresentation;
    let initial = Model::new(baseline()).unwrap();
    let select = |representation| Edit::OutputColorRepresentation {
        bus: 0,
        address: 0,
        representation,
    };
    let selected = initial
        .edit(1, &[select(Some(ColorRepresentation::Temperature))])
        .expect("color selection kept old request generation");
    let find = |model: &Model, target: &str| {
        model
            .projections()
            .into_iter()
            .find(|p| p.target == target)
            .unwrap()
    };
    let old = find(&initial, "Bus1.Outputs.Light1");
    let changed = find(&selected, "Bus1.Outputs.Light1");
    assert_eq!(
        changed.color_representation,
        Some(ColorRepresentation::Temperature),
        "color selection missing from identity projection"
    );
    assert_eq!(
        changed.binding_revision, 2,
        "color selection kept old request generation"
    );
    assert_eq!(changed.location, old.location);
    let edge = &selected.ledger().batches[0].transitions[0];
    assert_eq!(
        edge.from.as_ref(),
        Some(&old),
        "color selection lost continuity source"
    );
    assert_eq!(
        edge.to.as_ref(),
        Some(&changed),
        "color selection created a replacement lifetime"
    );
    assert_eq!(
        find(&selected, "Bus1.Groups.All"),
        find(&initial, "Bus1.Groups.All")
    );
    assert_eq!(
        find(&selected, "Bus1.Outputs.Light2"),
        find(&initial, "Bus1.Outputs.Light2")
    );
    let same = selected
        .edit(2, &[select(Some(ColorRepresentation::Temperature))])
        .unwrap();
    assert_eq!(
        same.inventory(),
        selected.inventory(),
        "identical color selection changed revision"
    );
    let renamed = selected
        .edit(
            2,
            &[
                Edit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Kitchen".into(),
                },
                select(Some(ColorRepresentation::Xy)),
            ],
        )
        .unwrap();
    let mut consumer = crate::inventory::Model::new(
        serde_json::from_slice(&serde_json::to_vec(renamed.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        consumer.state_bytes().unwrap(),
        renamed.state_bytes().unwrap(),
        "color selection reload changed complete inventory"
    );
    consumer = consumer.edit(3, &[select(None)]).unwrap();
    assert_eq!(find(&consumer, "Bus1.Outputs.Kitchen").binding_revision, 4);
    let returned = consumer
        .edit(4, &[select(Some(ColorRepresentation::Temperature))])
        .unwrap();
    assert_eq!(
        find(&returned, "Bus1.Outputs.Kitchen").binding_revision,
        5,
        "returning color selection reused retired generation"
    );
    let mut tampered = selected.inventory().clone();
    tampered.buses[0].outputs[0].color_representation = Some(ColorRepresentation::Xy);
    assert!(
        Model::new(tampered).is_err(),
        "unrecorded color selection was accepted"
    );
    let mut illegal = selected.inventory().clone();
    let group = illegal
        .identity_ledger
        .as_mut()
        .unwrap()
        .baseline
        .iter_mut()
        .find(|p| p.target == "Bus1.Groups.All")
        .unwrap();
    group.color_representation = Some(ColorRepresentation::Xy);
    assert!(
        Model::new(illegal).is_err(),
        "unrecorded group color projection was accepted"
    );
}

#[test]
fn color_selection_rejects_non_output_metadata_even_in_past_history() {
    let mut inventory = baseline();
    inventory.buses[0]
        .scenes
        .push(serde_json::from_value(json!({"slot":0,"name":"Evening"})).unwrap());
    let initial = Model::new(inventory.clone()).unwrap();
    inventory.revision = 2;
    let mut ledger = initial.ledger().clone();
    let scene = ledger
        .baseline
        .iter_mut()
        .find(|p| p.target == "Bus1.Scenes.Evening")
        .unwrap();
    let mut to = scene.clone();
    to.binding_revision = 2;
    scene.color_representation = Some(crate::config::ColorRepresentation::Xy);
    ledger.batches.push(Batch {
        revision: 2,
        transitions: vec![Transition {
            from: Some(scene.clone()),
            to: Some(to),
        }],
    });
    inventory.identity_ledger = Some(ledger);
    assert!(
        Model::new(inventory).is_err(),
        "past non-output color projection was accepted"
    );
}

#[test]
fn group_color_selection_rotates_its_contract_but_preserves_origin_members_and_outputs() {
    use crate::config::ColorRepresentation;
    let initial = Model::new(baseline()).unwrap();
    let select = |representation| Edit::GroupColorRepresentation {
        bus: 0,
        address: 0,
        representation,
    };
    let find = |m: &Model, target: &str| {
        m.projections()
            .into_iter()
            .find(|p| p.target == target)
            .unwrap()
    };
    let before = find(&initial, "Bus1.Groups.All");
    let selected = initial
        .edit(1, &[select(Some(ColorRepresentation::Xy))])
        .unwrap();
    let after = find(&selected, "Bus1.Groups.All");
    assert_eq!(
        after.binding_revision, 2,
        "group color selection kept retired contract generation"
    );
    assert_eq!(after.color_representation, Some(ColorRepresentation::Xy));
    assert_eq!(after.members, before.members);
    assert_eq!(after.location, before.location);
    let edge = &selected.ledger().batches[0].transitions[0];
    assert_eq!(edge.from.as_ref(), Some(&before));
    assert_eq!(edge.to.as_ref(), Some(&after));
    for target in ["Bus1.Outputs.Light1", "Bus1.Outputs.Light2"] {
        assert_eq!(find(&initial, target), find(&selected, target));
    }
    let same = selected
        .edit(2, &[select(Some(ColorRepresentation::Xy))])
        .unwrap();
    assert_eq!(same.inventory(), selected.inventory());
    let cleared = selected.edit(2, &[select(None)]).unwrap();
    assert_eq!(find(&cleared, "Bus1.Groups.All").binding_revision, 3);
    assert_eq!(find(&cleared, "Bus1.Groups.All").color_representation, None);
    let reloaded = Model::new(
        serde_json::from_slice(&serde_json::to_vec(cleared.inventory()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reloaded.state_bytes().unwrap(),
        cleared.state_bytes().unwrap(),
        "valid historical group color was rejected or lost on reload"
    );
}

fn baseline() -> Inventory {
    serde_json::from_value(json!({
        "schema_version": 2, "revision": 1,
        "inventory_epoch": "0123456789abcdef0123456789abcdef",
        "name": "HouseController", "buses": [{
            "bus": 0, "name": "Bus1", "enabled": true,
            "outputs": [
                {"short_address": 0, "name": "Light1"},
                {"short_address": 1, "name": "Light2"}
            ],
            "groups": [{"group_address": 0, "name": "All", "members": [0,1]}]
        }]
    }))
    .unwrap()
}

#[test]
fn bus_ancestor_names_must_remain_consistent_after_every_history_batch() {
    let initial = Model::new(baseline()).unwrap();
    let renamed = initial
        .edit(
            1,
            &[Edit::RenameBus {
                bus: 0,
                name: "Upper".into(),
            }],
        )
        .unwrap();
    let mut invalid = renamed.inventory().clone();
    invalid.revision = 3;
    invalid.revision_high_water = Some(3);
    let ledger = invalid.identity_ledger.as_mut().unwrap();
    let mut later = ledger.batches[0].transitions.split_off(1);
    for edge in &mut later {
        edge.to.as_mut().unwrap().binding_revision = 3;
    }
    ledger.batches.push(Batch {
        revision: 3,
        transitions: later,
    });
    assert!(
        Model::new(invalid).is_err(),
        "accepted a split ancestor rename with an impossible intermediate bus"
    );

    let mut invalid = baseline();
    invalid.buses[0].groups.clear();
    let mut ledger = Model::new(invalid.clone()).unwrap().ledger().clone();
    if let Location::Output { bus, .. } = &mut ledger.baseline[1].location {
        *bus = 1;
    }
    // The final configuration would disagree too; assert replay itself catches
    // one public ancestor assigned to two numeric buses at the baseline.
    assert!(
        ledger::replay(&ledger, 1).is_err(),
        "accepted one ancestor name for two physical buses"
    );
}

#[test]
fn an_old_schema_two_document_gets_a_complete_named_baseline_without_changing_its_identity() {
    let inventory = baseline();
    let model = Model::new(inventory.clone()).expect("valid P0 inventory did not get a baseline");
    assert_eq!(model.inventory(), &inventory);
    assert_eq!(model.ledger().baseline.len(), 3);
    assert!(model.ledger().batches.is_empty());
    let records = model.projections();
    assert!(records.iter().all(|r| r.binding_revision == 1));
    let group = records
        .iter()
        .find(|r| r.target == "Bus1.Groups.All")
        .unwrap();
    assert_eq!(
        group.members,
        vec![
            Origin {
                target: "Bus1.Outputs.Light1".into(),
                revision: 1
            },
            Origin {
                target: "Bus1.Outputs.Light2".into(),
                revision: 1
            },
        ]
    );
    let state: Value = serde_json::from_slice(&model.state_bytes().unwrap()).unwrap();
    assert_eq!(state["Inventory"]["schema_version"], 1);
    assert_eq!(
        state["Inventory"]["inventory_epoch"],
        inventory.inventory_epoch
    );
    assert_eq!(
        state["Inventory"]["buses"][0]["outputs"][0]["target"],
        "Bus1.Outputs.Light1"
    );
    assert_eq!(
        state["Inventory"]["buses"][0]["outputs"][0]["binding_revision"],
        1
    );
    assert!(
        state["Inventory"].get("revision_high_water").is_none(),
        "private reservation leaked onto MQTT"
    );
}

#[test]
fn individually_valid_targets_can_still_exceed_the_complete_document_limit() {
    let mut inventory = baseline();
    inventory.description = Some("x".repeat(MAX_STATE_BYTES));
    assert!(
        Model::new(inventory).is_err(),
        "oversized complete inventory was accepted"
    );
    let mut inventory = baseline();
    inventory.buses[0].outputs[0].name = "💡".repeat(1000);
    assert!(
        Model::new(inventory).is_err(),
        "oversized encoded target was accepted"
    );
}

#[test]
fn rename_back_and_readdress_preserve_lifetime_and_group_meaning_but_never_reuse_fences() {
    let initial = Model::new(baseline()).unwrap();
    let renamed = initial
        .edit(
            1,
            &[Edit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            }],
        )
        .unwrap();
    assert_eq!(
        initial.inventory().revision,
        1,
        "editing mutated the accepted model"
    );
    assert_eq!(renamed.inventory().revision, 2);
    let moved = renamed
        .edit(
            2,
            &[Edit::ReaddressOutputs {
                bus: 0,
                addresses: vec![(0, 17)],
            }],
        )
        .unwrap();
    assert_eq!(moved.inventory().buses[0].groups[0].members, vec![17, 1]);
    let restored = moved
        .edit(
            3,
            &[Edit::RenameOutput {
                bus: 0,
                address: 17,
                name: "Light1".into(),
            }],
        )
        .unwrap();
    let output = restored
        .projections()
        .into_iter()
        .find(|r| r.target == "Bus1.Outputs.Light1")
        .unwrap();
    assert_eq!(output.binding_revision, 4);
    let group = restored
        .projections()
        .into_iter()
        .find(|r| r.target == "Bus1.Groups.All")
        .unwrap();
    assert_eq!(
        group.binding_revision, 1,
        "renaming/readdressing one lifetime changed the group's meaning"
    );
    assert_eq!(
        group.members[0],
        Origin {
            target: "Bus1.Outputs.Light1".into(),
            revision: 1
        }
    );
    let ledger = restored.inventory().identity_ledger.as_ref().unwrap();
    assert_eq!(ledger.batches.len(), 3);
    assert!(ledger.batches.iter().all(|b| b.transitions.len() == 1));
    Model::new(restored.inventory().clone()).expect("saved ledger could not be reconstructed");
}

#[test]
fn replacement_at_the_same_address_and_name_rotates_dependent_groups_and_is_a_new_lifetime() {
    let initial = Model::new(baseline()).unwrap();
    let changed = initial
        .edit(
            1,
            &[Edit::ReplaceOutput {
                bus: 0,
                output: initial.inventory().buses[0].outputs[0].clone(),
            }],
        )
        .unwrap();
    assert_eq!(changed.inventory().buses[0].groups[0].members, vec![0, 1]);
    let group = changed
        .projections()
        .into_iter()
        .find(|r| r.target == "Bus1.Groups.All")
        .unwrap();
    assert_eq!(
        group.binding_revision, 2,
        "unchanged numeric membership hid a replacement fixture"
    );
    assert!(group.members.contains(&Origin {
        target: "Bus1.Outputs.Light1".into(),
        revision: 2
    }));
    let transitions = &changed.ledger().batches[0].transitions;
    assert_eq!(
        transitions
            .iter()
            .filter(|e| e.from.is_some() && e.to.is_none())
            .count(),
        1
    );
    assert_eq!(
        transitions
            .iter()
            .filter(|e| e.from.is_none() && e.to.is_some())
            .count(),
        1
    );
    assert_eq!(
        transitions
            .iter()
            .filter(|e| e.from.is_some() && e.to.is_some())
            .count(),
        1
    );
    Model::new(changed.inventory().clone()).unwrap();
}

#[test]
fn atomic_name_swaps_and_description_gaps_validate_without_losing_edges_or_reusing_reserved_revisions(
) {
    let initial = Model::new(baseline()).unwrap();
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
    assert_eq!(swapped.ledger().batches[0].transitions.len(), 2);
    let described = swapped
        .edit(
            2,
            &[Edit::Description {
                description: Some("upstairs".into()),
            }],
        )
        .unwrap();
    assert_eq!(described.inventory().revision, 3);
    assert_eq!(described.ledger().batches.len(), 1);
    let mut reserved = described.inventory().clone();
    reserved.revision_high_water = Some(8);
    let reserved = Model::new(reserved).unwrap();
    let renamed = reserved
        .edit(
            3,
            &[Edit::RenameBus {
                bus: 0,
                name: "Upper".into(),
            }],
        )
        .unwrap();
    assert_eq!(
        renamed.inventory().revision,
        9,
        "a reserved revision was reused"
    );
    assert_eq!(renamed.ledger().batches[1].revision, 9);
    assert!(renamed
        .projections()
        .iter()
        .all(|p| p.target.starts_with("Upper.") && p.binding_revision == 9));
    assert!(
        renamed
            .edit(3, &[Edit::Description { description: None }])
            .is_err(),
        "stale expected revision was accepted"
    );
    let mut exhausted = renamed.inventory().clone();
    exhausted.revision_high_water = Some(u64::MAX);
    assert!(
        Model::new(exhausted)
            .unwrap()
            .edit(9, &[Edit::Description { description: None }])
            .is_err(),
        "revision counter wrapped"
    );
}

#[test]
fn a_present_incomplete_ledger_and_impossible_group_dependency_never_fall_back_to_baseline() {
    let initial = Model::new(baseline()).unwrap();
    let mut invalid = initial.inventory().clone();
    invalid.identity_ledger = Some(Ledger {
        baseline: vec![],
        batches: vec![],
    });
    assert!(
        Model::new(invalid).is_err(),
        "present incomplete ledger silently became a baseline"
    );
    let changed = initial
        .edit(
            1,
            &[Edit::ReplaceOutput {
                bus: 0,
                output: initial.inventory().buses[0].outputs[0].clone(),
            }],
        )
        .unwrap();
    let mut invalid = changed.inventory().clone();
    invalid.identity_ledger.as_mut().unwrap().batches[0]
        .transitions
        .retain(|edge| {
            !edge
                .from
                .as_ref()
                .is_some_and(|p| p.target.contains(".Groups."))
        });
    assert!(
        Model::new(invalid).is_err(),
        "missing group dependency edge was accepted"
    );
    let mut invalid = changed.inventory().clone();
    let duplicate = invalid.identity_ledger.as_ref().unwrap().batches[0].transitions[0].clone();
    invalid.identity_ledger.as_mut().unwrap().batches[0]
        .transitions
        .push(duplicate);
    assert!(
        Model::new(invalid).is_err(),
        "duplicate source or destination was accepted"
    );
    let mut invalid = changed.inventory().clone();
    invalid.revision_high_water = Some(1);
    assert!(
        Model::new(invalid).is_err(),
        "high-water below active revision was accepted"
    );
    let mut invalid = changed.inventory().clone();
    invalid.identity_ledger.as_mut().unwrap().batches[0]
        .transitions
        .iter_mut()
        .find_map(|edge| edge.from.as_mut())
        .unwrap()
        .binding_revision = 99;
    assert!(
        Model::new(invalid).is_err(),
        "inconsistent historical source was accepted"
    );
}

#[test]
fn explicit_null_identity_or_allocation_fields_are_not_treated_as_legacy_absence() {
    for field in ["identity_ledger", "revision_high_water"] {
        let mut value = serde_json::to_value(baseline()).unwrap();
        value[field] = Value::Null;
        assert!(
            serde_json::from_value::<Inventory>(value).is_err(),
            "explicit null {field} became a legacy default"
        );
    }
}

#[test]
fn edits_are_atomic_for_new_items_simultaneous_address_moves_and_invalid_groups() {
    let initial = Model::new(baseline()).unwrap();
    let no_op = initial
        .edit(
            1,
            &[Edit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Light1".into(),
            }],
        )
        .unwrap();
    assert_eq!(
        no_op.inventory(),
        initial.inventory(),
        "duplicate inline save created another revision"
    );
    let swapped = initial
        .edit(
            1,
            &[Edit::ReaddressOutputs {
                bus: 0,
                addresses: vec![(0, 1), (1, 0)],
            }],
        )
        .unwrap();
    let group = swapped
        .projections()
        .into_iter()
        .find(|p| p.target == "Bus1.Groups.All")
        .unwrap();
    assert_eq!(group.binding_revision, 1);
    let added = swapped
        .edit(
            2,
            &[
                Edit::AddOutput {
                    bus: 0,
                    output: crate::config::Output {
                        short_address: 9,
                        name: "New".into(),
                        description: None,
                        color_representation: None,
                    },
                },
                Edit::RenameOutput {
                    bus: 0,
                    address: 9,
                    name: "Desk".into(),
                },
                Edit::AddGroup {
                    bus: 0,
                    group: crate::config::Group {
                        group_address: 5,
                        name: "Work".into(),
                        description: None,
                        members: vec![9, 0],
                        color_representation: None,
                    },
                },
                Edit::RenameGroup {
                    bus: 0,
                    address: 5,
                    name: "Office".into(),
                },
                Edit::RemoveGroup { bus: 0, address: 0 },
            ],
        )
        .unwrap();
    let group = added
        .projections()
        .into_iter()
        .find(|p| p.target == "Bus1.Groups.Office")
        .unwrap();
    assert!(
        group.members.contains(&Origin {
            target: "Bus1.Outputs.Desk".into(),
            revision: 3
        }),
        "an intermediate unsaved name became a lifetime origin"
    );
    for invalid in [
        Edit::GroupMembers {
            bus: 0,
            address: 5,
            members: vec![9, 9],
        },
        Edit::GroupMembers {
            bus: 0,
            address: 5,
            members: vec![63],
        },
        Edit::ReaddressOutputs {
            bus: 0,
            addresses: vec![(0, 1)],
        },
        Edit::RenameBus {
            bus: 0,
            name: "bad/name".into(),
        },
    ] {
        assert!(added.edit(3, &[invalid]).is_err());
        assert_eq!(added.inventory().revision, 3);
    }
    let changed = added
        .edit(
            3,
            &[Edit::GroupMembers {
                bus: 0,
                address: 5,
                members: vec![9],
            }],
        )
        .unwrap();
    let group = changed
        .projections()
        .into_iter()
        .find(|p| p.target == "Bus1.Groups.Office")
        .unwrap();
    assert_eq!(group.binding_revision, 4);
}

#[test]
fn the_snapshot_limit_is_inclusive_and_target_errors_include_path_and_encoded_size() {
    let mut inventory = baseline();
    inventory.description = Some(String::new());
    let fixed_bytes = Model::new(inventory.clone())
        .unwrap()
        .state_bytes()
        .unwrap()
        .len();
    inventory.description = Some("x".repeat(MAX_STATE_BYTES - fixed_bytes));
    assert_eq!(
        Model::new(inventory.clone())
            .unwrap()
            .state_bytes()
            .unwrap()
            .len(),
        MAX_STATE_BYTES
    );
    inventory.description.as_mut().unwrap().push('x');
    let error = Model::new(inventory)
        .err()
        .expect("oversized inventory state accepted");
    assert!(error.contains("524289 bytes"));
    let mut inventory = baseline();
    inventory.buses[0].outputs[0].name = "💡".repeat(1000);
    let error = Model::new(inventory).unwrap_err();
    assert!(
        error.contains("buses[0].outputs[0].name") && error.contains("12013 bytes"),
        "imprecise target error: {error}"
    );
}
