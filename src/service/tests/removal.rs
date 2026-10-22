use super::*;
use crate::service::discovery::Space;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

#[tokio::test]
async fn removal_requires_item_authority_when_controller_generation_has_advanced() {
    for input in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service, &handle, &broker).await;
        handle
            .edit(
                1,
                MetadataEdit::Description(Some("metadata only".into())),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        let edit = if input {
            MetadataEdit::RemoveInput { bus: 0, address: 0 }
        } else {
            MetadataEdit::RemoveOutput { bus: 0, address: 0 }
        };
        assert!(
            handle
                .edit(2, edit.clone(), context(2), deadline())
                .await
                .is_err(),
            "controller token authorized removal of an item with another binding token"
        );
        assert_eq!(fixture.disk().revision, 2);
        assert!(
            handle
                .edit(1, edit.clone(), context(1), deadline())
                .await
                .is_err(),
            "stale revision removed an item"
        );
        let removed = handle.edit(2, edit, context(1), deadline()).await.unwrap();
        assert_eq!(removed.model.inventory().revision, 3);
        assert_eq!(fixture.disk().buses[0].inputs.is_empty(), input);
        assert_eq!(fixture.disk().buses[0].outputs.is_empty(), !input);
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn removal_strict_mqtt_payloads_reject_unknown_fields_and_non_fixture_targets() {
    let fixture = Fixture::new();
    fixture.add_button();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let valid =
        json!({"command":"remove_from_list","target":"Bus1.Outputs.Light1","expected_revision":1});
    let mut invalid = Vec::new();
    for (field, value) in [
        ("extra", json!(true)),
        ("expected_revision", json!(null)),
        ("expected_revision", json!("1")),
        ("expected_revision", json!(0)),
        ("target", json!("__inventory")),
        ("target", json!("Bus1.Groups.All")),
        ("target", json!("Bus1.Inputs.Panel.Instances.Button")),
        ("target", json!("Bus1.Outputs.Unknown")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        invalid.push(body);
    }
    for field in ["target", "expected_revision"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        invalid.push(body);
    }
    for (i, body) in invalid.into_iter().enumerate() {
        let reply = command(&broker, body, 1, &format!("test/remove-invalid/{i}")).await;
        assert!(
            reply.get("error").is_some(),
            "invalid remove-from-list payload was accepted: {reply}"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert_eq!(fixture.disk().buses[0].outputs.len(), 1);
        assert_eq!(fixture.disk().buses[0].inputs.len(), 1);
    }
    assert!(running.stop().await);
}

#[tokio::test]
async fn removal_input_module_retires_all_children_and_old_actions_but_keeps_output_address_zero() {
    let fixture = Fixture::new();
    fixture.add_button();
    let mut saved = fixture.disk();
    saved.buses[0].inputs[0].instances.push(
        serde_json::from_value(json!({"number":1,"name":"Second","instance_type":1})).unwrap(),
    );
    fs::write(&fixture.paths.current, serde_json::to_vec(&saved).unwrap()).unwrap();
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    let old = "Bus1.Inputs.Panel.Instances.Button";
    wait_for(&handle, |s| s.input_listeners.contains(old)).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let before = handle.snapshot().unwrap().model;
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    handle.input_event_pause.armed.store(true, Ordering::SeqCst);
    emulator
        .emulator
        .control(
            Control::InputEvent {
                bus: 0,
                frame: 0x008001,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        handle.input_event_pause.entered.notified(),
    )
    .await
    .unwrap();
    let reply = command(
        &broker,
        json!({"command":"remove_from_list","target":"Bus1.Inputs.Panel","expected_revision":1}),
        1,
        "test/remove-input",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["revision"] == 2,
        "input removal did not complete: {reply}"
    );
    let removed = handle.snapshot().unwrap();
    assert!(
        removed.model.inventory().buses[0].inputs.is_empty(),
        "input module remained saved after removal"
    );
    assert!(
        removed.inputs.is_empty() && removed.input_listeners.is_empty(),
        "input removal retained child observations or listeners"
    );
    assert!(
        removed
            .model
            .projections()
            .iter()
            .all(|p| !p.target.contains(".Inputs.")),
        "input removal retained a descendant lifetime"
    );
    for target in ["Bus1.Outputs.Light1", "Bus1.Groups.All"] {
        assert_eq!(
            before
                .projections()
                .into_iter()
                .find(|p| p.target == target),
            removed
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
        );
    }
    let after = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        after["buses"][0]["inputs"], physical["buses"][0]["inputs"],
        "input removal changed hardware configuration"
    );
    handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    let report = handle
        .discover_address(0, Space::Inputs, 0, 2, context(2), deadline())
        .await
        .unwrap();
    assert_eq!(report.added_items, 3);
    let fresh = "Bus1.Inputs.Input1.Instances.Input1";
    wait_for(&handle, |s| s.input_listeners.contains(fresh)).await;
    assert!(handle
        .read_input(old, context(1), deadline())
        .await
        .is_err());
    emulator
        .emulator
        .control(
            Control::InputEvent {
                bus: 0,
                frame: 0x008000,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    wait_for(&handle, |s| s.inputs.contains_key(fresh)).await;
    handle.input_event_pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("DALI/Event/Controller")
                .is_empty())
            .await
    );
    assert!(running.stop().await);
    let events = broker.received_on("DALI/Event/Controller");
    assert_eq!(
        events.len(),
        1,
        "removed input action reached a new module lifetime"
    );
    let event: serde_json::Value = serde_json::from_slice(&events[0].payload).unwrap();
    assert_eq!(event["target"], fresh);
    assert_eq!(event["action"], "released");
    assert_eq!(
        events[0].user_property("target-generation"),
        context(3).generation()
    );
    assert!(handle
        .snapshot()
        .unwrap()
        .model
        .projections()
        .iter()
        .filter(|p| p.target.contains(".Inputs."))
        .all(|p| p.binding_revision == 3));
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn removal_failed_and_uncertain_saves_keep_existing_activation_rules() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected removal save failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(service, &handle, &broker).await;
        let result = handle
            .edit(
                1,
                MetadataEdit::RemoveOutput { bus: 0, address: 0 },
                context(1),
                deadline(),
            )
            .await;
        if at == Step::BeforeRename(Stage::Reservation) {
            assert!(matches!(result, Err(ServiceError::Rejected(_))));
            assert_eq!(fixture.disk().revision, 1);
            assert_eq!(
                handle.snapshot().unwrap().model.inventory().buses[0]
                    .outputs
                    .len(),
                1
            );
        } else {
            assert!(matches!(result, Err(ServiceError::Uncertain(_))));
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
            assert_eq!(
                handle.snapshot().unwrap().model.inventory().buses[0]
                    .outputs
                    .len(),
                1
            );
            let recovered = recover_when_idle(&handle).await;
            assert!(
                recovered.model.inventory().buses[0].outputs.is_empty(),
                "recovery did not adopt the durable removal"
            );
        }
        assert!(running.stop().await);
    }
}

async fn recover_when_idle(handle: &ServiceHandle) -> Snapshot {
    let until = deadline();
    tokio::time::timeout_at(until, async {
        loop {
            match handle.recover(context(1), until).await {
                Ok(snapshot) => break snapshot,
                Err(ServiceError::Busy) => tokio::time::sleep(Duration::from_millis(1)).await,
                Err(ServiceError::Uncertain(reason))
                    if reason == "configuration worker is busy" =>
                {
                    tokio::time::sleep(Duration::from_millis(1)).await
                }
                result => panic!("removal recovery failed: {result:?}"),
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn removal_held_save_keeps_permit_and_active_mapping_until_explicit_recovery() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(service, &handle, &broker).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::RemoveOutput { bus: 0, address: 0 },
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().buses[0]
            .outputs
            .len(),
        1,
        "pending removal changed active mapping"
    );
    assert!(
        matches!(
            handle
                .edit(1, MetadataEdit::Description(None), context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "pending removal released the global operation permit"
    );
    pending.abort();
    let _ = pending.await;
    wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
    hold.release.store(true, Ordering::SeqCst);
    assert!(recover_when_idle(&handle).await.model.inventory().buses[0]
        .outputs
        .is_empty());
    assert!(running.stop().await);
}

async fn command(
    broker: &FakeBroker,
    body: serde_json::Value,
    token: u64,
    topic: &str,
) -> serde_json::Value {
    assert!(broker.publish(
        "DALI/Command/Controller",
        body.to_string(),
        &SendOptions::default()
            .user_property("target-generation", context(token).generation().unwrap())
            .user_property("traceparent", TRACE)
            .response_topic(topic)
            .correlation_data(topic.as_bytes().to_vec())
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(topic).is_empty())
            .await,
        "remove-from-list command did not complete"
    );
    let records = broker.received_on(topic);
    assert_eq!(
        records.len(),
        1,
        "remove-from-list returned duplicate replies"
    );
    assert_eq!(
        records[0].user_property("target-generation"),
        context(token).generation()
    );
    assert_eq!(records[0].user_property("traceparent"), Some(TRACE));
    assert_eq!(
        records[0].correlation_data().unwrap().as_ref(),
        topic.as_bytes()
    );
    serde_json::from_slice(&records[0].payload).unwrap()
}

#[tokio::test]
async fn removal_mqtt_forgets_only_saved_output_and_rediscovery_starts_a_new_lifetime() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Other");
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let target = "Bus1.Outputs.Light1";
    handle
        .read_output(target, context(1), deadline())
        .await
        .unwrap();
    let state_topic = format!("DALI/State/Controller/{target}");
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&state_topic)
                .iter()
                .any(|r| !r.payload.is_empty()))
            .await
    );
    let before = handle.snapshot().unwrap().model;
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let reply = command(
        &broker,
        json!({"command":"remove_from_list","target":target,"expected_revision":1}),
        1,
        "test/remove-output",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["revision"] == 2,
        "saved output was not removed: {reply}"
    );
    let removed = handle.snapshot().unwrap();
    assert_eq!(
        fixture.disk().buses[0].outputs.len(),
        1,
        "remove-from-list did not remove the saved output"
    );
    assert_eq!(fixture.disk().buses[0].outputs[0].name, "Other");
    assert!(fixture.disk().buses[0].groups[0].members.is_empty());
    assert!(!removed.outputs.contains_key(target));
    assert!(handle
        .read_output(target, context(1), deadline())
        .await
        .is_err());
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&state_topic)
                .iter()
                .any(|r| r.payload.is_empty()))
            .await,
        "removed fixture State was not retired"
    );
    let after = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        after["buses"][0]["outputs"], physical["buses"][0]["outputs"],
        "removing a saved fixture mutated hardware"
    );
    let old_other = before
        .projections()
        .into_iter()
        .find(|p| p.target == "Bus1.Outputs.Other")
        .unwrap();
    assert_eq!(
        removed
            .model
            .projections()
            .into_iter()
            .find(|p| p.target == old_other.target)
            .unwrap(),
        old_other
    );
    let report = handle
        .discover_address(0, Space::Outputs, 0, 2, context(2), deadline())
        .await
        .unwrap();
    assert_eq!(
        report.revision, 3,
        "rediscovery did not add a fresh fixture lifetime"
    );
    let recreated = handle.snapshot().unwrap().model;
    let item = recreated
        .projections()
        .into_iter()
        .find(|p| p.target == target)
        .unwrap();
    assert_eq!(
        item.binding_revision, 3,
        "rediscovery reused a removed fixture binding"
    );
    assert!(
        recreated
            .ledger()
            .batches
            .last()
            .unwrap()
            .transitions
            .iter()
            .any(|t| t.from.is_none() && t.to.as_ref().is_some_and(|p| p.target == target)),
        "rediscovery resurrected a removed Origin"
    );
    assert!(
        handle
            .read_output(target, context(1), deadline())
            .await
            .is_err(),
        "old token reached a rediscovered fixture"
    );
    assert_eq!(
        fs::read(&fixture.paths.legacy).unwrap(),
        b"legacy must not be touched"
    );
    assert!(running.stop().await);
    assert_eq!(broker.received_on("test/remove-output").len(), 1);
    emulator.emulator.close().await.unwrap();
}
