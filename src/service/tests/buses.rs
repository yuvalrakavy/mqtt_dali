use super::*;
use crate::emulator::session::Control;
use crate::{atx::transactions::RepeatPolicy, emulator::pty};

fn setup(enabled: bool) -> MetadataEdit {
    MetadataEdit::ConfigureBus {
        bus: 0,
        name: "Bus1".into(),
        enabled,
    }
}

#[tokio::test]
async fn bus_setup_queued_action_remains_valid_during_an_unrelated_metadata_save() {
    let fixture = Fixture::new();
    fixture.add_button();
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| {
        s.input_listeners
            .contains("Bus1.Inputs.Panel.Instances.Button")
    })
    .await;
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
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::Description(Some("updated description".into())),
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Writing);
    handle.input_event_pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(1), |b| !b
                .received_on("DALI/Event/Controller")
                .is_empty())
            .await,
        "unrelated metadata save dropped a valid queued button action"
    );
    hold.release.store(true, Ordering::SeqCst);
    pending.await.unwrap().unwrap();
    assert!(running.stop().await);
    assert_eq!(broker.received_on("DALI/Event/Controller").len(), 1);
    emulator.emulator.close().await.unwrap();
}

#[test]
fn bus_setup_model_preserves_children_and_lifetimes_and_validates_every_index() {
    let mut inventory = model().inventory().clone();
    inventory.buses[0].description = Some("existing wiring".into());
    inventory.buses[0].outputs[0].color_representation =
        Some(crate::config::ColorRepresentation::Rgbwaf);
    inventory.buses[0].inputs = serde_json::from_value(json!([
        {"short_address":0,"name":"Panel","instances":[{"number":0,"name":"Key","instance_type":1}]}])).unwrap();
    let first = Model::new(inventory).unwrap();
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
    let mut expected = first.inventory().buses[0].clone();
    expected.enabled = false;
    assert_eq!(
        disabled.inventory().buses[0],
        expected,
        "bus setup discarded saved child metadata"
    );
    assert_eq!(
        disabled.projections(),
        first.projections(),
        "enable-only edit replaced child lifetime/binding"
    );
    let repeated = disabled
        .edit(
            2,
            &[Edit::ConfigureBus {
                bus: 0,
                name: "Bus1".into(),
                enabled: false,
            }],
        )
        .unwrap();
    assert_eq!(
        repeated.inventory().revision,
        2,
        "no-op bus setup rotated revision"
    );
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
    for (old, new) in first.projections().iter().zip(renamed.projections()) {
        assert_eq!(new.location, old.location);
        assert_eq!(new.binding_revision, 3);
        assert!(new.target.starts_with("Upper."));
    }
    for index in 1..4 {
        let added = first
            .edit(
                1,
                &[Edit::ConfigureBus {
                    bus: index,
                    name: format!("Bus{}", index + 1),
                    enabled: true,
                }],
            )
            .unwrap();
        assert_eq!(added.inventory().buses[1].bus, index);
        assert!(added.inventory().buses[1].outputs.is_empty());
        assert_eq!(added.projections(), first.projections());
    }
    for (index, name) in [
        (4, "New"),
        (255, "New"),
        (1, "Bus1"),
        (1, ""),
        (1, "bad/name"),
    ] {
        assert!(
            first
                .edit(
                    1,
                    &[Edit::ConfigureBus {
                        bus: index,
                        name: name.into(),
                        enabled: true
                    }]
                )
                .is_err(),
            "invalid bus configuration was accepted"
        );
    }
    assert!(first
        .edit(
            0,
            &[Edit::ConfigureBus {
                bus: 1,
                name: "New".into(),
                enabled: true
            }]
        )
        .is_err());
}

