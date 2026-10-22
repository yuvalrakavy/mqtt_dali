use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
    service::{discovery::Space, reassignment::ResetOutcome},
};
mod literal;

fn limit() -> Instant {
    Instant::now() + Duration::from_secs(8)
}
async fn physical(emulator: &crate::emulator::session::Emulator) -> serde_json::Value {
    emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()
}
async fn bus(two: bool) -> pty::PtyEmulator {
    pty::spawn(serde_json::from_value(json!({"version":1,"repeat":if two{"two_terminals"}else{"one_terminal"},"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":99,"groups":5},{"id":"used","short_address":0,"level":42}],
        "inputs":[{"id":"panel","short_address":0,"instances":[1],"values":[{"instance":0,"resolution":1,"bytes":[255]}]}]}]})).unwrap()).await.unwrap()
}

#[tokio::test]
async fn reassignment_retires_selected_rows_prunes_refs_and_reuses_names_with_fresh_tokens() {
    for two in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let mut disk = serde_json::to_value(fixture.disk()).unwrap();
        disk["buses"][0]["scenes"] = json!([{"slot":0,"name":"Evening","contents":[{"output_origin":{"target":"Bus1.Outputs.Light1","revision":1},"setting":{"level":{"kind":"value","value":42},"color":{"kind":"mask"}}}]}]);
        fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
        let emulator = bus(two).await;
        let before = physical(&emulator.emulator).await;
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
        let old = handle.snapshot().unwrap().model;
        let report = handle
            .reset_address_space(0, Space::Outputs, 1, context(1), limit())
            .await
            .unwrap();
        assert_eq!(report.outcome, ResetOutcome::Cleared);
        assert_eq!(report.revision, 2);
        assert_eq!(report.checked_addresses, 64);
        assert_eq!(report.retired_devices, 1);
        let disk = fixture.disk();
        assert!(disk.buses[0].outputs.is_empty());
        assert!(disk.buses[0].groups[0].members.is_empty());
        assert!(disk.buses[0].scenes[0].contents.is_empty());
        let now = handle.snapshot().unwrap().model;
        for location in [
            Location::Input {
                bus: 0,
                short_address: 0,
            },
            Location::InputInstance {
                bus: 0,
                short_address: 0,
                instance: 0,
            },
        ] {
            assert_eq!(old.origin(location), now.origin(location));
        }
        let after = physical(&emulator.emulator).await;
        assert!(after["buses"][0]["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["short_address"].is_null()));
        assert_eq!(after["buses"][0]["inputs"], before["buses"][0]["inputs"]);
        assert_eq!(after["buses"][0]["outputs"][0]["level"], 99);
        assert_eq!(after["buses"][0]["outputs"][0]["groups"], 5);
        let assigned = handle
            .assign_next_output(0, 2, context(2), limit())
            .await
            .unwrap();
        assert_eq!(
            assigned.outcome,
            crate::service::commissioning::AssignmentOutcome::Assigned
        );
        assert_eq!(assigned.target.as_deref(), Some("Bus1.Outputs.Light1"));
        assert_ne!(
            old.origin(Location::Output {
                bus: 0,
                short_address: 0
            }),
            handle.snapshot().unwrap().model.origin(Location::Output {
                bus: 0,
                short_address: 0
            })
        );
        assert!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await
                .is_err(),
            "retired token controlled reused default name"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn reassignment_input_retirement_keeps_output_identity_and_unverified_output_profile_is_allowed(
) {
    let fixture = Fixture::new();
    fixture.add_button();
    let emulator = bus(false).await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let before = handle.snapshot().unwrap().model;
    let report = handle
        .reset_address_space(0, Space::Inputs, 1, context(1), limit())
        .await
        .unwrap();
    assert_eq!(
        report.outcome,
        ResetOutcome::Cleared,
        "input clear was not verified"
    );
    assert_eq!(report.retired_instances, 1);
    assert_eq!(report.retired_devices, 1);
    let now = handle.snapshot().unwrap().model;
    assert!(fixture.disk().buses[0].inputs.is_empty());
    assert_eq!(
        before.origin(Location::Output {
            bus: 0,
            short_address: 0
        }),
        now.origin(Location::Output {
            bus: 0,
            short_address: 0
        })
    );
    let native = physical(&emulator.emulator).await;
    assert!(native["buses"][0]["inputs"][0]["short_address"].is_null());
    assert_eq!(native["buses"][0]["outputs"][0]["short_address"], 0);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn reassignment_noop_saved_space_still_clears_unsaved_physical_gear() {
    let fixture = Fixture::new();
    let mut disk = fixture.disk();
    disk.buses[0].outputs.clear();
    disk.buses[0].groups[0].members.clear();
    fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
    let emulator = bus(false).await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let report = handle
        .reset_address_space(0, Space::Outputs, 1, context(1), limit())
        .await
        .unwrap();
    assert_eq!(report.outcome, ResetOutcome::Cleared);
    assert_eq!(report.revision, 1);
    assert_eq!(report.retired_devices, 0);
    assert!(
        physical(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"].is_null()
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn reassignment_save_failure_or_uncertainty_never_clears_and_held_save_keeps_permit() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let emulator = bus(false).await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected reassignment save failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .reset_address_space(0, Space::Outputs, 1, context(1), limit())
            .await;
        assert!(
            result.is_err(),
            "unconfirmed reassignment save was accepted"
        );
        assert_eq!(
            physical(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
            0
        );
        if at == Step::DirectorySync(Stage::Mapping) {
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
    let fixture = Fixture::new();
    let emulator = bus(false).await;
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
        h.reset_address_space(0, Space::Outputs, 1, context(1), limit())
            .await
    });
    hold.entered().await;
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Writing);
    assert_eq!(
        physical(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
        0
    );
    assert!(matches!(
        handle
            .read_output("Bus1.Outputs.Light1", context(1), deadline())
            .await,
        Err(ServiceError::Busy)
    ));
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
    tokio::time::sleep(Duration::from_millis(20)).await;
    hold.release.store(true, Ordering::SeqCst);
    let report = pending.await.unwrap().unwrap();
    assert_eq!(report.outcome, ResetOutcome::RetiredButUnconfirmed);
    assert_eq!(report.revision, 2);
    assert_eq!(
        physical(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
        0,
        "old physical stamp was replaced after save"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn reassignment_final_stamp_and_unknown_profile_are_not_success_or_early_retirement() {
    for final_cycle in [false, true] {
        let fixture = Fixture::new();
        let emulator = bus(false).await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        if final_cycle {
            service.output_acceptance_hook =
                Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        }
        let running = start(
            service.with_uart(
                &emulator.path,
                if final_cycle {
                    RepeatPolicy::OneTerminal
                } else {
                    RepeatPolicy::Unverified
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .reset_address_space(0, Space::Outputs, 1, context(1), limit())
            .await;
        if final_cycle {
            let report = result.unwrap();
            assert_eq!(
                report.outcome,
                ResetOutcome::RetiredButUnconfirmed,
                "stale final address-clear evidence claimed success"
            );
            assert_eq!(report.checked_addresses, 64);
            assert_eq!(fixture.disk().revision, 2);
        } else {
            assert!(
                result.is_err(),
                "unverified output profile retired saved identities"
            );
            assert_eq!(fixture.disk().revision, 1);
            assert_eq!(
                physical(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
                0
            );
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn reassignment_mqtt_strict_controller_scope_and_original_completion() {
    let fixture = Fixture::new();
    let emulator = bus(false).await;
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
    for (index,body) in [json!({"command":"reset_address_space","bus":0,"space":"both","expected_revision":1}),json!({"command":"reset_address_space","bus":0,"space":{"outputs":null},"expected_revision":1}),json!({"command":"reset_address_space","target":"Bus1.Outputs.Light1","bus":0,"space":"outputs","expected_revision":1}),json!({"command":"reset_address_space","bus":0,"space":"outputs","expected_revision":1,"extra":true})].into_iter().enumerate(){
        let reply=super::groups::call(&broker,body,1,&format!("test/reset-invalid/{index}")).await;assert!(reply.get("error").is_some());assert_eq!(fixture.disk().revision,1);
    }
    let reply = super::groups::call(
        &broker,
        json!({"command":"reset_address_space","bus":0,"space":"outputs","expected_revision":1}),
        1,
        "test/reset-ok",
    )
    .await;
    assert_eq!(
        reply["value"]["outcome"], "cleared",
        "original command lost reply after retirement: {reply}"
    );
    assert_eq!(reply["value"]["revision"], 2);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
