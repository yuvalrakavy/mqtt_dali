use super::*;

async fn relay(two: bool, adjustable: bool) -> pty::PtyEmulator {
    pty::spawn(
        serde_json::from_value(json!({"version":1,
            "repeat":if two {"two_terminals"}else{"one_terminal"},
            "buses":[{"number":0,"outputs":[{"id":"relay","short_address":0,
            "device_type":7,"level":254,"switching":{"adjustable_thresholds":adjustable}}]}]
        }))
        .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn managed_dt7_settings_native_thresholds_and_virtual_limits_keep_actual_contact() {
    for two in [false, true] {
        let fixture = Fixture::new();
        let original = fs::read(&fixture.paths.current).unwrap();
        let emulator = relay(two, true).await;
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &emulator.path,
                if two {
                    RepeatPolicy::TwoTerminals
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let patch = serde_json::from_value::<Patch>(
            json!({"switching":{"upward_on":50,"upward_off":100,"downward_on":150,"downward_off":200}}),
        );
        assert!(
            patch.is_ok(),
            "managed DT7 settings rejects the approved nested relay patch: {patch:?}"
        );
        let result = handle
            .configure_output(TARGET, 1, patch.unwrap(), context(1), limit())
            .await
            .unwrap();
        assert_eq!(
            result.outcome, "configured",
            "managed DT7 thresholds were not confirmed: {result:?}"
        );
        let state = physical(&emulator.emulator).await;
        let gear = &state["buses"][0]["outputs"][0];
        assert_eq!(
            gear["switching_state"]["thresholds"],
            json!([50, 100, 150, 200]),
            "managed DT7 threshold values did not reach native gear"
        );
        assert_eq!(
            gear["level"], 254,
            "programming thresholds operated the contact"
        );
        assert_eq!(gear["switching_state"]["virtual_level"], 254);
        let result = handle
            .configure_output(
                TARGET,
                1,
                Patch {
                    minimum_level: Some(1),
                    ..Default::default()
                },
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(
            result.outcome, "configured",
            "adjustable DT7 virtual minimum was incorrectly constrained by physical254: {result:?}"
        );
        let result = handle
            .configure_output(
                TARGET,
                1,
                serde_json::from_value(json!({"switching":{"upward_on":255,"downward_off":0}}))
                    .unwrap(),
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(result.outcome, "configured");
        let state = physical(&emulator.emulator).await;
        let gear = &state["buses"][0]["outputs"][0];
        assert_eq!(
            gear["switching_state"]["thresholds"],
            json!([255, 100, 150, 0]),
            "managed DT7 disabled255/downward0 lost native meaning"
        );
        assert_eq!(gear["min_level"], 1);
        assert_eq!(gear["level"], 254);
        let report = serde_json::to_value(
            handle
                .read_output_settings(TARGET, context(1), limit())
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["settings"]["switching_support"], true);
        assert_eq!(
            report["settings"]["switching"]["adjustable_thresholds"],
            true
        );
        assert_eq!(report["settings"]["switching"]["upward_on"], 255);
        assert_eq!(report["settings"]["quality"], "complete");
        let second = handle
            .configure_output(
                TARGET,
                1,
                Patch {
                    minimum_level: Some(2),
                    maximum_level: Some(200),
                    ..Default::default()
                },
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(
            second.outcome, "configured",
            "a second DT7 limit save rejected existing virtual limits: {second:?}"
        );
        assert_eq!(
            (
                second.settings.as_ref().unwrap().minimum_level,
                second.settings.as_ref().unwrap().maximum_level
            ),
            (Some(2), Some(200))
        );
        let actual = handle
            .read_output(TARGET, context(1), limit())
            .await
            .unwrap();
        assert_eq!(
            (actual.brightness(), actual.power()),
            (Some(1.0), Some(true))
        );
        assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn managed_dt7_settings_virtual_limit_has_real_native_floor_override() {
    let fixture = Fixture::new();
    let emulator = relay(false, true).await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .configure_output(
            TARGET,
            1,
            Patch {
                minimum_level: Some(1),
                ..Default::default()
            },
            context(1),
            limit(),
        )
        .await
        .unwrap();
    assert_eq!(
        result.outcome, "configured",
        "adjustable DT7 virtual minimum was incorrectly constrained by physical254: {result:?}"
    );
    let state = physical(&emulator.emulator).await;
    assert_eq!(state["buses"][0]["outputs"][0]["min_level"], 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_dt7_settings_mqtt_capabilities_and_threshold_writes_are_generic_and_truthful() {
    for adjustable in [false, true] {
        let fixture = Fixture::new();
        let original = fs::read(&fixture.paths.current).unwrap();
        let emulator = relay(false, adjustable).await;
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
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
        let trace = "00-11111111111111111111111111111111-2222222222222222-01";
        for(index,call)in[json!({"command":"read_output_settings","target":TARGET}),json!({"command":"configure_output","target":TARGET,"expected_revision":1,"settings":{"switching":{"upward_on":255,"downward_off":0}}})].into_iter().enumerate(){
            let topic=format!("test/dt7-settings/{index}");
            assert!(broker.publish("DALI/Command/Controller",call.to_string(),&SendOptions::default().response_topic(&topic).user_property("target-generation",context(1).generation().unwrap()).user_property("traceparent",trace)));
            assert!(broker.wait_until(Duration::from_secs(3),|b|!b.received_on(&topic).is_empty()).await);
            let replies=broker.received_on(&topic);assert_eq!(replies.len(),1);
            let reply:serde_json::Value=serde_json::from_slice(&replies[0].payload).unwrap();
            assert!(reply.get("error").is_none(),"generic DT7 MQTT settings dispatch failed: {reply}");
            let result=&reply["value"];
            assert_eq!(result["outcome"],if index==0{"current"}else if adjustable{"configured"}else{"rejected"},"DT7 MQTT capability/result mismatch: {reply}");
            assert_eq!(result["settings"]["switching_support"],true);assert_eq!(result["settings"]["switching"]["adjustable_thresholds"],adjustable);
            assert_eq!(result["target_generation"],context(1).generation().unwrap());assert_eq!(replies[0].user_property("traceparent"),Some(trace));
        }
        let state = physical(&emulator.emulator).await;
        let gear = &state["buses"][0]["outputs"][0];
        assert_eq!(
            gear["switching_state"]["thresholds"],
            if adjustable {
                json!([255, 255, 255, 0])
            } else {
                json!([1, 255, 255, 0])
            }
        );
        assert_eq!(gear["level"], 254);
        assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}
