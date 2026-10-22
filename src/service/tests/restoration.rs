use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
    service::{
        commissioning::AssignmentOutcome,
        restoration::{Action, ResultKind},
    },
};
mod clear;
mod clear_literal;
mod literal;

fn limit() -> Instant {
    Instant::now() + Duration::from_secs(12)
}
fn unique(revision: u64) -> Action {
    Action::OnlyUnaddressed {
        expected_revision: revision,
    }
}
async fn native(emulator: &crate::emulator::session::Emulator) -> serde_json::Value {
    emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()
}
async fn physical(input: bool, count: usize, two: bool, kind: u8) -> pty::PtyEmulator {
    let fixtures:Vec<_>=(0..count).map(|i|if input{json!({"id":format!("input-{i}"),"short_address":null,"instances":[kind],"values":[{"instance":0,"resolution":1,"bytes":[255]}]})}else{json!({"id":format!("light-{i}"),"short_address":null,"level":100+i,"groups":4,"scenes":[42,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]})}).collect();
    pty::spawn(serde_json::from_value(json!({"version":1,"seed":9,"repeat":if two{"two_terminals"}else{"one_terminal"},"buses":[{"number":0,"outputs":if input{vec![]}else{fixtures.clone()},"inputs":if input{fixtures}else{vec![]}}]})).unwrap()).await.unwrap()
}
fn target(input: bool) -> &'static str {
    if input {
        "Bus1.Inputs.Panel"
    } else {
        "Bus1.Outputs.Light1"
    }
}
fn location(input: bool, address: u8) -> Location {
    if input {
        Location::Input {
            bus: 0,
            short_address: address,
        }
    } else {
        Location::Output {
            bus: 0,
            short_address: address,
        }
    }
}

