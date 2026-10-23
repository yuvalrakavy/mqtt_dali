use super::settings_peer::{script, SettingsPeer};
use super::*;
use crate::service::input::settings::Patch;

fn sensor_script(mode: &str, after: bool) -> Vec<(String, String)> {
    let mut steps = script(
        3,
        3,
        if after && mode == "partial_common" {
            "N\n"
        } else {
            "JFF\n"
        },
    );
    for (command, reply) in &mut steps {
        if command == "l01032D\n" {
            if mode.starts_with("unsupported") {
                *reply = "JFF\n".into();
            }
            if matches!(mode, "missing_read" | "missing_hold" | "partial_common") {
                *reply = "N\n".into();
            }
            if mode == "collision_read" {
                *reply = "X2\n".into();
            }
        }
        if command == "l01032E\n" && mode == "unsupported_read" {
            *reply = "JFF\n".into();
        }
        if command == "l01032C\n" && mode == "unsupported_read" {
            *reply = "JFF\n".into();
        }
        if command == "l01032E\n" && after && mode == "unsupported_report" {
            *reply = "J32\n".into();
        }
    }
    steps
}
#[tokio::test]
async fn managed_sensor_settings_literal_optional_hold_missing_evidence_and_actual_match() {
    for mode in [
        "unsupported_read",
        "missing_read",
        "collision_read",
        "unsupported_report",
        "unsupported_write",
        "missing_hold",
        "partial_common",
        "ignored",
    ] {
        let (fixture, _) = configured();
        let mut steps = sensor_script(mode, false);
        let before = steps.iter().map(|s| s.0.clone()).collect::<Vec<_>>();
        let reading = mode.ends_with("_read");
        if matches!(mode, "unsupported_report" | "ignored") {
            steps.extend([
                ("lC13032\n".into(), "N\n".into()),
                ("l010322\n".into(), "N\n".into()),
                ("l010322\n".into(), "N\n".into()),
            ]);
            steps.extend(sensor_script(mode, true));
        }
        if mode == "partial_common" {
            steps.extend([
                ("l010363\n".into(), "N\n".into()),
                ("l010363\n".into(), "N\n".into()),
            ]);
            steps.extend(sensor_script(mode, true));
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
        let result = if reading {
            handle
                .read_input_settings(&target("Occupancy"), context(1), deadline())
                .await
        } else {
            let patch = if mode == "partial_common" {
                json!({"enabled":false})
            } else if matches!(mode, "unsupported_write" | "missing_hold") {
                json!({"enabled":false,"sensor_settings":{"report":50,"hold":0}})
            } else {
                json!({"sensor_settings":{"report":50}})
            };
            handle
                .configure_input(
                    &target("Occupancy"),
                    1,
                    serde_json::from_value(patch).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
        };
        if matches!(mode, "unsupported_write" | "missing_hold") {
            let sent = peer
                .commands
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.starts_with('l'))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(sent,before,"sensor preflight sent a setting before checking support/evidence for the whole patch");
        }
        let r = result.unwrap();
        let expected = if reading {
            "current"
        } else if matches!(mode, "partial_common" | "unsupported_report") {
            "configured"
        } else {
            "unconfirmed"
        };
        assert_eq!(
            r.outcome, expected,
            "sensor configuration lacked complete matching supported evidence: {mode}"
        );
        if reading || mode == "partial_common" || mode == "unsupported_report" {
            let v = serde_json::to_value(r.settings.unwrap()).unwrap();
            let sensor = &v["sensor_settings"];
            if mode.starts_with("unsupported") {
                assert_eq!(sensor["hold"], 255, "unsupported hold MASK was lost");
                assert_eq!(sensor["hold_supported"], false);
                assert_eq!(
                    sensor["quality"], "complete",
                    "unsupported hold blocked other sensor settings"
                );
                if reading {
                    assert_eq!(sensor["report"], 255);
                    assert_eq!(sensor["deadtime"], 255);
                }
            } else {
                assert!(
                    sensor["hold"].is_null(),
                    "missing sensor evidence became zero or unsupported"
                );
                assert!(sensor["hold_supported"].is_null());
                assert_eq!(sensor["quality"], "partial");
            }
        }
        assert!(running.stop().await);
    }
}
#[tokio::test]
async fn managed_sensor_settings_strict_family_ranges_and_mqtt_roundtrip() {
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
    for (name, value) in [
        ("Button", json!({"report":1})),
        ("Contact", json!({"hold":0})),
        ("Occupancy", json!({"hold":255})),
        ("Light", json!({"hold":0})),
        ("Light", json!({"hysteresis":26})),
        ("Contact", json!({})),
        ("Contact", json!(null)),
        ("Contact", json!({"report":null})),
        ("Contact", json!({"report":256})),
        ("Contact", json!({"report":1.5})),
        ("Light", json!({"unknown":0})),
    ] {
        if let Ok(patch) = serde_json::from_value::<Patch>(json!({"sensor_settings":value})) {
            assert!(
                matches!(
                    handle
                        .configure_input(&target(name), 1, patch, context(1), deadline())
                        .await,
                    Err(ServiceError::Rejected(_))
                ),
                "invalid sensor patch reached hardware"
            );
        }
    }
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (index, settings, success) in [
        (0, json!({"hysteresis":25,"hysteresis_min":255}), true),
        (1, json!({"hold":0}), false),
    ] {
        let reply = format!("test/sensor-settings/{index}");
        let call = json!({"command":"configure_input","target":target("Light"),"expected_revision":1,"settings":{"sensor_settings":settings}});
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
        let r: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(r.get("error").is_none(), success);
        if success {
            assert_eq!(r["value"]["outcome"], "configured");
            assert_eq!(
                r["value"]["settings"]["sensor_settings"]["hysteresis_min"],
                255
            );
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
async fn managed_sensor_settings_all_families_preserve_identity_and_input_values() {
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
    for (name, settings, raw) in [
        ("Contact", json!({"report":255,"deadtime":255}), "00"),
        (
            "Occupancy",
            json!({"report":255,"deadtime":255,"hold":0}),
            "02",
        ),
        (
            "Light",
            json!({"report":255,"deadtime":255,"hysteresis":25,"hysteresis_min":255}),
            "1234",
        ),
    ] {
        let patch: Patch = serde_json::from_value(json!({"sensor_settings":settings}))
            .expect("sensor settings patch is unsupported");
        let report = handle
            .configure_input(&target(name), 1, patch, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(
            report.outcome, "configured",
            "sensor settings were not verified: {report:?}"
        );
        let values = serde_json::to_value(report.settings.unwrap()).unwrap();
        for (field, value) in settings.as_object().unwrap() {
            assert_eq!(
                values["sensor_settings"][field], *value,
                "actual sensor setting was lost"
            );
        }
        assert_eq!(values["sensor_settings"]["quality"], "complete");
        if name == "Occupancy" {
            assert_eq!(values["sensor_settings"]["hold_supported"], true);
        }
        handle
            .read_input(&target(name), context(1), deadline())
            .await
            .unwrap();
        assert_eq!(projected(&handle, name)["InputValueRaw"], raw);
    }
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
