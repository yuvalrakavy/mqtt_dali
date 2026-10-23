use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
    service::discovery::{Quality, Space},
};
use std::collections::{BTreeMap, VecDeque};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Literal peer is independent of both bridge query codecs and emulator models.
struct Peer {
    path: PathBuf,
    commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
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
    fn start(replies: Vec<(&str, Vec<&str>)>) -> Self {
        Self::start_held(replies, false)
    }
    fn start_held(replies: Vec<(&str, Vec<&str>)>, mut hold: bool) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (signal, resume) = (held.clone(), release.clone());
        let mut replies: BTreeMap<Vec<u8>, VecDeque<Vec<u8>>> = replies
            .into_iter()
            .map(|(c, r)| {
                (
                    c.as_bytes().to_vec(),
                    r.into_iter().map(|s| s.as_bytes().to_vec()).collect(),
                )
            })
            .collect();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                assert!(line.len() < 32);
                recorded.lock().unwrap().push(line.clone());
                if hold && line.as_slice() == b"h0390\n" {
                    hold = false;
                    write.write_all(b"L008000\n").await.unwrap();
                    signal.notify_one();
                    resume.notified().await;
                }
                let response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    b"h01A0\n" => b"J64\n".to_vec(),
                    b"h0190\n" => b"J04\n".to_vec(),
                    _ => replies
                        .get_mut(&line)
                        .and_then(VecDeque::pop_front)
                        .unwrap_or_else(|| panic!("unexpected discovery query: {line:?}")),
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

#[tokio::test]
async fn discovery_waits_for_busy_owner_without_renewing_deadline_or_sending_canceled_work() {
    let fixture = Fixture::new();
    let peer = Peer::start(vec![("h0390\n", vec!["N\n"])]);
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::BeforeRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    // The same permit is held during an ordinary background state read.
    let busy = handle.permit.clone().try_acquire_owned().unwrap();
    let client = handle.clone();
    let waiting = tokio::spawn(async move { probe(&client, Space::Outputs, 1, 1).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        !waiting.is_finished(),
        "discovery aborted instead of waiting for the busy owner"
    );
    waiting.abort();
    let _ = waiting.await;
    let until = Instant::now() + Duration::from_millis(30);
    let expired = handle
        .discover_address(0, Space::Outputs, 1, 1, context(1), until)
        .await;
    assert!(
        matches!(expired, Err(ServiceError::Expired)),
        "waiting discovery renewed its deadline or returned Busy"
    );
    assert!(
        !peer
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|c| c == b"h0390\n"),
        "unadmitted discovery reached UART"
    );
    let client = handle.clone();
    let waiting = tokio::spawn(async move { probe(&client, Space::Outputs, 1, 1).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        !waiting.is_finished(),
        "discovery aborted instead of waiting for the busy owner"
    );
    drop(busy);
    let report = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report.probe.quality, Quality::Missing);
    assert_eq!(
        peer.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_slice() == b"h0390\n")
            .count(),
        1,
        "canceled or expired discovery ran later"
    );
    // Waiting does not grant permission to replace the request's original root.
    let client = handle.clone();
    let edit = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::Description(Some("changed".into())),
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    let client = handle.clone();
    let stale = tokio::spawn(async move {
        client
            .discover_address(0, Space::Outputs, 1, 1, context(1), deadline())
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        !stale.is_finished(),
        "discovery aborted instead of waiting for the busy owner"
    );
    hold.release.store(true, Ordering::SeqCst);
    edit.await.unwrap().unwrap();
    assert!(
        matches!(stale.await.unwrap(), Err(ServiceError::Rejected(_))),
        "waiting discovery accepted retired authority"
    );
    assert_eq!(
        peer.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_slice() == b"h0390\n")
            .count(),
        1,
        "retired queued discovery reached UART"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn discovery_canceled_caller_keeps_owner_until_settlement_and_accepts_native_input() {
    let fixture = Fixture::new();
    fixture.add_button();
    let peer = Peer::start_held(
        vec![("h0390\n", vec!["J00\n"]), ("h0399\n", vec!["JFE\n"])],
        true,
    );
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
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
    let pending = tokio::spawn(async move { probe(&client, Space::Outputs, 1, 1).await });
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
        "canceled discovery caller released an active physical owner"
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
        "canceled discovery adopted inventory after caller departure"
    );
    assert_eq!(fixture.disk().buses[0].outputs.len(), 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn discovery_stamp_loss_before_save_refuses_adoption() {
    let fixture = Fixture::new();
    let peer = Peer::start(vec![
        ("h0390\n", vec!["J00\n"]),
        ("h0399\n", vec!["JFE\nD04\nD24\n"]),
    ]);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .discover_address(0, Space::Outputs, 1, 1, context(1), deadline())
        .await;
    assert!(
        result.is_err(),
        "discovery adopted evidence from a retired physical epoch"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn discovery_new_input_modules_keep_common_unknown_and_zero_instance_metadata() {
    for types in [vec![1, 2, 3, 4, 31], vec![]] {
        let fixture = Fixture::new();
        let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"panel","short_address":0,"instances":types}]}]})).unwrap();
        let emulator = pty::spawn(scenario).await.unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = probe(&handle, Space::Inputs, 0, 1).await;
        assert_eq!(report.probe.instance_types, types);
        assert!(report.probe.instances_complete);
        assert_eq!(
            report.added_items,
            1 + types.len(),
            "discovery count omitted newly saved child items"
        );
        assert_eq!(report.target.as_deref(), Some("Bus1.Inputs.Input1"));
        assert_eq!(
            fixture.disk().buses[0].outputs.len(),
            1,
            "input adoption replaced an output at the same address"
        );
        assert_eq!(
            fixture.disk().buses[0].inputs[0].instances.len(),
            types.len()
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}
async fn probe(
    handle: &ServiceHandle,
    space: Space,
    address: u8,
    revision: u64,
) -> super::super::discovery::Report {
    handle
        .discover_address(0, space, address, revision, context(revision), deadline())
        .await
        .unwrap()
}

