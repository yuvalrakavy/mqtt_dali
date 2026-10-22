use super::*;
use crate::service::scene_settings::Action;
use crate::service::scenes::settings_value::Comparison;

#[tokio::test]
async fn scene_settings_capture_rgbwaf_uses_assigned_native_channels_not_selected_component() {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let mut config = fixture.disk();
    config.buses[0].outputs[0].color_representation =
        Some(crate::config::ColorRepresentation::Temperature);
    fs::write(&fixture.paths.current, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut profile: serde_json::Value =
        serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json")).unwrap();
    profile["buses"][0]["outputs"][0]["short_address"] = json!(0);
    profile["buses"][0]["outputs"][0]["colour"]["active"] = json!("rgbwaf");
    profile["buses"][0]["outputs"][0]["colour"]["rgbwaf"] = json!([10, 20, 30]);
    let emulator = pty::spawn(serde_json::from_value(profile).unwrap())
        .await
        .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let capture = handle
        .change_scene_row(
            OUTPUT,
            Action::Capture {
                slot: crate::dali::scenes::Slot::new(0).unwrap(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(capture.candidate).unwrap(),
        json!({"level":{"kind":"value","value":100},
        "color":{"kind":"rgbwaf","channels":[10,20,30,null,null,null],"control":128}}),
        "capture guessed component representation or required unassigned channels"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(
        handle.snapshot().unwrap().outputs.is_empty(),
        "capture manufactured cached output State"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn scene_settings_final_stamp_fences_both_probe_and_post_write_acceptance() {
    use std::sync::atomic::AtomicUsize;
    for trigger in [1, 2] {
        let fixture = Fixture::new();
        save_scene(&fixture);
        let peer = super::peer::Peer::new(0);
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let calls = Arc::new(AtomicUsize::new(0));
        service.output_acceptance_hook = Some(Arc::new(move |hat| {
            if calls.fetch_add(1, Ordering::SeqCst) + 1 == trigger {
                hat.power_cycle_output_samples_for_test();
            }
        }));
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .change_scene_row(
                SCENE,
                Action::Set {
                    fixture: OUTPUT.into(),
                    setting: serde_json::from_value(base_setting(80)).unwrap(),
                    expected_revision: 1,
                },
                context(1),
                deadline(),
            )
            .await;
        if trigger == 1 {
            assert!(
                result.is_err(),
                "stale scene probe authorized a private save"
            );
            assert_eq!(fixture.disk().revision, 1);
            assert!(peer.settings().is_empty());
        } else {
            let report = result.unwrap();
            assert_eq!(
                report.outcome, "unconfirmed",
                "stale post-write scene evidence claimed verified"
            );
            assert_eq!(report.revision, 2);
            assert!(report.stored.unwrap().stale);
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_settings_repair_rechecks_review_immediately_before_native_programming() {
    use std::sync::atomic::AtomicBool;
    let fixture = Fixture::new();
    save_scene(&fixture);
    let peer = super::peer::Peer::new(0);
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let armed = Arc::new(AtomicBool::new(false));
    let gate = armed.clone();
    let physical = peer.level.clone();
    service.output_acceptance_hook = Some(Arc::new(move |_| {
        if gate.swap(false, Ordering::SeqCst) {
            physical.store(88, Ordering::SeqCst);
        }
    }));
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let reviewed = handle
        .read_scene(
            OUTPUT,
            crate::dali::scenes::Slot::new(0).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap()
        .stored
        .unwrap();
    handle
        .change_scene_row(
            SCENE,
            Action::Adopt {
                fixture: OUTPUT.into(),
                reviewed: reviewed.clone(),
                expected_revision: 1,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    armed.store(true, Ordering::SeqCst);
    let report = handle
        .change_scene_row(
            SCENE,
            Action::Repair {
                fixture: OUTPUT.into(),
                reviewed,
                expected_revision: 2,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        report.outcome, "unconfirmed",
        "repair ignored native drift after probe handoff"
    );
    assert_eq!(peer.level.load(Ordering::SeqCst), 88);
    assert!(
        peer.settings().is_empty(),
        "repair overwrote changed review during continuation"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn scene_settings_native_color_masks_repeat_profiles_preserve_actual_and_other_rows() {
    for auto in [false, true] {
        let fixture = Fixture::new();
        fixture.add_output(1, "Other");
        save_scene(&fixture);
        let mut profile: serde_json::Value =
            serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json"))
                .unwrap();
        profile["repeat"] = json!(if auto {
            "two_terminals"
        } else {
            "one_terminal"
        });
        profile["buses"][0]["outputs"][0]["short_address"] = json!(0);
        profile["buses"][0]["outputs"][0]["colour"]["auto_activation"] = json!(auto);
        profile["buses"][0]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"other","short_address":1,"level":80}));
        let emulator = pty::spawn(serde_json::from_value(profile).unwrap())
            .await
            .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &emulator.path,
                if auto {
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
        let mut revision = 1;
        let cases = [
            json!({"level":{"kind":"mask"},"color":{"kind":"temperature","value":350}}),
            json!({"level":{"kind":"value","value":0},"color":{"kind":"xy","x":0,"y":65534}}),
            json!({"level":{"kind":"value","value":254},"color":{"kind":"rgbwaf","channels":[1,2,3,4,5,6],"control":128}}),
            json!({"level":{"kind":"mask"},"color":{"kind":"temperature","value":null}}),
            json!({"level":{"kind":"mask"},"color":{"kind":"xy","x":null,"y":null}}),
            json!({"level":{"kind":"mask"},"color":{"kind":"rgbwaf","channels":[null,null,null,null,null,null],"control":null}}),
            base_setting(80),
            json!({"level":{"kind":"mask"},"color":{"kind":"mask"}}),
        ];
        for (index, desired) in cases.into_iter().enumerate() {
            let result = handle
                .change_scene_row(
                    SCENE,
                    Action::Set {
                        fixture: OUTPUT.into(),
                        setting: serde_json::from_value(desired.clone()).unwrap(),
                        expected_revision: revision,
                    },
                    context(1),
                    Instant::now() + Duration::from_secs(18),
                )
                .await
                .unwrap();
            assert_eq!(
                result.outcome, "verified",
                "native scene family failed: {desired}: {result:?}"
            );
            revision += 1;
            assert_eq!(result.revision, revision);
            assert_eq!(serde_json::to_value(result.saved).unwrap(), desired);
            let observed = handle
                .read_scene(
                    OUTPUT,
                    crate::dali::scenes::Slot::new(0).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap();
            assert_eq!(observed.comparison, Some(Comparison::Matching));
            let physical = emulator
                .emulator
                .control(Control::Snapshot, Duration::from_secs(1))
                .await
                .unwrap();
            let gear = &physical["buses"][0]["outputs"][0];
            assert_eq!(gear["level"], 100, "scene editing activated brightness");
            assert_eq!(gear["colour_state"]["actual"]["active"], "temperature");
            assert_eq!(
                gear["colour_state"]["actual"]["temperature"]["value"], 250,
                "scene editing activated colour"
            );
            assert_eq!(gear["scenes"][1], 255, "scene editing changed another slot");
            assert_eq!(
                physical["buses"][0]["outputs"][1]["scenes"][0], 255,
                "scene editing changed another fixture"
            );
            if index == 0 {
                let capture = handle
                    .change_scene_row(
                        OUTPUT,
                        Action::Capture {
                            slot: crate::dali::scenes::Slot::new(0).unwrap(),
                        },
                        context(1),
                        Instant::now() + Duration::from_secs(8),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(capture.candidate).unwrap(),
                    json!({"level":{"kind":"value","value":100},
                    "color":{"kind":"temperature","value":250}}),
                    "capture used stored colour instead of active family"
                );
                assert_eq!(fixture.disk().revision, revision);
            }
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn scene_settings_save_failure_uncertainty_and_unverified_repeat_never_program() {
    for fault in [0, 1, 2] {
        let fixture = Fixture::new();
        save_scene(&fixture);
        let emulator = pty::spawn(
            serde_json::from_value(json!({"version":1,"buses":[{"number":0,
            "outputs":[{"id":"fixture","short_address":0,"level":100}]}]}))
            .unwrap(),
        )
        .await
        .unwrap();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        if fault < 2 {
            let once = Arc::new(AtomicBool::new(false));
            service.hooks = Hooks {
                callback: Some(Arc::new(move |step| {
                    let at = if fault == 0 {
                        Step::BeforeRename(Stage::Reservation)
                    } else {
                        Step::DirectorySync(Stage::Mapping)
                    };
                    if step == at && !once.swap(true, Ordering::SeqCst) {
                        Err("injected scene save failure".into())
                    } else {
                        Ok(())
                    }
                })),
            };
        }
        let running = start(
            service.with_uart(
                &emulator.path,
                if fault == 2 {
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
            .change_scene_row(
                SCENE,
                Action::Set {
                    fixture: OUTPUT.into(),
                    setting: serde_json::from_value(base_setting(42)).unwrap(),
                    expected_revision: 1,
                },
                context(1),
                deadline(),
            )
            .await;
        if fault == 1 {
            assert!(
                matches!(result, Err(ServiceError::Uncertain(_))),
                "uncertain private save was accepted: {result:?}"
            );
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
            assert_eq!(fixture.disk().buses[0].scenes[0].contents.len(), 1);
        } else {
            assert!(
                matches!(result, Err(ServiceError::Rejected(_))),
                "unconfirmed save/policy was accepted: {result:?}"
            );
            assert_eq!(fixture.disk().revision, 1);
            assert!(fixture.disk().buses[0].scenes[0].contents.is_empty());
        }
        let physical = emulator
            .emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            physical["buses"][0]["outputs"][0]["scenes"][0], 255,
            "scene programming preceded confirmed durable save"
        );
        assert_eq!(physical["buses"][0]["outputs"][0]["level"], 100);
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn scene_settings_held_save_retains_permit_and_original_physical_stamp() {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let emulator = pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"fixture","short_address":0,"level":100}]}]}))
        .unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::BeforeRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let h = handle.clone();
    let pending = tokio::spawn(async move {
        h.change_scene_row(
            SCENE,
            Action::Set {
                fixture: OUTPUT.into(),
                setting: serde_json::from_value(base_setting(42)).unwrap(),
                expected_revision: 1,
            },
            context(1),
            Instant::now() + Duration::from_secs(8),
        )
        .await
    });
    hold.entered().await;
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["scenes"][0], 255);
    assert!(
        matches!(
            handle.read_output(OUTPUT, context(1), deadline()).await,
            Err(ServiceError::Busy)
        ),
        "save handoff released scene permit"
    );
    for powered in [false, true] {
        emulator
            .emulator
            .control(
                Control::BusPower { bus: 0, powered },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
    }
    hold.release.store(true, Ordering::SeqCst);
    let report = pending.await.unwrap().unwrap();
    assert_eq!(
        report.outcome, "unconfirmed",
        "scene continuation substituted new physical authority"
    );
    assert_eq!(report.revision, 2);
    assert!(report.stored.is_none());
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["scenes"][0], 255,
        "stale scene continuation programmed native settings"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
