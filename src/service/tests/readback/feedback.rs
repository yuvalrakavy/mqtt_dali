use super::settings_peer::SettingsPeer;
use super::*;

fn with_feedback() -> (Fixture, Scenario) {
    let (fixture, mut scenario) = configured();
    scenario.buses[0].inputs[0].feedback =
        serde_json::from_value(json!([{"instance":0},{"instance":null}])).unwrap();
    (fixture, scenario)
}

#[tokio::test]
async fn managed_feedback_disabled_bus_cannot_read_or_activate_saved_inputs() {
    let (fixture, scenario) = with_feedback();
    let mut inventory = fixture.disk();
    inventory.buses[0].enabled = false;
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
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
    wait_for(&handle, |s| s.phase == Phase::Ready).await;
    for t in ["Bus1.Inputs.Panel".to_string(), target("Button")] {
        for writing in [false, true] {
            let r = if writing {
                handle
                    .set_input_feedback(&t, 1, true, context(1), deadline())
                    .await
            } else {
                handle.read_input_feedback(&t, context(1), deadline()).await
            };
            assert!(
                matches!(r,Err(ServiceError::Rejected(ref e))if e.contains("disabled")),
                "disabled input bus accepted feedback work"
            );
        }
    }
    let native = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        native["buses"][0]["inputs"][0]["feedback_state"][0]["active"],
        false
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_feedback_mqtt_read_and_set_preserve_actual_original_identity() {
    let (fixture, scenario) = with_feedback();
    let original = fs::read(&fixture.paths.current).unwrap();
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
    for (index, command, params, active, outcome) in [
        (0, "read_input_feedback", json!({}), false, "current"),
        (
            1,
            "set_input_feedback",
            json!({"expected_revision":1,"active":true}),
            true,
            "set",
        ),
        (
            2,
            "set_input_feedback",
            json!({"expected_revision":1,"active":false}),
            false,
            "set",
        ),
    ] {
        let reply = format!("test/input-feedback/{index}");
        let mut call = json!({"command":command,"target":target("Button")});
        call.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
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
        let report: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert!(
            report.get("error").is_none(),
            "managed feedback command is unsupported: {report}"
        );
        assert_eq!(report["value"]["outcome"], outcome);
        assert_eq!(report["value"]["feedback"]["features"], json!([32]));
        assert_eq!(report["value"]["feedback"]["capability"], 1);
        assert_eq!(report["value"]["feedback"]["supported"], true);
        assert_eq!(report["value"]["feedback"]["active"], active);
        assert_eq!(
            replies[0].user_property("target-generation"),
            context(1).generation()
        );
    }
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_feedback_device_and_instance_keep_input_values_and_notices() {
    let (fixture, scenario) = with_feedback();
    let original = fs::read(&fixture.paths.current).unwrap();
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
    let before = handle.snapshot().unwrap().inputs[&target("Button")].clone();
    for t in ["Bus1.Inputs.Panel".to_string(), target("Button")] {
        for active in [true, true, false] {
            let r = handle
                .set_input_feedback(&t, 1, active, context(1), deadline())
                .await
                .unwrap();
            assert_eq!(r.outcome, "set");
            assert_eq!(r.instance, t.ends_with("Button").then_some(0));
            assert_eq!(r.feedback.unwrap().active, Some(active));
            assert!(
                notice.current(&handle.snapshot().unwrap()),
                "feedback revoked a continuing button notice"
            );
            assert_eq!(
                handle.snapshot().unwrap().inputs[&target("Button")],
                before,
                "feedback replaced native input value evidence"
            );
        }
    }
    let unsupported = handle
        .read_input_feedback(&target("Contact"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(unsupported.outcome, "unsupported");
    assert_eq!(unsupported.feedback.unwrap().features, Some(vec![]));
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

fn feedback_script(mode: &str, active: bool, device: bool) -> Vec<(String, String)> {
    let present = if device { "l01FE30\n" } else { "l010080\n" };
    let present_reply = if device { "J00\n" } else { "J01\n" };
    let ordinary = if device { "FE" } else { "00" };
    let feature = if device { "FC" } else { "20" };
    let mut s = vec![(
        present.into(),
        if mode == "absent" {
            "N\n"
        } else if mode == "wrong_type" {
            "J02\n"
        } else {
            present_reply
        }
        .into(),
    )];
    if mode == "absent" || mode == "wrong_type" {
        return s;
    }
    s.push((
        format!("l01{ordinary}8E\n"),
        match mode {
            "none" => "JFE\n",
            "missing_feature" => "N\n",
            "multiple" | "duplicate" | "unfinished" => "JFF\n",
            _ => "J20\n",
        }
        .into(),
    ));
    if matches!(mode, "multiple" | "duplicate" | "unfinished") {
        for reply in if mode == "multiple" {
            vec!["J20\n", "J21\n", "JFE\n"]
        } else if mode == "duplicate" {
            vec!["J20\n", "J20\n"]
        } else {
            vec!["J20\n", "J21\n", "N\n"]
        } {
            s.push((format!("l01{ordinary}8F\n"), reply.into()));
        }
    }
    if !matches!(
        mode,
        "none" | "missing_feature" | "duplicate" | "unfinished"
    ) {
        s.push((
            format!("l01{feature}4F\n"),
            match mode {
                "cap_missing" => "N\n",
                "cap_zero" => "J00\n",
                "cap_unknown" => "J40\n",
                "future" => "J41\n",
                _ => "J01\n",
            }
            .into(),
        ));
        if !matches!(mode, "cap_missing" | "cap_zero" | "cap_unknown") {
            s.push((
                format!("l01{feature}4E\n"),
                if mode == "active_collision" {
                    "X2\n"
                } else if mode == "active_bad" {
                    "J00\n"
                } else if active {
                    "JFF\n"
                } else {
                    "N\n"
                }
                .into(),
            ));
        }
    }
    s.push((
        present.into(),
        if mode == "closing_missing" {
            "N\n"
        } else if mode == "closing_wrong" {
            "J02\n"
        } else {
            present_reply
        }
        .into(),
    ));
    s
}

#[tokio::test]
async fn managed_feedback_literal_evidence_preflight_and_actual_match() {
    for (mode, writing, device) in [
        ("none", true, false),
        ("cap_missing", true, false),
        ("cap_unknown", true, false),
        ("active_collision", false, false),
        ("active_collision", true, false),
        ("closing_missing", false, false),
        ("closing_missing", true, false),
        ("current", false, false),
        ("current", false, true),
        ("absent", false, false),
        ("wrong_type", false, false),
        ("missing_feature", false, false),
        ("multiple", false, false),
        ("duplicate", true, false),
        ("unfinished", true, false),
        ("cap_zero", true, false),
        ("future", false, false),
        ("active_bad", true, false),
        ("closing_wrong", false, false),
        ("ignored", true, false),
        ("equal", true, false),
        ("ignored", true, true),
        ("equal", true, true),
    ] {
        let (fixture, _) = configured();
        let mut steps = feedback_script(mode, mode == "equal", device);
        let before = steps.iter().map(|s| s.0.clone()).collect::<Vec<_>>();
        let sending = writing && matches!(mode, "ignored" | "equal");
        if sending {
            steps.push((
                if device { "l01FC10\n" } else { "l012010\n" }.into(),
                "N\n".into(),
            ));
            steps.extend(feedback_script(mode, mode == "equal", device));
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
        let t = if device {
            "Bus1.Inputs.Panel".into()
        } else {
            target("Button")
        };
        let result = if writing {
            handle
                .set_input_feedback(&t, 1, true, context(1), deadline())
                .await
        } else {
            handle.read_input_feedback(&t, context(1), deadline()).await
        };
        if writing && !sending {
            let sent = peer
                .commands
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.starts_with('l'))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(sent,before,"feedback preflight sent an action without complete support/presence evidence: {mode}");
        }
        let r = result.unwrap();
        let expected = match mode {
            "none" | "cap_zero" => "unsupported",
            "wrong_type" | "closing_wrong" => "type_mismatch",
            "current" | "multiple" | "future" => "current",
            "ignored" => "unconfirmed",
            "equal" => "set",
            _ => "unavailable",
        };
        assert_eq!(
            r.outcome, expected,
            "feedback lacked complete actual evidence: {mode}"
        );
        if matches!(
            mode,
            "absent" | "wrong_type" | "closing_missing" | "closing_wrong"
        ) {
            assert!(
                r.feedback.is_none(),
                "missing closing presence exposed a false feedback state"
            );
        } else {
            let v = r.feedback.unwrap();
            if matches!(mode, "active_collision" | "active_bad") {
                assert_eq!(
                    v.active, None,
                    "feedback collision or invalid byte became false"
                );
            }
            if matches!(mode, "missing_feature" | "duplicate" | "unfinished") {
                assert!(
                    v.features.is_none(),
                    "unknown feature evidence became an empty list"
                );
            }
            if mode == "multiple" {
                assert_eq!(v.features, Some(vec![32, 33]));
            }
            if mode == "future" {
                assert_eq!(v.capability, Some(65));
                assert_eq!(v.active, Some(false));
            }
            if mode == "cap_unknown" {
                assert_eq!(v.capability, Some(64));
                assert_eq!(v.supported, None);
            }
            if mode == "ignored" {
                assert_eq!(v.active, Some(false));
            }
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_feedback_final_epoch_and_original_deadline_refuse_success() {
    for writing in [false, true] {
        let (fixture, scenario) = with_feedback();
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
        if writing {
            let r = handle
                .set_input_feedback(&target("Button"), 1, true, context(1), deadline())
                .await
                .unwrap();
            assert_eq!(
                r.outcome, "unconfirmed",
                "retired feedback epoch reported success"
            );
            assert!(r.feedback.is_none());
        } else {
            assert!(
                matches!(
                    handle
                        .read_input_feedback(&target("Button"), context(1), deadline())
                        .await,
                    Err(ServiceError::Rejected(_))
                ),
                "retired feedback read epoch reported success"
            );
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
    use std::{future::Future, task::Poll};
    let (fixture, scenario) = with_feedback();
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
    let t = target("Button");
    let until = Instant::now() + Duration::from_millis(150);
    let mut read = Box::pin(handle.read_input_feedback(&t, context(1), until));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(read.await, Err(ServiceError::Rejected(_))),
        "ready feedback reply bypassed original caller expiry"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_feedback_strict_targets_generations_revisions_and_mqtt_params() {
    let (fixture, scenario) = with_feedback();
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
    for (t, expected, cause) in [
        (target("Button"), 0, context(1)),
        (target("Button"), 1, context(2)),
        (target("Button"), 1, RequestContext::default()),
        ("Bus1.Outputs.Kitchen".into(), 1, context(1)),
        (target("missing"), 1, context(1)),
    ] {
        assert!(
            matches!(
                handle
                    .set_input_feedback(&t, expected, true, cause, deadline())
                    .await,
                Err(ServiceError::Rejected(_))
            ),
            "invalid feedback target/authority reached hardware"
        );
    }
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (index, command, params) in [
        (0, "read_input_feedback", json!({"active":true})),
        (
            1,
            "set_input_feedback",
            json!({"expected_revision":1,"active":null}),
        ),
        (
            2,
            "set_input_feedback",
            json!({"expected_revision":1,"active":1}),
        ),
        (
            3,
            "set_input_feedback",
            json!({"expected_revision":1,"active":"true"}),
        ),
        (
            4,
            "set_input_feedback",
            json!({"expected_revision":1,"active":true,"unknown":0}),
        ),
        (5, "set_input_feedback", json!({"active":true})),
    ] {
        let reply = format!("test/invalid-feedback/{index}");
        let mut call = json!({"command":command,"target":target("Button")});
        call.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
        assert!(broker.publish(
            "DALI/Command/Controller",
            call.to_string(),
            &SendOptions::default()
                .user_property("target-generation", context(1).generation().unwrap())
                .response_topic(&reply)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| !b
                    .received_on(&reply)
                    .is_empty())
                .await
        );
        let replies = broker.received_on(&reply);
        assert_eq!(replies.len(), 1);
        let r: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert!(
            r.get("error").is_some(),
            "invalid feedback params accepted: {r}"
        );
    }
    let r = handle
        .read_input_feedback(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(r.feedback.unwrap().active, Some(false));
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