#[tokio::test]
async fn discovery_literal_multi_type_terminator_and_malformed_prefixes() {
    for (sequence, types, complete) in [
        (
            vec![
                "J00\n", "J01\n", "J02\n", "J03\n", "J04\n", "J05\n", "J06\n", "J07\n", "J08\n",
                "J09\n", "JFE\n",
            ],
            (0..10).collect(),
            true,
        ),
        (vec!["J06\n", "J08\n", "JFE\n"], vec![6, 8], true),
        (vec!["J06\n", "J08\n", "JFF\n"], vec![6, 8], false),
        (vec!["J08\n", "J06\n"], vec![8], false),
        (vec!["J06\n", "X\n"], vec![6], false),
        (vec!["J06\n", "N\n"], vec![6], false),
    ] {
        let expected_queries = sequence.len();
        let fixture = Fixture::new();
        let peer = Peer::start(vec![
            ("h0390\n", vec!["J00\n"]),
            ("h0399\n", vec!["JFF\n"]),
            ("h03A7\n", sequence),
        ]);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = probe(&handle, Space::Outputs, 1, 1).await;
        assert_eq!(
            report.probe.device_types, types,
            "discovery lost its verified type prefix"
        );
        assert_eq!(
            report.probe.types_complete, complete,
            "discovery used an invalid type-list terminator"
        );
        assert_eq!(
            report.probe.quality,
            if complete {
                Quality::Complete
            } else {
                Quality::Partial
            }
        );
        assert_eq!(
            report.revision, 2,
            "responding basic output was lost with partial type information"
        );
        assert_eq!(report.probe.status, Some(0));
        assert_eq!(
            peer.commands
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.as_slice() == b"h03A7\n")
                .count(),
            expected_queries
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn discovery_save_noncommit_and_uncertainty_reuse_existing_recovery() {
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
                    Err("injected discovery save failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .discover_address(0, Space::Outputs, 1, 1, context(1), deadline())
            .await;
        assert_eq!(
            handle.snapshot().unwrap().model.inventory().revision,
            1,
            "failed discovery save activated unconfirmed inventory"
        );
        if at == Step::BeforeRename(Stage::Reservation) {
            assert!(matches!(result, Err(ServiceError::Rejected(_))));
            assert_eq!(handle.snapshot().unwrap().phase, Phase::Ready);
            assert_eq!(fixture.disk().revision, 1);
        } else {
            assert!(matches!(result, Err(ServiceError::Uncertain(_))));
            assert!(
                matches!(handle.snapshot().unwrap().phase, Phase::Uncertain(_)),
                "uncertain discovery save admitted routing"
            );
            let recovered = handle.recover(context(1), deadline()).await.unwrap();
            assert_eq!(recovered.model.inventory().revision, 2);
            assert_eq!(recovered.model.inventory().buses[0].outputs.len(), 2);
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn discovery_literal_missing_collision_and_partial_input_do_not_create_items() {
    for (space, replies, quality) in [
        (
            Space::Outputs,
            vec![("h0390\n", vec!["N\n"])],
            Quality::Missing,
        ),
        (
            Space::Outputs,
            vec![("h0390\n", vec!["X\n"])],
            Quality::Collision,
        ),
        (
            Space::Inputs,
            vec![("l01FE30\n", vec!["JFF\n"]), ("l01FE35\n", vec!["N\n"])],
            Quality::Partial,
        ),
        (
            Space::Inputs,
            vec![("l01FE30\n", vec!["J00\n"]), ("l01FE35\n", vec!["J21\n"])],
            Quality::Partial,
        ),
        (
            Space::Inputs,
            vec![
                ("l01FE30\n", vec!["J00\n"]),
                ("l01FE35\n", vec!["J01\n"]),
                ("l010080\n", vec!["J20\n"]),
            ],
            Quality::Partial,
        ),
    ] {
        let fixture = Fixture::new();
        let peer = Peer::start(replies);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = probe(
            &handle,
            space,
            if space == Space::Outputs { 1 } else { 0 },
            1,
        )
        .await;
        assert_eq!(report.probe.quality, quality);
        assert_eq!(
            report.revision, 1,
            "incomplete discovery fabricated saved inventory"
        );
        assert_eq!(report.added_items, 0);
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn discovery_adds_input_children_without_replacing_the_module_or_existing_child() {
    let fixture = Fixture::new();
    fixture.add_button();
    let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"panel","short_address":0,"instances":[1,2,31]}]}]})).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let before = handle.snapshot().unwrap().model;
    let report = probe(&handle, Space::Inputs, 0, 1).await;
    assert_eq!(report.added_items, 2);
    assert_eq!(report.probe.instance_types, vec![1, 2, 31]);
    let after = handle.snapshot().unwrap().model;
    for name in [
        "Bus1.Inputs.Panel",
        "Bus1.Inputs.Panel.Instances.Button",
        "Bus1.Outputs.Light1",
    ] {
        let old = before
            .projections()
            .into_iter()
            .find(|p| p.target == name)
            .unwrap();
        let new = after
            .projections()
            .into_iter()
            .find(|p| p.target == name)
            .unwrap();
        assert_eq!(old, new, "input expansion replaced a continuing item");
    }
    assert_eq!(
        fixture.disk().buses[0].inputs[0]
            .instances
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Button", "Input1", "Input2"]
    );
    let repeat = probe(&handle, Space::Inputs, 0, 2).await;
    assert_eq!(repeat.revision, 2);
    assert_eq!(repeat.added_items, 0);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn discovery_input_mismatch_and_smaller_count_preserve_saved_children() {
    for types in [vec![2, 1], vec![]] {
        let fixture = Fixture::new();
        fixture.add_button();
        let before = fixture.disk();
        let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"panel","short_address":0,"instances":types}]}]})).unwrap();
        let emulator = pty::spawn(scenario).await.unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = probe(&handle, Space::Inputs, 0, 1).await;
        assert_eq!(
            report.probe.quality,
            if types.is_empty() {
                Quality::Complete
            } else {
                Quality::TypeMismatch
            }
        );
        assert_eq!(
            serde_json::to_value(fixture.disk()).unwrap(),
            serde_json::to_value(before).unwrap(),
            "discovery rewrote saved input types or removed absent children"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn discovery_retired_authority_and_invalid_bounds_send_no_uart_probe() {
    let fixture = Fixture::new();
    let peer = Peer::start(vec![]);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for (bus, address, revision, cause) in [
        (0, 1, 2, context(1)),
        (0, 1, 1, context(2)),
        (1, 1, 1, context(1)),
        (4, 1, 1, context(1)),
        (0, 64, 1, context(1)),
        (0, 1, 1, RequestContext::default()),
    ] {
        assert!(
            handle
                .discover_address(bus, Space::Outputs, address, revision, cause, deadline())
                .await
                .is_err(),
            "discovery admitted invalid authority/address"
        );
    }
    assert!(
        peer.commands.lock().unwrap().iter().all(|c| matches!(
            c.as_slice(),
            b"v\n" | b"d\n" | b"h01A0\n" | b"h0190\n" | b"h0199\n"
        )),
        "invalid discovery reached UART"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn discovery_committed_save_keeps_historical_probe_when_bus_disappears() {
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
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move { probe(&client, Space::Outputs, 1, 1).await });
    hold.entered().await;
    assert!(
        matches!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "discovery released physical permit during durable save"
    );
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    hold.release.store(true, Ordering::SeqCst);
    let report = pending.await.unwrap();
    assert_eq!(report.revision, 2);
    assert_eq!(fixture.disk().revision, 2);
    assert_eq!(report.probe.quality, Quality::Complete);
    let snapshot = handle.snapshot().unwrap();
    assert!(
        !snapshot.outputs.contains_key("Bus1.Outputs.Light2"),
        "historical discovery fabricated current output evidence"
    );
    assert!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light2")
            .iter()
            .all(
                |r| serde_json::from_slice::<serde_json::Value>(&r.payload).unwrap()["Available"]
                    != true
            ),
        "discovery claimed current fixture availability while saving"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
