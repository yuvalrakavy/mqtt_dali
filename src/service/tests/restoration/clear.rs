use super::*;

#[tokio::test]
async fn individual_clear_preserves_saved_identity_native_settings_and_other_fixtures() {
    for input in [false, true] {
        for two in [false, true] {
            let fixture = Fixture::new();
            fixture.add_button();
            let emulator = physical(input, 2, two, 1).await;
            for index in 0..2 {
                emulator
                    .emulator
                    .control(
                        Control::SetAddress {
                            id: format!("{}-{index}", if input { "input" } else { "light" }),
                            short_address: Some(index),
                        },
                        Duration::from_secs(1),
                    )
                    .await
                    .unwrap();
            }
            let before = native(&emulator.emulator).await;
            let saved_bytes = fs::read(&fixture.paths.current).unwrap();
            let broker = FakeBroker::start().await;
            let (service, handle) = fixture.service();
            let running = start(
                service.with_uart(
                    &emulator.path,
                    if input {
                        RepeatPolicy::Unverified
                    } else if two {
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
            let saved = handle.snapshot().unwrap().model;
            let report = handle
                .clear_short_address(target(input), 1, context(1), limit())
                .await
                .unwrap();
            assert_eq!(
                report.outcome,
                ResultKind::Cleared,
                "individual address clearing did not complete"
            );
            assert_eq!(report.operation, "clear_short_address");
            assert_eq!(
                (report.revision, report.address, report.previous_address),
                (1, 0, 0)
            );
            assert!(!report.configuration_changed);
            assert_eq!(report.current_generation, report.target_generation);
            assert_eq!(
                saved_bytes,
                fs::read(&fixture.paths.current).unwrap(),
                "individual clear rewrote saved configuration"
            );
            let after = handle.snapshot().unwrap().model;
            assert_eq!(
                saved.state_bytes().unwrap(),
                after.state_bytes().unwrap(),
                "clear changed saved child identities or private intent"
            );
            assert_eq!(
                saved.origin(location(input, 0)),
                after.origin(location(input, 0)),
                "clear retired saved identity"
            );
            assert_eq!(
                saved.inventory(),
                after.inventory(),
                "clear changed saved names, children or binding intent"
            );
            let space = if input { "inputs" } else { "outputs" };
            let mut expected = before;
            expected["buses"][0][space][0]["short_address"] = serde_json::Value::Null;
            let mut actual = native(&emulator.emulator).await;
            // DTR0 is a broadcast scratch register; clocks also advance normally.
            for snapshot in [&mut actual, &mut expected] {
                for device in snapshot["buses"][0][space].as_array_mut().unwrap() {
                    device.as_object_mut().unwrap().remove("dtr");
                    device.as_object_mut().unwrap().remove("dtr0");
                }
            }
            assert_eq!(
                actual["buses"], expected["buses"],
                "individual clear changed other native settings or fixtures"
            );
            assert!(running.stop().await);
            emulator.emulator.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn individual_clear_refuses_unknown_profile_retired_authority_and_stale_final_evidence() {
    for fault in ["profile", "revision", "generation", "final", "absent"] {
        let fixture = Fixture::new();
        let emulator = physical(false, 1, false, 1).await;
        if fault != "absent" {
            emulator
                .emulator
                .control(
                    Control::SetAddress {
                        id: "light-0".into(),
                        short_address: Some(0),
                    },
                    Duration::from_secs(1),
                )
                .await
                .unwrap();
        }
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        if fault == "final" {
            service.output_acceptance_hook =
                Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        }
        let running = start(
            service.with_uart(
                &emulator.path,
                if fault == "profile" {
                    RepeatPolicy::Unverified
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .clear_short_address(
                target(false),
                if fault == "revision" { 2 } else { 1 },
                context(if fault == "generation" { 2 } else { 1 }),
                limit(),
            )
            .await;
        if matches!(fault, "final" | "absent") {
            assert_eq!(
                result.unwrap().outcome,
                ResultKind::Unconfirmed,
                "individual clear claimed stale or missing evidence"
            );
        } else {
            assert!(
                result.is_err(),
                "individual clear admitted invalid authority or repeat profile"
            );
        }
        assert_eq!(
            native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
            if matches!(fault, "final" | "absent") {
                serde_json::Value::Null
            } else {
                json!(0)
            }
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn individual_clear_mqtt_is_strict_and_completes_with_the_original_authority() {
    let fixture = Fixture::new();
    fixture.add_button();
    let emulator = physical(false, 1, false, 1).await;
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "light-0".into(),
                short_address: Some(0),
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
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
    for (index, invalid) in [json!({"command":"clear_short_address","expected_revision":1}),json!({"command":"clear_short_address","target":"Bus1.Groups.All","expected_revision":1}),json!({"command":"clear_short_address","target":"Bus1.Inputs.Panel.Instances.Button","expected_revision":1}),json!({"command":"clear_short_address","target":target(false),"expected_revision":1,"address":0})].into_iter().enumerate() {
        assert!(super::super::groups::call(&broker,invalid,1,&format!("test/clear-invalid/{index}")).await.get("error").is_some(),"individual clear accepted invalid parameters");
    }
    let reply = super::super::groups::call(
        &broker,
        json!({"command":"clear_short_address","target":target(false),"expected_revision":1}),
        1,
        "test/clear-ok",
    )
    .await;
    assert!(
        reply.get("error").is_none(),
        "individual clear MQTT failed: {reply}"
    );
    assert_eq!(reply["value"]["outcome"], "cleared");
    assert_eq!(
        reply["value"]["target_generation"],
        context(1).generation().unwrap()
    );
    assert_eq!(reply["value"]["revision"], 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
