use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
    service::commissioning::AssignmentOutcome,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(super) struct Peer {
    pub(super) path: PathBuf,
    pub(super) commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    held: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    /// Independent literal addressing peer; RANDOMISE deliberately selects a
    /// forced boundary value, not the emulator/bridge search implementation.
    pub(super) fn start(
        random: u32,
        two: bool,
        fail_final_status: bool,
        hold_compare: bool,
        exact_collision: bool,
    ) -> Self {
        Self::start_held(
            random,
            two,
            fail_final_status,
            hold_compare,
            if exact_collision { 1 } else { 0 },
            false,
        )
    }
    fn start_held(
        random: u32,
        two: bool,
        fail_final_status: bool,
        mut hold_compare: bool,
        exact_fault: u8,
        mut hold_program: bool,
    ) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (signal, resume) = (held.clone(), release.clone());
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            let mut search = 0xffffffu32;
            let mut initialized = false;
            let mut address = None;
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                assert!(line.len() < 32);
                recorded.lock().unwrap().push(line.clone());
                let response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    b"h01A0\n" => b"J64\n".to_vec(),
                    b"h0190\n" => b"J04\n".to_vec(),
                    command if command.len() == 6 && matches!(command[0], b'h' | b't') => {
                        let raw =
                            u16::from_str_radix(std::str::from_utf8(&command[1..5]).unwrap(), 16)
                                .unwrap();
                        if raw == 0xa900 && hold_compare {
                            hold_compare = false;
                            write.write_all(b"L008000\n").await.unwrap();
                            signal.notify_one();
                            resume.notified().await;
                        }
                        let value = match raw {
                            0x0390 => {
                                if address == Some(1) && !fail_final_status {
                                    Some(0)
                                } else {
                                    None
                                }
                            }
                            0xa100 => {
                                initialized = false;
                                None
                            }
                            0xa5ff => {
                                assert_eq!(command[0], b't');
                                initialized = true;
                                None
                            }
                            0xa700 => {
                                assert_eq!(command[0], b't');
                                None
                            }
                            0xa900 => {
                                if initialized && address.is_none() && search >= random {
                                    if exact_fault != 0 && search == random {
                                        let reply: &[u8] = match exact_fault {
                                            1 => b"X\n",
                                            2 => b"Z\n",
                                            _ => b"J01\n",
                                        };
                                        write.write_all(reply).await.unwrap();
                                        line.clear();
                                        continue;
                                    }
                                    Some(255)
                                } else {
                                    None
                                }
                            }
                            0xbb00 => {
                                if initialized && search == random {
                                    Some(address.map_or(255, |a| (a << 1) | 1))
                                } else {
                                    None
                                }
                            }
                            0xb701 | 0xb703 => {
                                assert!(initialized && search == random);
                                address = Some(((raw & 255) as u8) >> 1);
                                None
                            }
                            0xb903 => {
                                if initialized && address == Some(1) {
                                    Some(255)
                                } else {
                                    None
                                }
                            }
                            0xab00 => None,
                            frame if frame >> 8 == 0xb1 => {
                                search = (search & 0xffff) | (((frame & 255) as u32) << 16);
                                None
                            }
                            frame if frame >> 8 == 0xb3 => {
                                search = (search & 0xff00ff) | (((frame & 255) as u32) << 8);
                                None
                            }
                            frame if frame >> 8 == 0xb5 => {
                                search = (search & 0xffff00) | ((frame & 255) as u32);
                                None
                            }
                            _ => panic!("unexpected literal commissioning command {command:?}"),
                        };
                        if raw == 0xb703 && hold_program {
                            hold_program = false;
                            write.write_all(b"L008000\n").await.unwrap();
                            signal.notify_one();
                            resume.notified().await;
                        }
                        let mut response = value.map_or_else(
                            || b"N\n".to_vec(),
                            |v| format!("J{v:02X}\n").into_bytes(),
                        );
                        if command[0] == b't' && two {
                            response.extend_from_slice(b"N\n");
                        }
                        response
                    }
                    other => panic!("unexpected commissioning peer request {other:?}"),
                };
                write.write_all(&response).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            commands,
            held,
            release,
            task,
        }
    }
}
async fn assign(handle: &ServiceHandle) -> super::super::commissioning::Report {
    handle
        .assign_next_output(0, 1, context(1), Instant::now() + Duration::from_secs(5))
        .await
        .unwrap()
}
async fn control(
    emulator: &crate::emulator::session::Emulator,
    command: Control,
) -> serde_json::Value {
    emulator
        .control(command, Duration::from_secs(1))
        .await
        .unwrap()
}
fn unaddressed(snapshot: &serde_json::Value) -> bool {
    snapshot["buses"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == "new-fixture")
        .unwrap()["short_address"]
        .is_null()
}

