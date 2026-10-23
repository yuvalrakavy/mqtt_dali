use super::settings_peer::{script, SettingsPeer};
use super::*;
use crate::service::input::settings::Patch;

fn timing_script(mode: &str, after: bool) -> Vec<(String, String)> {
    let mut steps = script(
        0,
        1,
        if after && mode == "partial_common" {
            "N\n"
        } else {
            "JFF\n"
        },
    );
    for (command, reply) in &mut steps {
        if command == "l01000D\n" {
            if matches!(mode, "missing_min" | "partial_common") {
                *reply = "N\n".into();
            }
            if mode == "invalid_min" {
                *reply = "JFF\n".into();
            }
            if mode == "later_min" {
                *reply = "J14\n".into();
            }
        }
    }
    steps
}

#[tokio::test]
async fn managed_button_timing_literal_minimum_proofs_preflight_and_ignored_write() {
    for mode in [
        "missing_min",
        "invalid_min",
        "later_min",
        "ignored",
        "partial_common",
    ] {
        let (fixture, _) = configured();
        let mut steps = timing_script(mode, false);
        let before_only = steps.iter().map(|s| s.0.clone()).collect::<Vec<_>>();
        if mode == "ignored" {
            steps.extend([
                ("lC13032\n".into(), "N\n".into()),
                ("l010000\n".into(), "N\n".into()),
                ("l010000\n".into(), "N\n".into()),
            ]);
            steps.extend(timing_script(mode, true));
        } else if mode == "partial_common" {
            steps.extend([
                ("l010063\n".into(), "N\n".into()),
                ("l010063\n".into(), "N\n".into()),
            ]);
            steps.extend(timing_script(mode, true));
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
        let patch = match mode {
            "ignored" => json!({"button_timers":{"short":50}}),
            "partial_common" => json!({"enabled":false}),
            "later_min" => json!({"enabled":false,"button_timers":{"short":50,"double":10}}),
            _ => json!({"enabled":false,"button_timers":{"double":0}}),
        };
        let r = handle
            .configure_input(
                &target("Button"),
                1,
                serde_json::from_value(patch).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        if mode == "partial_common" {
            assert_eq!(
                r.outcome, "configured",
                "partial timer evidence blocked a common setting"
            );
            let values = serde_json::to_value(r.settings.unwrap()).unwrap();
            assert_eq!(values["button_timers"]["quality"], "partial");
            assert!(
                values["button_timers"]["double_min"].is_null(),
                "missing timer minimum became zero"
            );
        } else {
            assert_eq!(
                r.outcome, "unconfirmed",
                "timer write succeeded without complete matching actual timing"
            );
            if mode != "ignored" {
                let input_commands = peer
                    .commands
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|s| s.starts_with('l'))
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(
                    input_commands, before_only,
                    "timer preflight sent a setting before checking the whole patch"
                );
            }
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_button_timing_strict_patch_and_non_button_refusal() {
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
    for value in [
        json!(null),
        json!({}),
        json!({"short":null}),
        json!({"double":1}),
        json!({"short":256}),
        json!({"repeat":4}),
        json!({"repeat":101}),
        json!({"stuck":4}),
        json!({"short":25.5}),
        json!({"short_min":25}),
        json!({"unknown":1}),
    ] {
        if let Ok(patch) = serde_json::from_value::<Patch>(json!({"button_timers":value})) {
            assert!(
                matches!(
                    handle
                        .configure_input(&target("Button"), 1, patch, context(1), deadline())
                        .await,
                    Err(ServiceError::Rejected(_))
                ),
                "invalid timing patch reached hardware"
            );
        }
    }
    let patch = serde_json::from_value(json!({"button_timers":{"short":50}})).unwrap();
    assert!(matches!(
        handle
            .configure_input(&target("Contact"), 1, patch, context(1), deadline())
            .await,
        Err(ServiceError::Rejected(_))
    ));
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_button_timing_patch_preserves_identity_values_and_native_fields() {
    let (fixture, scenario) = configured();
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
    let mut events = handle.subscribe_inputs();
    for timers in [
        json!({"short":255,"double":100,"repeat":5,"stuck":255}),
        json!({"short":50,"double":0,"repeat":100,"stuck":5}),
    ] {
        let patch: Patch = serde_json::from_value(json!({"button_timers":timers}))
            .expect("button timer patch is unsupported");
        let report = handle
            .configure_input(&target("Button"), 1, patch, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(
            report.outcome, "configured",
            "button timing was not verified: {report:?}"
        );
        let values = serde_json::to_value(report.settings.unwrap()).unwrap();
        for field in ["short", "double", "repeat", "stuck"] {
            assert_eq!(
                values["button_timers"][field], timers[field],
                "actual button timing was lost"
            );
        }
        assert_eq!(values["button_timers"]["short_min"], 25);
        assert_eq!(values["button_timers"]["double_min"], 10);
        assert_eq!(values["button_timers"]["quality"], "complete");
        assert_eq!(values["enabled"], true);
        assert_eq!(values["event_filter"], 255);
        handle
            .read_input(&target("Button"), context(1), deadline())
            .await
            .unwrap();
        assert_eq!(projected(&handle, "Button")["InputValueRaw"], "01");
    }
    assert!(
        events.try_recv().is_err(),
        "timing configuration generated an input action"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_button_timing_minimum_preflight_prevents_mixed_partial_programming() {
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
    let patch: Patch =
        serde_json::from_value(json!({"enabled":false,"button_timers":{"short":24,"double":0}}))
            .unwrap();
    let report = handle
        .configure_input(&target("Button"), 1, patch, context(1), deadline())
        .await
        .unwrap();
    assert_ne!(report.outcome, "configured");
    let snapshot = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        snapshot["buses"][0]["inputs"][0]["settings"][0]["enabled"], true,
        "invalid timer minimum allowed a common setting write"
    );
    assert_eq!(
        snapshot["buses"][0]["inputs"][0]["settings"][0]["button_timers"]["short"],
        25
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