#[tokio::test]
async fn restoration_unique_preserves_saved_identity_children_and_native_settings() {
    for input in [false, true] {
        for two in [false, true] {
            let fixture = Fixture::new();
            fixture.add_button();
            let emulator = physical(input, 1, two, 1).await;
            let before = native(&emulator.emulator).await;
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
                .restore_address(target(input), unique(1), context(1), limit())
                .await
                .unwrap();
            assert_eq!(
                report.outcome,
                ResultKind::Restored,
                "unique fixture was not restored: {report:?}"
            );
            assert_eq!(report.revision, 1);
            assert!(!report.configuration_changed);
            assert_eq!(fixture.disk().revision, 1);
            let after = handle.snapshot().unwrap().model;
            assert_eq!(
                saved.state_bytes().unwrap(),
                after.state_bytes().unwrap(),
                "restoration recreated or renamed the saved fixture"
            );
            if input {
                assert_eq!(
                    saved.origin(Location::InputInstance {
                        bus: 0,
                        short_address: 0,
                        instance: 0
                    }),
                    after.origin(Location::InputInstance {
                        bus: 0,
                        short_address: 0,
                        instance: 0
                    })
                );
            }
            let actual = native(&emulator.emulator).await;
            let space = if input { "inputs" } else { "outputs" };
            assert_eq!(actual["buses"][0][space][0]["short_address"], 0);
            for key in if input {
                vec!["instances", "values"]
            } else {
                vec!["level", "groups", "scenes"]
            } {
                assert_eq!(
                    actual["buses"][0][space][0][key],
                    before["buses"][0][space][0][key]
                );
            }
            assert!(running.stop().await);
            emulator.emulator.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn restoration_unique_refuses_none_multiple_occupied_and_input_type_mismatch() {
    for (input, count, kind, occupied, expected) in [
        (false, 0, 1, false, Some(ResultKind::NoUnaddressedFixture)),
        (
            false,
            2,
            1,
            false,
            Some(ResultKind::MultipleUnaddressedFixtures),
        ),
        (
            true,
            2,
            1,
            false,
            Some(ResultKind::MultipleUnaddressedFixtures),
        ),
        (true, 1, 2, false, None),
        (false, 1, 1, true, None),
    ] {
        let fixture = Fixture::new();
        fixture.add_button();
        let emulator = physical(input, count, false, kind).await;
        if occupied {
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
        let before = native(&emulator.emulator).await;
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
            .restore_address(target(input), unique(1), context(1), limit())
            .await
            .unwrap();
        assert_eq!(
            report.outcome,
            expected.unwrap_or(ResultKind::Unconfirmed),
            "ambiguous/incompatible restoration was accepted: {report:?}"
        );
        assert_eq!(fixture.disk().revision, 1);
        let after = native(&emulator.emulator).await;
        let space = if input { "inputs" } else { "outputs" };
        for (a, b) in after["buses"][0][space]
            .as_array()
            .unwrap()
            .iter()
            .zip(before["buses"][0][space].as_array().unwrap())
        {
            assert_eq!(
                a["short_address"], b["short_address"],
                "refused restore assigned an address"
            );
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn restoration_unique_works_when_all_64_saved_slots_are_reserved() {
    let fixture = Fixture::new();
    let mut disk = fixture.disk();
    for address in 1..64 {
        disk.buses[0].outputs.push(crate::config::Output {
            short_address: address,
            name: format!("Saved{address}"),
            description: None,
            color_representation: None,
        });
    }
    fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
    let emulator = physical(false, 1, false, 1).await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert_eq!(
        handle
            .assign_next_output(0, 1, context(1), limit())
            .await
            .unwrap()
            .outcome,
        AssignmentOutcome::NoFreeAddress
    );
    let report = handle
        .restore_address(target(false), unique(1), context(1), limit())
        .await
        .unwrap();
    assert_eq!(
        report.outcome,
        ResultKind::Restored,
        "full saved bus incorrectly required a free temporary address"
    );
    assert_eq!(fixture.disk().buses[0].outputs.len(), 64);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn restoration_identified_retires_only_candidate_and_preserves_original_origin() {
    for input in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let emulator = physical(input, 1, false, 1).await;
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let old = handle.snapshot().unwrap().model;
        let assigned = if input {
            handle
                .assign_next_input(0, 1, context(1), limit())
                .await
                .unwrap()
        } else {
            handle
                .assign_next_output(0, 1, context(1), limit())
                .await
                .unwrap()
        };
        assert_eq!(assigned.outcome, AssignmentOutcome::Assigned);
        assert_eq!(assigned.address, Some(1));
        let source = assigned.target.unwrap();
        let rev = if input {
            handle
                .discover_address(
                    0,
                    crate::service::discovery::Space::Inputs,
                    1,
                    assigned.revision,
                    context(assigned.revision),
                    limit(),
                )
                .await
                .unwrap()
                .revision
        } else {
            assigned.revision
        };
        let discovered = handle.snapshot().unwrap().model;
        if input {
            assert!(discovered
                .origin(Location::InputInstance {
                    bus: 0,
                    short_address: 1,
                    instance: 0
                })
                .is_some());
        }
        let source_generation =
            routing::generations(&handle.snapshot().unwrap().model).targets[&source].clone();
        let report = handle
            .restore_address(
                target(input),
                Action::Identified {
                    candidate: source.clone(),
                    candidate_generation: source_generation,
                    expected_revision: rev,
                },
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(
            report.outcome,
            ResultKind::Restored,
            "identified fixture did not move: {report:?}"
        );
        assert!(report.configuration_changed);
        assert_eq!(report.revision, rev + 1);
        let current = handle.snapshot().unwrap().model;
        assert_eq!(
            old.origin(location(input, 0)),
            current.origin(location(input, 1)),
            "restoration replaced the original saved origin"
        );
        assert!(
            current.origin(location(input, 0)).is_none(),
            "recovery kept the previous short address reserved"
        );
        assert!(!routing::generations(&current).targets.contains_key(&source));
        if input {
            assert_eq!(
                current.origin(Location::InputInstance {
                    bus: 0,
                    short_address: 1,
                    instance: 0
                }),
                old.origin(Location::InputInstance {
                    bus: 0,
                    short_address: 0,
                    instance: 0
                }),
                "restoration replaced an original input child identity"
            );
        }
        let actual = native(&emulator.emulator).await;
        assert_eq!(
            actual["buses"][0][if input { "inputs" } else { "outputs" }][0]["short_address"],
            1
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

fn identified(revision: u64) -> Action {
    Action::Identified {
        candidate: "Bus1.Outputs.Candidate".into(),
        candidate_generation: context(1).generation().unwrap().into(),
        expected_revision: revision,
    }
}

#[tokio::test]
async fn restoration_address_adoption_frees_old_slot_for_the_next_new_fixture() {
    for input in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let emulator = physical(input, 2, false, 1).await;
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let original = handle
            .snapshot()
            .unwrap()
            .model
            .origin(location(input, 0))
            .unwrap()
            .clone();
        let candidate = if input {
            handle
                .assign_next_input(0, 1, context(1), limit())
                .await
                .unwrap()
        } else {
            handle
                .assign_next_output(0, 1, context(1), limit())
                .await
                .unwrap()
        };
        assert_eq!(candidate.address, Some(1));
        let selected = candidate.target.unwrap();
        let generation =
            routing::generations(&handle.snapshot().unwrap().model).targets[&selected].clone();
        let restored = handle
            .restore_address(
                target(input),
                Action::Identified {
                    candidate: selected,
                    candidate_generation: generation,
                    expected_revision: candidate.revision,
                },
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(restored.outcome, ResultKind::Restored);
        let next = if input {
            handle
                .assign_next_input(0, restored.revision, context(restored.revision), limit())
                .await
                .unwrap()
        } else {
            handle
                .assign_next_output(0, restored.revision, context(restored.revision), limit())
                .await
                .unwrap()
        };
        assert_eq!(next.outcome, AssignmentOutcome::Assigned);
        assert_eq!(
            next.address,
            Some(0),
            "new fixture could not reuse the released previous address"
        );
        let model = handle.snapshot().unwrap().model;
        assert_eq!(model.origin(location(input, 1)), Some(&original));
        assert_ne!(
            model.origin(location(input, 0)),
            Some(&original),
            "released short address inherited old bindings"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

async fn addressed_candidate() -> pty::PtyEmulator {
    let emulator = physical(false, 1, false, 1).await;
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "light-0".into(),
                short_address: Some(1),
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    emulator
}

#[tokio::test]
async fn restoration_failed_or_uncertain_candidate_save_never_moves_the_address() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        fixture.add_output(1, "Candidate");
        let emulator = addressed_candidate().await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected candidate retirement failure".into())
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
        assert!(
            handle
                .restore_address(target(false), identified(1), context(1), limit())
                .await
                .is_err(),
            "unconfirmed candidate retirement was accepted"
        );
        assert_eq!(
            native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
            1,
            "address moved before a confirmed candidate retirement save"
        );
        if at == Step::DirectorySync(Stage::Mapping) {
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
        } else {
            assert!(handle
                .snapshot()
                .unwrap()
                .model
                .origin(location(false, 1))
                .is_some());
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn restoration_held_save_keeps_permit_and_original_physical_stamp() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Candidate");
    let emulator = addressed_candidate().await;
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
        h.restore_address(target(false), identified(1), context(1), limit())
            .await
    });
    hold.entered().await;
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Writing);
    assert_eq!(
        native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
        1
    );
    assert!(
        matches!(
            handle
                .read_output(target(false), context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "candidate retirement released the physical permit"
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
        report.outcome,
        ResultKind::ConfigurationUpdatedButUnconfirmed,
        "mapping save silently accepted changed physical evidence"
    );
    assert!(report.configuration_changed);
    assert_eq!(report.revision, 2);
    assert!(handle
        .snapshot()
        .unwrap()
        .model
        .origin(location(false, 0))
        .is_none());
    assert_eq!(
        native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
        1,
        "address moved after the original physical evidence changed"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn restoration_final_evidence_and_unknown_profile_cannot_report_success() {
    for known in [false, true] {
        let fixture = Fixture::new();
        let emulator = physical(false, 1, false, 1).await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        if known {
            service.output_acceptance_hook =
                Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        }
        let running = start(
            service.with_uart(
                &emulator.path,
                if known {
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
            .restore_address(target(false), unique(1), context(1), limit())
            .await;
        if known {
            assert_eq!(
                result.unwrap().outcome,
                ResultKind::Unconfirmed,
                "stale final restoration evidence claimed success"
            );
            assert_eq!(
                native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
                0
            );
        } else {
            assert!(
                result.is_err(),
                "unknown repeat profile admitted address restoration"
            );
            assert!(
                native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"]
                    .is_null()
            );
        }
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn restoration_mqtt_helper_never_replays_old_press_after_new_event_and_query() {
    let fixture = Fixture::new();
    fixture.add_button();
    let emulator = physical(true, 1, false, 1).await;
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "input-0".into(),
                short_address: Some(0),
            },
            Duration::from_secs(1),
        )
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
    let button = "Bus1.Inputs.Panel.Instances.Button";
    wait_for(&handle, |s| s.input_listeners.contains(button)).await;
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
    let original = handle.snapshot().unwrap().model;
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "input-0".into(),
                short_address: None,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    let report = handle
        .restore_address(target(true), unique(1), context(1), limit())
        .await
        .unwrap();
    assert_eq!(report.outcome, ResultKind::Restored);
    wait_for(&handle, |s| s.input_listeners.contains(button)).await;
    handle
        .read_input(button, context(1), deadline())
        .await
        .unwrap();
    let mut notices = handle.subscribe_inputs();
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
    tokio::time::timeout(Duration::from_secs(1), notices.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        original.state_bytes().unwrap(),
        handle.snapshot().unwrap().model.state_bytes().unwrap()
    );
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
        "restoration replayed a pre-repair queued action after fresh activity"
    );
    assert_eq!(actions[0]["action"], "released");
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn restoration_mqtt_requires_both_original_identities_and_keeps_original_reply() {
    let fixture = Fixture::new();
    fixture.add_button();
    fixture.add_output(1, "Candidate");
    let emulator = addressed_candidate().await;
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
    let original = handle.snapshot().unwrap().model;
    let valid = json!({"command":"restore_identified_address","target":target(false),"candidate":"Bus1.Outputs.Candidate","candidate_generation":context(1).generation(),"expected_revision":1});
    let mut invalids = vec![
        json!({"command":"restore_unaddressed_address","expected_revision":1}),
        json!({"command":"restore_unaddressed_address","target":"Bus1.Groups.All","expected_revision":1}),
        json!({"command":"restore_unaddressed_address","target":target(false),"expected_revision":1,"address":0}),
        json!({"command":"restore_identified_address","target":target(false),"candidate":"Bus1.Outputs.Candidate","expected_revision":1}),
    ];
    for (field, value) in [
        ("candidate_generation", json!(context(2).generation())),
        ("candidate", json!(target(false))),
        ("candidate", json!(target(true))),
        ("expected_revision", json!(2)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        invalids.push(invalid);
    }
    for (i, invalid) in invalids.into_iter().enumerate() {
        let reply =
            super::groups::call(&broker, invalid, 1, &format!("test/restore-invalid/{i}")).await;
        assert!(
            reply.get("error").is_some(),
            "invalid restoration authority was accepted: {reply}"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert_eq!(
            native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
            1
        );
    }
    let reply = super::groups::call(&broker, valid, 1, "test/restore-ok").await;
    assert_eq!(
        reply["value"]["outcome"], "restored",
        "restoration lost original reply after retiring candidate: {reply}"
    );
    assert_eq!(reply["value"]["revision"], 2);
    assert_eq!(
        reply["value"]["target_generation"],
        context(1).generation().unwrap()
    );
    assert_eq!(
        handle.snapshot().unwrap().model.origin(location(false, 1)),
        original.origin(location(false, 0))
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn restoration_unique_exact_collision_and_original_deadline_never_program() {
    for collision in [true, false] {
        let fixture = Fixture::new();
        let mut disk = fixture.disk();
        disk.buses[0].outputs[0].short_address = 1;
        disk.buses[0].groups[0].members = vec![1];
        fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
        let peer = super::commissioning::Peer::start(0x123456, false, false, false, collision);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .restore_address(
                target(false),
                unique(1),
                context(1),
                if collision {
                    limit()
                } else {
                    Instant::now() + Duration::from_millis(40)
                },
            )
            .await;
        if collision {
            assert_eq!(
                result.unwrap().outcome,
                ResultKind::Unconfirmed,
                "ambiguous exact random selection restored an address"
            );
        } else {
            assert!(
                result.is_err(),
                "restoration renewed the original randomization deadline"
            );
        }
        assert!(running.stop().await);
        assert!(
            !peer
                .commands
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with(b"hB7")),
            "collision or expired restore sent PROGRAM SHORT ADDRESS"
        );
        assert_eq!(fixture.disk().revision, 1);
    }
}

#[tokio::test]
async fn restoration_cancelled_candidate_save_never_programs_after_caller_leaves() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Candidate");
    let emulator = addressed_candidate().await;
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
        h.restore_address(target(false), identified(1), context(1), limit())
            .await
    });
    hold.entered().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    hold.release.store(true, Ordering::SeqCst);
    wait_for(&handle, |s| s.phase != Phase::Writing).await;
    assert_eq!(
        native(&emulator.emulator).await["buses"][0]["outputs"][0]["short_address"],
        1,
        "cancelled candidate retirement moved the address after its caller left"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
