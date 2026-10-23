use super::*;
use crate::service::group_output as group_service;
mod literal;

#[tokio::test]
async fn group_scalar_common_mutations_retire_only_their_bus_group_evidence() {
    let fixture = Fixture::new();
    fixture.add_button();
    let mut inventory = fixture.disk();
    inventory.buses[0].groups.push(
        serde_json::from_value(json!({"group_address":1,"name":"Sibling","members":[0]})).unwrap(),
    );
    inventory.buses[0]
        .scenes
        .push(serde_json::from_value(json!({"slot":0,"name":"Evening"})).unwrap());
    let mut second = inventory.buses[0].clone();
    second.bus = 1;
    second.name = "Bus2".into();
    second.inputs.clear();
    inventory.buses.push(second);
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let saved = fs::read(&fixture.paths.current).unwrap();
    let emulator = pty::spawn(serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[{"id":"one","short_address":0,"level":100,"groups":3,"scenes":[40,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]}],"inputs":[{"id":"panel","short_address":0,"instances":[1],"values":[{"instance":0,"resolution":1,"bytes":[255]}],"feedback":[{"instance":0}]}]},
        {"number":1,"outputs":[{"id":"other","short_address":0,"level":150,"groups":3}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let input = "Bus1.Inputs.Panel.Instances.Button";
    wait_for(&handle, |s| s.input_listeners.contains(input)).await;
    let mut notices = handle.subscribe_inputs();
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
    let notice = tokio::time::timeout(Duration::from_secs(2), notices.recv())
        .await
        .unwrap()
        .unwrap();
    let targets = ["Bus1.Groups.All", "Bus1.Groups.Sibling", "Bus2.Groups.All"];
    for operation in ["individual", "group", "scene", "membership", "clear"] {
        for target in targets {
            handle
                .read_group_output(target, context(1), deadline())
                .await
                .unwrap();
        }
        handle
            .read_input(input, context(1), deadline())
            .await
            .unwrap();
        assert!(
            handle
                .snapshot()
                .unwrap()
                .group_outputs
                .values()
                .all(|o| !o.stale),
            "input read retired group evidence"
        );
        handle
            .set_input_feedback(input, 1, true, context(1), deadline())
            .await
            .unwrap();
        assert!(
            handle
                .snapshot()
                .unwrap()
                .group_outputs
                .values()
                .all(|o| !o.stale),
            "input feedback retired group evidence"
        );
        let before_input = handle.snapshot().unwrap().inputs[input].clone();
        match operation {
            "individual" => {
                handle
                    .apply_output(
                        "Bus1.Outputs.Light1",
                        crate::dali::gear::Intent::new(None, Some(0.5)).unwrap(),
                        context(1),
                        deadline(),
                    )
                    .await
                    .unwrap();
            }
            "group" => {
                assert_eq!(
                    handle
                        .set_group_output(
                            targets[0],
                            1,
                            crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                            context(1),
                            deadline()
                        )
                        .await
                        .unwrap()
                        .outcome,
                    "sent"
                );
            }
            "scene" => {
                assert_eq!(
                    handle
                        .recall_scene("Bus1.Scenes.Evening", None, 1, context(1), deadline())
                        .await
                        .unwrap()
                        .outcome,
                    "sent"
                );
            }
            "membership" => {
                handle
                    .change_groups(
                        "Bus1.Outputs.Light1",
                        crate::service::groups::Action::Repair { reviewed_mask: 3 },
                        1,
                        context(1),
                        deadline(),
                    )
                    .await
                    .unwrap();
            }
            _ => {
                handle
                    .clear_short_address("Bus1.Outputs.Light1", 1, context(1), deadline())
                    .await
                    .unwrap();
            }
        }
        let snapshot = handle.snapshot().unwrap();
        assert_eq!(
            snapshot.group_outputs[targets[0]].stale,
            operation != "group",
            "selected group freshness survived a native mutation: {operation}"
        );
        assert!(
            snapshot.group_outputs[targets[1]].stale,
            "sibling group retained old actual evidence: {operation}"
        );
        assert!(
            !snapshot.group_outputs[targets[2]].stale,
            "another bus group was invalidated: {operation}"
        );
        assert!(
            notice.current(&snapshot),
            "group cache cut revoked continuing input notice: {operation}"
        );
        assert_eq!(
            snapshot.inputs[input], before_input,
            "output operation replaced input measurement: {operation}"
        );
        assert_eq!(
            fs::read(&fixture.paths.current).unwrap(),
            saved,
            "actual operation changed saved inventory: {operation}"
        );
    }
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_output_identification_blocks_group_writes_but_not_read_admission() {
    let fixture = Fixture::new();
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"member","short_address":0,"groups":1,"level":100}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    handle
        .identify_output(
            "Bus1.Outputs.Light1",
            false,
            context(1),
            Instant::now() + Duration::from_secs(4),
        )
        .await
        .unwrap();
    let snapshot = handle.snapshot().unwrap();
    let write = group_service::resolve(
        &snapshot,
        "Bus1.Groups.All",
        &group_service::Action::Set {
            expected: 1,
            intent: crate::dali::gear::Intent::new(Some(false), None).unwrap(),
        },
        &context(1),
    )
    .unwrap();
    assert!(
        matches!(
            group_service::admit(&handle, &write),
            Err(ServiceError::Rejected(_))
        ),
        "group write ignored active identification"
    );
    let read = group_service::resolve(
        &snapshot,
        "Bus1.Groups.All",
        &group_service::Action::Read,
        &context(1),
    )
    .unwrap();
    assert!(
        group_service::admit(&handle, &read).is_ok(),
        "identification unnecessarily blocked read admission"
    );
    handle
        .identify_output(
            "Bus1.Outputs.Light1",
            true,
            context(1),
            Instant::now() + Duration::from_secs(4),
        )
        .await
        .unwrap();
    assert!(group_service::admit(&handle, &write).is_ok());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_output_final_epoch_and_original_caller_deadline_refuse_current_success() {
    use crate::dali::gear::Intent;
    for writing in [false, true] {
        let fixture = Fixture::new();
        let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"member","short_address":0,"groups":1,"level":100}]}]})).unwrap()).await.unwrap();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        service.output_acceptance_hook =
            Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        if writing {
            let r = handle
                .set_group_output(
                    "Bus1.Groups.All",
                    1,
                    Intent::new(Some(false), None).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap();
            assert_eq!(
                r.outcome, "unconfirmed",
                "retired group write epoch reported sent"
            );
            assert!(r.values.is_none());
        } else {
            assert!(
                matches!(
                    handle
                        .read_group_output("Bus1.Groups.All", context(1), deadline())
                        .await,
                    Err(ServiceError::Rejected(_))
                ),
                "retired group read epoch reported current evidence"
            );
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
    use std::{future::Future, task::Poll};
    let fixture = Fixture::new();
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"member","short_address":0,"groups":1,"level":100}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let until = Instant::now() + Duration::from_millis(150);
    let mut pending = Box::pin(handle.read_group_output("Bus1.Groups.All", context(1), until));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    wait_for(&handle, |_| handle.permit.available_permits() == 1).await;
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(pending.await, Err(ServiceError::Rejected(_))),
        "ready group reply bypassed original caller expiry"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_output_invalidates_its_bus_actuals_but_preserves_other_bus_and_input_activity() {
    let fixture = Fixture::new();
    fixture.add_button();
    let mut inventory = fixture.disk();
    let mut second = inventory.buses[0].clone();
    second.bus = 1;
    second.name = "Bus2".into();
    second.inputs.clear();
    inventory.buses.push(second);
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let saved = fs::read(&fixture.paths.current).unwrap();
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[
        {"number":0,"outputs":[{"id":"first","short_address":0,"level":100,"groups":1}],"inputs":[{"id":"panel","short_address":0,"instances":[1]}]},
        {"number":1,"outputs":[{"id":"other","short_address":0,"level":150,"groups":1}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for target in ["Bus1.Outputs.Light1", "Bus2.Outputs.Light1"] {
        handle
            .read_output(target, context(1), deadline())
            .await
            .unwrap();
    }
    let input = "Bus1.Inputs.Panel.Instances.Button";
    wait_for(&handle, |s| s.input_listeners.contains(input)).await;
    let mut events = handle.subscribe_inputs();
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
    let notice = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    let before = handle.snapshot().unwrap().inputs[input].clone();
    let result = handle
        .set_group_output(
            "Bus1.Groups.All",
            1,
            crate::dali::gear::Intent::new(Some(false), None).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(result.outcome, "sent");
    let s = handle.snapshot().unwrap();
    assert_eq!(
        s.outputs["Bus1.Outputs.Light1"].quality,
        output::Quality::Stale,
        "native group send retained old current data"
    );
    assert_eq!(
        s.outputs["Bus2.Outputs.Light1"].quality,
        output::Quality::Current,
        "group send invalidated another bus"
    );
    assert_eq!(
        s.inputs[input], before,
        "group send replaced input evidence"
    );
    assert!(
        notice.current(&s),
        "group send revoked a continuing button notice"
    );
    let native = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(native["buses"][1]["outputs"][0]["level"], 150);
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), saved);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

#[tokio::test]
async fn group_output_mqtt_uses_native_membership_and_reports_actual_without_saving() {
    let fixture = Fixture::new();
    fixture.add_output(1, "ActualMember");
    fixture.add_output(2, "Outside");
    let saved = fs::read(&fixture.paths.current).unwrap();
    let scenario = serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"saved-only","short_address":0,"groups":0,"level":10},
        {"id":"member","short_address":1,"groups":1,"level":100},
        {"id":"outside","short_address":2,"groups":0,"level":200},
        {"id":"untracked","short_address":3,"groups":1,"level":200}]}]}))
    .unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
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
    handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    let read = groups::call(
        &broker,
        json!({"command":"read_group_output","target":"Bus1.Groups.All"}),
        1,
        "test/group-output/read",
    )
    .await;
    assert!(
        read.get("error").is_none(),
        "native group read rejected: {read}"
    );
    let read = &read["value"];
    assert_eq!(
        read["outcome"], "observed",
        "native group output command is unavailable: {read}"
    );
    assert_eq!(read["coverage"], "saved_inventory");
    assert_eq!(read["values"]["power_state"], true);
    assert_eq!(read["values"]["known_members"], 1);
    assert_eq!(read["values"]["brightness"], json!(100.0 / 254.0));
    assert_eq!(
        read["membership_matches"], false,
        "group read hid saved/native membership drift"
    );
    let applied=groups::call(&broker,json!({"command":"set_group_output","target":"Bus1.Groups.All","expected_revision":1,"power":false}),1,"test/group-output/off").await;
    assert!(
        applied.get("error").is_none(),
        "native group write rejected: {applied}"
    );
    let applied = &applied["value"];
    assert_eq!(applied["outcome"], "sent");
    assert_eq!(applied["values"]["power_state"], false);
    assert_eq!(applied["configuration_changed"], false);
    assert_eq!(
        applied["target_generation"],
        context(1).generation().unwrap()
    );
    let native = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let outputs = native["buses"][0]["outputs"].as_array().unwrap();
    for (id, level) in [
        ("saved-only", 10),
        ("member", 0),
        ("outside", 200),
        ("untracked", 0),
    ] {
        assert_eq!(
            outputs.iter().find(|o| o["id"] == id).unwrap()["level"],
            level,
            "group command used saved membership or individual fanout: {id}"
        );
    }
    assert_eq!(
        handle.snapshot().unwrap().outputs["Bus1.Outputs.Light1"].quality,
        output::Quality::Stale,
        "group mutation retained old current individual data"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), saved);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
