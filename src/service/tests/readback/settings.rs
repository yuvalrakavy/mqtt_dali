use super::settings_peer::{script, SettingsPeer};

#[tokio::test]
async fn managed_input_settings_ready_reply_retains_original_caller_deadline() {
    use std::{future::Future, task::Poll};
    let (fixture, scenario) = configured();
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
    let target = target("Button");
    let until = Instant::now() + Duration::from_millis(150);
    let mut read = Box::pin(handle.read_input_settings(&target, context(1), until));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(read.await, Err(ServiceError::Rejected(_))),
        "ready input settings bypassed caller expiry"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
use super::*;
use crate::service::input::settings::Patch;

#[tokio::test]
async fn managed_input_settings_literal_enabled_collision_partial_and_type_proofs() {
    for mode in [
        "disabled",
        "receive_collision",
        "transmit_collision",
        "bad_yes",
        "gone",
        "wrong_type",
        "wide",
        "partial",
    ] {
        let (fixture, _) = configured();
        let wide = matches!(mode, "wide" | "partial");
        let enabled = match mode {
            "receive_collision" => "X2\n",
            "transmit_collision" => "Z\n",
            "bad_yes" => "J00\n",
            _ => "N\n",
        };
        let mut steps = script(if wide { 6 } else { 0 }, if wide { 31 } else { 1 }, enabled);
        if mode == "gone" {
            steps.last_mut().unwrap().1 = "N\n".into();
        }
        if mode == "wrong_type" {
            steps = vec![("l010080\n".into(), "J02\n".into())];
        }
        if mode == "partial" {
            steps[9].1 = "N\n".into();
        }
        let peer = SettingsPeer::new(steps);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let r = handle
            .read_input_settings(
                &target(if wide { "Unknown" } else { "Button" }),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        match mode {
            "disabled" | "wide" => {
                assert_eq!(r.outcome, "current");
                assert_eq!(r.settings.as_ref().unwrap().enabled, Some(false));
                if wide {
                    assert_eq!(
                        r.settings.unwrap().event_filter,
                        Some(0xffffff),
                        "wide FF filter became MASK"
                    );
                }
            }
            "gone" => assert!(r.settings.is_none(), "missing final type became disabled"),
            "wrong_type" => assert_eq!(r.outcome, "type_mismatch"),
            "partial" => {
                assert_eq!(
                    r.outcome, "unavailable",
                    "missing filter byte became complete"
                );
                assert_eq!(
                    r.settings.unwrap().event_filter,
                    None,
                    "missing filter byte became zero"
                );
            }
            _ => {
                assert_eq!(r.outcome, "unavailable");
                assert_eq!(
                    r.settings.unwrap().enabled,
                    None,
                    "collision or invalid YES became disabled"
                );
            }
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_input_settings_literal_repeat_and_actual_patch_match() {
    for ignored in [false, true] {
        let (fixture, _) = configured();
        let mut steps = script(0, 1, "JFF\n");
        steps.extend([
            ("l010063\n".into(), "N\n".into()),
            ("l010063\n".into(), "N\n".into()),
        ]);
        steps.extend(script(0, 1, if ignored { "JFF\n" } else { "N\n" }));
        let peer = SettingsPeer::new(steps);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let r = handle
            .configure_input(
                &target("Button"),
                1,
                Patch {
                    enabled: Some(false),
                    ..Patch::default()
                },
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            r.outcome,
            if ignored { "unconfirmed" } else { "configured" },
            "native input send was accepted without matching readback"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_input_settings_mqtt_preserves_original_reply_and_strict_params() {
    let (fixture, scenario) = configured();
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
    for (index, extra, success) in [
        (
            0,
            json!({"expected_revision":1,"settings":{"enabled":false}}),
            true,
        ),
        (
            1,
            json!({"expected_revision":1,"settings":{"enabled":null}}),
            false,
        ),
        (
            2,
            json!({"expected_revision":1,"settings":{"enabled":true},"extra":true}),
            false,
        ),
    ] {
        let reply = format!("test/input-settings/{index}");
        let mut call = json!({"command":"configure_input","target":target("Button")});
        call.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(broker.publish(
            "DALI/Command/Controller",
            call.to_string(),
            &SendOptions::default()
                .user_property("target-generation", context(1).generation().unwrap())
                .response_topic(&reply)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(3), |b| !b
                    .received_on(&reply)
                    .is_empty())
                .await
        );
        let replies = broker.received_on(&reply);
        assert_eq!(replies.len(), 1);
        let value: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(
            value.get("error").is_none(),
            success,
            "strict input setting command accepted wrong parameters: {value}"
        );
        if success {
            assert_eq!(value["value"]["outcome"], "configured");
            assert_eq!(
                replies[0].user_property("target-generation"),
                context(1).generation()
            );
        }
    }
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_input_settings_common_types_patch_and_saved_identity() {
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
    let original = fs::read(&fixture.paths.current).unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for (name, mask) in [
        ("Button", 255),
        ("Contact", 1),
        ("Occupancy", 31),
        ("Light", 1),
    ] {
        let target = target(name);
        let before = handle
            .read_input_settings(&target, context(1), deadline())
            .await
            .expect("managed input event settings read missing");
        assert_eq!(before.outcome, "current");
        assert_eq!(before.settings.unwrap().event_filter, Some(mask));
        let patch = Patch {
            enabled: Some(false),
            event_priority: Some(2),
            event_scheme: Some(2),
            event_filter: Some(0),
            groups: Some([Some(0), None, Some(31)]),
        };
        let report = handle
            .configure_input(&target, 1, patch, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(
            report.outcome, "configured",
            "input patch was not confirmed: {report:?}"
        );
        let values = report.settings.unwrap();
        assert_eq!(values.enabled, Some(false));
        assert_eq!(values.event_priority, Some(2));
        assert_eq!(values.event_filter, Some(0));
        assert_eq!(values.groups, [Some(0), Some(255), Some(31)]);
        assert!(!report.configuration_changed);
        assert_eq!(report.target_generation, context(1).generation().unwrap());
        assert_eq!(
            handle
                .read_input_settings(&target, context(1), deadline())
                .await
                .unwrap()
                .settings
                .unwrap(),
            values
        );
    }
    assert_eq!(
        fs::read(&fixture.paths.current).unwrap(),
        original,
        "hardware input settings rewrote saved identity"
    );
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_input_settings_refuse_stale_invalid_unknown_filters_and_missing_type() {
    let (fixture, scenario) = configured();
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
    for (name, revision, cause, patch) in [
        (
            "Button",
            0,
            context(1),
            Patch {
                enabled: Some(false),
                ..Patch::default()
            },
        ),
        (
            "Button",
            1,
            context(2),
            Patch {
                enabled: Some(false),
                ..Patch::default()
            },
        ),
        (
            "Button",
            1,
            RequestContext::default(),
            Patch {
                enabled: Some(false),
                ..Patch::default()
            },
        ),
        ("Button", 1, context(1), Patch::default()),
        (
            "Button",
            1,
            context(1),
            Patch {
                event_priority: Some(1),
                ..Patch::default()
            },
        ),
        (
            "Button",
            1,
            context(1),
            Patch {
                event_scheme: Some(5),
                ..Patch::default()
            },
        ),
        (
            "Contact",
            1,
            context(1),
            Patch {
                event_filter: Some(2),
                ..Patch::default()
            },
        ),
        (
            "Unknown",
            1,
            context(1),
            Patch {
                event_filter: Some(1),
                ..Patch::default()
            },
        ),
        (
            "Button",
            1,
            context(1),
            Patch {
                groups: Some([Some(32), None, None]),
                ..Patch::default()
            },
        ),
    ] {
        assert!(
            handle
                .configure_input(&target(name), revision, patch, cause, deadline())
                .await
                .is_err(),
            "invalid/stale input settings reached hardware"
        );
    }
    assert_eq!(
        handle
            .read_input_settings(&target("Button"), context(1), deadline())
            .await
            .unwrap()
            .settings
            .unwrap()
            .enabled,
        Some(true)
    );
    for payload in [
        json!({"enabled":null}),
        json!({"event_filter":-1}),
        json!({"event_priority":2.5}),
        json!({"groups":[0,null]}),
        json!({"unknown":true}),
    ] {
        assert!(
            serde_json::from_value::<Patch>(payload).is_err(),
            "malformed input patch decoded"
        );
    }
    control(
        &emulator.emulator,
        Control::SetAddress {
            id: "panel".into(),
            short_address: Some(1),
        },
    )
    .await;
    let missing = handle
        .read_input_settings(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(missing.outcome, "unavailable");
    assert!(missing.settings.is_none(), "missing input became disabled");
    let refused = handle
        .configure_input(
            &target("Button"),
            1,
            Patch {
                enabled: Some(false),
                ..Patch::default()
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(refused.outcome, "unavailable");
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_input_settings_final_epoch_cannot_report_success() {
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
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
    let report = handle
        .configure_input(
            &target("Button"),
            1,
            Patch {
                enabled: Some(false),
                ..Patch::default()
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        report.outcome, "unconfirmed",
        "retired physical epoch reported configured input"
    );
    assert!(report.settings.is_none());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_input_settings_revoke_old_notices_and_do_not_replace_values() {
    let (fixture, scenario) = configured();
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
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    wait_for(&handle, |s| s.input_listeners.contains(&target("Button"))).await;
    let mut events = handle.subscribe_inputs();
    control(
        &emulator.emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x008001,
        },
    )
    .await;
    let notice = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(notice.current(&handle.snapshot().unwrap()));
    let before = handle.snapshot().unwrap().inputs[&target("Button")].clone();
    handle
        .read_input_settings(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        handle.snapshot().unwrap().inputs[&target("Button")],
        before,
        "settings read replaced native value evidence"
    );
    control(
        &emulator.emulator,
        Control::NextFault {
            effect: crate::emulator::session::Fault::Delay { millis: 200 },
        },
    )
    .await;
    let writer = handle.clone();
    let configuring = tokio::spawn(async move {
        writer
            .configure_input(
                &target("Button"),
                1,
                Patch {
                    event_priority: Some(2),
                    ..Patch::default()
                },
                context(1),
                deadline(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let native = emulator
                .emulator
                .control(Control::Snapshot, Duration::from_secs(1))
                .await
                .unwrap();
            if native["transport"]["scheduled_frames"].as_u64().unwrap() > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !notice.current(&handle.snapshot().unwrap()),
        "old press remained valid at input settings admission"
    );
    configuring.await.unwrap().unwrap();
    assert!(
        !notice.current(&handle.snapshot().unwrap()),
        "old press survived input setting change"
    );
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert!(
        !notice.current(&handle.snapshot().unwrap()),
        "old press revived after fresh query"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