#[tokio::test]
async fn bus_setup_strict_mqtt_and_stale_authority_do_not_change_inventory() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let valid = json!({"command":"configure_bus","bus":1,"name":"Second","enabled":true,"expected_revision":1});
    let mut invalid = Vec::new();
    for field in ["bus", "name", "enabled", "expected_revision"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        invalid.push(body);
    }
    for (field, value) in [
        ("bus", json!("1")),
        ("bus", json!(-1)),
        ("bus", json!(4)),
        ("name", json!(null)),
        ("enabled", json!(1)),
        ("enabled", json!("true")),
        ("extra", json!(1)),
        ("expected_revision", json!(0)),
        ("target", json!("Bus1.Outputs.Light1")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        invalid.push(body);
    }
    for (i, body) in invalid.into_iter().enumerate() {
        let reply = command(&broker, body, 1, &format!("test/bus-invalid/{i}")).await;
        assert!(
            reply.get("error").is_some(),
            "malformed bus setup was accepted: {reply}"
        );
        assert_eq!(fixture.disk().revision, 1);
    }
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("changed".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(
        handle
            .edit(
                2,
                MetadataEdit::ConfigureBus {
                    bus: 1,
                    name: "Retired".into(),
                    enabled: true
                },
                context(1),
                deadline()
            )
            .await
            .is_err(),
        "retired controller authority configured a bus"
    );
    let mut body = valid;
    body["expected_revision"] = json!(2);
    let reply = command(&broker, body, 1, "test/bus-stale").await;
    assert!(
        reply.get("error").is_some(),
        "retired controller authority configured a bus"
    );
    assert_eq!(fixture.disk().buses.len(), 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn bus_setup_disable_reenable_retires_values_and_queued_native_actions_with_stable_identity()
{
    let fixture = Fixture::new();
    fixture.add_button();
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
    let output = "Bus1.Outputs.Light1";
    let target = "Bus1.Inputs.Panel.Instances.Button";
    wait_for(&handle, |s| s.input_listeners.contains(target)).await;
    handle
        .read_output(output, context(1), deadline())
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
    wait_for(&handle, |s| s.inputs.contains_key(target)).await;
    let before = handle.snapshot().unwrap();
    assert!(before.outputs.contains_key(output));
    let disabled = handle
        .edit(1, setup(false), context(1), deadline())
        .await
        .unwrap();
    assert!(
        disabled.outputs.is_empty()
            && disabled.inputs.is_empty()
            && disabled.input_listeners.is_empty(),
        "disabled bus retained fixture values or listeners"
    );
    assert!(
        handle
            .read_output(output, context(1), deadline())
            .await
            .is_err(),
        "disabled bus admitted physical output work"
    );
    assert!(
        handle
            .read_input(target, context(1), deadline())
            .await
            .is_err(),
        "disabled bus admitted input queries"
    );
    let enabled = handle
        .edit(2, setup(true), context(2), deadline())
        .await
        .unwrap();
    assert!(
        enabled.outputs.is_empty() && enabled.inputs.is_empty(),
        "re-enabled bus revived retired fixture values"
    );
    assert_eq!(
        enabled.model.projections(),
        before.model.projections(),
        "enable transition rotated child identities"
    );
    wait_for(&handle, |s| s.input_listeners.contains(target)).await;
    // Queue a new lifetime's release while the old press still waits in the helper.
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
    wait_for(&handle, |s| {
        s.inputs
            .get(target)
            .is_some_and(|o| o.evidence != before.inputs[target].evidence)
    })
    .await;
    handle.input_event_pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("DALI/Event/Controller")
                .is_empty())
            .await
    );
    assert!(running.stop().await);
    let actions: Vec<serde_json::Value> = broker
        .received_on("DALI/Event/Controller")
        .iter()
        .map(|r| serde_json::from_slice(&r.payload).unwrap())
        .collect();
    assert_eq!(
        actions.len(),
        1,
        "old button action survived bus disable/re-enable"
    );
    assert_eq!(
        actions[0]["action"], "released",
        "old button action survived bus disable/re-enable"
    );
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn bus_setup_held_and_uncertain_save_changes_live_selection_only_after_activation() {
    let fixture = Fixture::new();
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let client = handle.clone();
    let pending =
        tokio::spawn(async move { client.edit(1, setup(false), context(1), deadline()).await });
    hold.entered().await;
    assert!(
        handle.snapshot().unwrap().model.inventory().buses[0].enabled,
        "pending save changed active bus selection"
    );
    assert!(
        matches!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "bus save released the global permit before activation"
    );
    pending.abort();
    let _ = pending.await;
    wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
    assert!(handle.snapshot().unwrap().model.inventory().buses[0].enabled);
    hold.release.store(true, Ordering::SeqCst);
    let until = deadline();
    let recovered = tokio::time::timeout_at(until, async {
        loop {
            match handle.recover(context(1), until).await {
                Ok(snapshot) => break snapshot,
                Err(ServiceError::Busy) => {
                    // Phase publication precedes releasing the canceled owner's
                    // permit; explicit recovery remains nonwaiting in production.
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                Err(ServiceError::Uncertain(reason))
                    if reason == "configuration worker is busy" =>
                {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                other => panic!("bus recovery failed: {other:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert!(
        !recovered.model.inventory().buses[0].enabled,
        "recovery did not adopt durable bus selection"
    );
    assert!(recovered.outputs.is_empty() && recovered.input_listeners.is_empty());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn bus_setup_failed_save_does_not_change_model_or_live_routes() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.hooks = Hooks {
        callback: Some(Arc::new(|step| {
            if step == Step::BeforeRename(Stage::Reservation) {
                Err("injected bus save failure".into())
            } else {
                Ok(())
            }
        })),
    };
    let running = start(service, &handle, &broker).await;
    assert!(matches!(
        handle.edit(1, setup(false), context(1), deadline()).await,
        Err(ServiceError::Rejected(_))
    ));
    assert!(handle.snapshot().unwrap().model.inventory().buses[0].enabled);
    assert!(fixture.disk().buses[0].enabled);
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
}

async fn command(
    broker: &FakeBroker,
    body: serde_json::Value,
    revision: u64,
    topic: &str,
) -> serde_json::Value {
    assert!(broker.publish(
        "DALI/Command/Controller",
        body.to_string(),
        &SendOptions::default()
            .user_property("target-generation", context(revision).generation().unwrap())
            .user_property(
                "traceparent",
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            )
            .response_topic(topic)
            .correlation_data(topic.as_bytes().to_vec()),
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(topic).is_empty())
            .await,
        "bus setup command never completed"
    );
    let replies = broker.received_on(topic);
    assert_eq!(replies.len(), 1, "bus setup returned duplicate replies");
    assert_eq!(
        replies[0].correlation_data().unwrap().as_ref(),
        topic.as_bytes(),
        "bus setup lost original command correlation"
    );
    assert_eq!(
        replies[0].user_property("target-generation"),
        context(revision).generation(),
        "bus setup lost original command authority"
    );
    assert_eq!(
        replies[0].user_property("traceparent"),
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    );
    serde_json::from_slice(&replies[0].payload).unwrap()
}

#[tokio::test]
async fn bus_setup_mqtt_bootstraps_empty_inventory_and_discovers_nonzero_bus_without_restart() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses.clear();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let scenario = serde_json::from_value(json!({"version":1,"buses":[
        {"number":0},
        {"number":2,"outputs":[{"id":"new-light","short_address":0}]}
    ]}))
    .unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(3))
            .await
    );
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let reply = command(
        &broker,
        json!({"command":"configure_bus","bus":2,"name":"Upper",
        "enabled":true,"expected_revision":1}),
        1,
        "test/bus-setup",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["revision"] == 2,
        "fresh installation could not configure a bus: {reply}"
    );
    assert_eq!(fixture.disk().buses.len(), 1);
    assert!(
        fixture.disk().buses[0].outputs.is_empty(),
        "bus setup invented fixtures"
    );
    wait_for(&handle, |s| {
        matches!(&s.hardware, Hardware::Ready {buses,..}
        if buses.iter().any(|b| b.bus==2 && b.status==2))
    })
    .await;
    let reply = command(
        &broker,
        json!({"command":"discover_address","bus":2,"space":"outputs",
        "address":0,"expected_revision":2}),
        2,
        "test/bus-discovery",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["revision"] == 3,
        "new bus could not discover without restarting: {reply}"
    );
    assert_eq!(fixture.disk().buses[0].name, "Upper");
    assert_eq!(fixture.disk().buses[0].outputs[0].name, "Light1");
    assert_eq!(
        fs::read(&fixture.paths.legacy).unwrap(),
        b"legacy must not be touched"
    );
    assert!(running.stop().await);
    assert_eq!(broker.received_on("test/bus-setup").len(), 1);
    emulator.emulator.close().await.unwrap();
}