#[tokio::test]
async fn commissioning_literal_search_includes_both_24_bit_boundaries_and_repeat_profiles() {
    for (random, two) in [
        (0, false),
        (1, true),
        (0x7fffff, false),
        (0x800000, true),
        (0xfffffe, false),
        (0xffffff, true),
    ] {
        let fixture = Fixture::new();
        let peer = Peer::start(random, two, false, false, false);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let policy = if two {
            RepeatPolicy::TwoTerminals
        } else {
            RepeatPolicy::OneTerminal
        };
        let running = start(
            service.with_uart(peer.path.clone(), policy),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .assign_next_output(0, 1, context(1), Instant::now() + Duration::from_secs(5))
            .await;
        assert!(
            result.is_ok(),
            "inclusive search missed a valid boundary: {result:?}"
        );
        let report = result.unwrap();
        assert_eq!(
            report.outcome,
            AssignmentOutcome::Assigned,
            "inclusive search missed a valid boundary"
        );
        assert_eq!(report.address, Some(1));
        assert_eq!(report.revision, 2);
        let commands = peer.commands.lock().unwrap().clone();
        assert_eq!(
            commands
                .iter()
                .filter(|c| c.as_slice() == b"hB703\n")
                .count(),
            1,
            "programming was duplicated or omitted"
        );
        assert!(
            commands
                .iter()
                .filter(|c| c.as_slice() == b"hA900\n")
                .count()
                <= 28,
            "binary search exceeded its finite comparison bound"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn commissioning_failed_post_program_query_returns_typed_reserved_inventory() {
    let fixture = Fixture::new();
    let peer = Peer::start(0x123456, false, true, false, false);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let report = assign(&handle).await;
    assert_eq!(
        report.outcome,
        AssignmentOutcome::ReservedButUnconfirmed,
        "failed verification claimed assignment success or lost the reservation result"
    );
    assert_eq!(report.address, Some(1));
    assert_eq!(report.target.as_deref(), Some("Bus1.Outputs.Light2"));
    assert_eq!(report.revision, 2);
    assert!(report.reason.unwrap().contains("fresh status"));
    assert_eq!(
        fixture.disk().buses[0].outputs[1].short_address,
        1,
        "unconfirmed assignment lost its durable reservation"
    );
    assert!(
        !handle
            .snapshot()
            .unwrap()
            .outputs
            .contains_key("Bus1.Outputs.Light2"),
        "assignment manufactured current light State"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn commissioning_exact_random_collision_never_reserves_or_programs() {
    for fault in 1..=3 {
        let fixture = Fixture::new();
        let peer = Peer::start_held(0x800000, false, false, false, fault, false);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .assign_next_output(0, 1, context(1), deadline())
            .await;
        assert!(
            result.is_err(),
            "ambiguous exact random selection was programmed"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(
            !peer
                .commands
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with(b"hB7")),
            "collision reached PROGRAM"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn commissioning_bus_epoch_change_during_reservation_skips_programming() {
    let fixture = Fixture::new();
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move { assign(&client).await });
    hold.entered().await;
    assert!(
        unaddressed(&control(&emulator.emulator, Control::Snapshot).await),
        "PROGRAM preceded durable reservation"
    );
    assert!(
        matches!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "reservation released the physical permit before programming handoff"
    );
    control(
        &emulator.emulator,
        Control::BusPower {
            bus: 0,
            powered: false,
        },
    )
    .await;
    control(
        &emulator.emulator,
        Control::BusPower {
            bus: 0,
            powered: true,
        },
    )
    .await;
    hold.release.store(true, Ordering::SeqCst);
    let report = pending.await.unwrap();
    assert_eq!(
        report.outcome,
        AssignmentOutcome::ReservedButUnconfirmed,
        "new physical epoch refreshed a stale commissioning candidate"
    );
    assert_eq!(report.revision, 2);
    assert_eq!(fixture.disk().revision, 2);
    assert!(
        unaddressed(&control(&emulator.emulator, Control::Snapshot).await),
        "stale candidate was programmed after a save-time bus cycle"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn commissioning_save_failures_do_not_program_and_recovery_only_adopts_reservation() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let emulator = pty::spawn(
            serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
        )
        .await
        .unwrap();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected reservation failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .assign_next_output(0, 1, context(1), deadline())
            .await;
        if at == Step::BeforeRename(Stage::Reservation) {
            assert!(matches!(result, Err(ServiceError::Rejected(_))));
            assert_eq!(fixture.disk().revision, 1);
        } else {
            assert!(matches!(result, Err(ServiceError::Uncertain(_))));
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
            assert_eq!(
                handle
                    .recover(context(1), deadline())
                    .await
                    .unwrap()
                    .model
                    .inventory()
                    .revision,
                2
            );
        }
        assert!(
            unaddressed(&control(&emulator.emulator, Control::Snapshot).await),
            "unconfirmed reservation programmed hardware"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn commissioning_cancellation_during_search_keeps_owner_and_native_events() {
    let fixture = Fixture::new();
    fixture.add_button();
    let peer = Peer::start(0x123456, false, false, true, false);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| {
        matches!(s.hardware, Hardware::Ready { .. }) && !s.input_listeners.is_empty()
    })
    .await;
    let mut events = handle.subscribe_inputs();
    let client = handle.clone();
    let pending = tokio::spawn(async move { assign(&client).await });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    pending.abort();
    let _ = pending.await;
    assert!(
        matches!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "cancelled search released its physical owner before settlement"
    );
    peer.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        while handle.permit.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture.disk().revision,
        1,
        "cancelled search reserved an address after caller departure"
    );
    assert!(!peer
        .commands
        .lock()
        .unwrap()
        .iter()
        .any(|c| c.starts_with(b"hB7")));
    assert!(running.stop().await);
}

#[tokio::test]
async fn commissioning_unverified_profile_refuses_before_initialising_devices() {
    let fixture = Fixture::new();
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let error = handle
        .assign_next_output(0, 1, context(1), deadline())
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("known HAT profile"),
        "unverified commissioning started an addressing sequence"
    );
    let snapshot = control(&emulator.emulator, Control::Snapshot).await;
    assert!(snapshot["buses"][0]["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|o| o["initialize_until"].is_null()));
    assert_eq!(fixture.disk().revision, 1);
    assert!(unaddressed(&snapshot));
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn commissioning_reserved_missing_and_unsaved_occupied_addresses_are_never_reused() {
    for all_reserved in [true, false] {
        let fixture = Fixture::new();
        let mut inventory = fixture.disk();
        if all_reserved {
            inventory.buses[0].outputs = (0..64)
                .map(|address| crate::config::Output {
                    short_address: address,
                    name: format!("Reserved{address}"),
                    description: None,
                    color_representation: None,
                })
                .collect();
            fs::write(
                &fixture.paths.current,
                serde_json::to_vec(&inventory).unwrap(),
            )
            .unwrap();
        }
        let mut outputs = Vec::new();
        if !all_reserved {
            for address in 1..64 {
                outputs.push(json!({"id":format!("used-{address}"),"short_address":address}));
            }
        }
        outputs.push(json!({"id":"new-fixture","short_address":null}));
        let emulator = pty::spawn(
            serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":outputs}]}))
                .unwrap(),
        )
        .await
        .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = assign(&handle).await;
        assert_eq!(
            report.outcome,
            AssignmentOutcome::NoFreeAddress,
            "commissioning reused a reserved or occupied address"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(unaddressed(
            &control(&emulator.emulator, Control::Snapshot).await
        ));
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn commissioning_original_deadline_bounds_the_randomisation_wait_without_reservation() {
    let fixture = Fixture::new();
    let peer = Peer::start(0x123456, false, false, false, false);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .assign_next_output(0, 1, context(1), Instant::now() + Duration::from_millis(50))
        .await;
    assert!(
        result.is_err(),
        "randomisation renewed an expired assignment deadline"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while handle.permit.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.disk().revision, 1);
    assert!(!peer
        .commands
        .lock()
        .unwrap()
        .iter()
        .any(|c| c.starts_with(b"hB7")));
    assert!(
        handle
            .read_output("Bus1.Outputs.Light1", context(1), deadline())
            .await
            .is_ok(),
        "expired addressing mode blocked ordinary light commands"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn commissioning_retired_authority_and_invalid_bus_never_reach_uart() {
    let fixture = Fixture::new();
    let peer = Peer::start(0x123456, false, false, false, false);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for (bus, expected, cause) in [
        (1, 1, context(1)),
        (4, 1, context(1)),
        (0, 2, context(1)),
        (0, 1, context(2)),
        (0, 1, RequestContext::default()),
    ] {
        assert!(
            handle
                .assign_next_output(bus, expected, cause, deadline())
                .await
                .is_err(),
            "assignment admitted retired authority or an invalid bus"
        );
    }
    assert!(
        peer.commands.lock().unwrap().iter().all(|c| matches!(
            c.as_slice(),
            b"v\n" | b"d\n" | b"h01A0\n" | b"h0190\n" | b"h0199\n"
        )),
        "invalid assignment reached UART"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn commissioning_cancel_and_managed_stop_during_program_keep_the_saved_reservation() {
    for shutdown in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let peer = Peer::start_held(0x123456, false, false, false, 0, true);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| {
            matches!(s.hardware, Hardware::Ready { .. }) && !s.input_listeners.is_empty()
        })
        .await;
        let mut events = handle.subscribe_inputs();
        let client = handle.clone();
        let pending = tokio::spawn(async move { assign(&client).await });
        tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            handle.snapshot().unwrap().model.inventory().revision,
            2,
            "programming started before durable runtime activation"
        );
        if shutdown {
            assert!(
                running.stop().await,
                "managed stop did not join the held programming phase"
            );
            let report = pending.await.unwrap();
            assert_eq!(report.outcome, AssignmentOutcome::ReservedButUnconfirmed);
        } else {
            pending.abort();
            let _ = pending.await;
            assert!(
                matches!(
                    handle
                        .read_output("Bus1.Outputs.Light1", context(1), deadline())
                        .await,
                    Err(ServiceError::Busy)
                ),
                "cancelled programming released ownership before settlement"
            );
            peer.release.notify_one();
            tokio::time::timeout(Duration::from_secs(1), async {
                while handle.permit.available_permits() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(running.stop().await);
        }
        assert_eq!(
            fixture.disk().revision,
            2,
            "abandoned programming removed a committed reservation"
        );
        assert_eq!(fixture.disk().buses[0].outputs[1].short_address, 1);
    }
}
