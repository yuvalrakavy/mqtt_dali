use super::*;
use crate::dali::gear::GroupAddress;
use crate::service::groups::{Action, Quality};
use std::sync::atomic::{AtomicU16, AtomicUsize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Independent literal peer: neither group frames nor masks use bridge helpers.
struct Peer {
    path: PathBuf,
    mask: Arc<AtomicU16>,
    held: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    low_reads: Arc<AtomicUsize>,
    commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    // fault 1/2/3: missing high/both/collision high; 4: no settings effect;
    // 5: high missing after settings; 6: change mask on second low read;
    // 7: collide on second setting; 8/9: hold first query/setting terminal.
    fn new(mask: u16, two: bool, fault: u8) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let mask = Arc::new(AtomicU16::new(mask));
        let current = mask.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (signal, resume) = (held.clone(), release.clone());
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let low_reads = Arc::new(AtomicUsize::new(0));
        let reads = low_reads.clone();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            let mut settings = 0;
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                recorded.lock().unwrap().push(line.clone());
                let f = fault;
                let mut response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    b"h01A0\n" => b"J64\n".to_vec(),
                    b"h0190\n" => b"J04\n".to_vec(),
                    b"h01C0\n" => {
                        let count = reads.fetch_add(1, Ordering::SeqCst) + 1;
                        if f == 8 && count == 1 {
                            write.write_all(b"L008001\n").await.unwrap();
                            signal.notify_one();
                            resume.notified().await;
                        }
                        if f == 6 && count == 2 {
                            current.store(2, Ordering::SeqCst);
                        }
                        if f == 2 {
                            b"N\n".to_vec()
                        } else {
                            format!("J{:02X}\n", current.load(Ordering::SeqCst) & 255).into_bytes()
                        }
                    }
                    b"h01C1\n" => match f {
                        1 | 2 => b"N\n".to_vec(),
                        3 => b"X\n".to_vec(),
                        5 if settings > 0 => b"N\n".to_vec(),
                        _ => format!("J{:02X}\n", current.load(Ordering::SeqCst) >> 8).into_bytes(),
                    },
                    cmd if cmd.len() == 6 && cmd[0] == b't' => {
                        let raw = u16::from_str_radix(std::str::from_utf8(&cmd[1..5]).unwrap(), 16)
                            .unwrap();
                        assert_eq!(raw >> 8, 1);
                        let op = (raw & 255) as u8;
                        assert!((0x60..=0x7f).contains(&op));
                        settings += 1;
                        if f != 4 && !(f == 7 && settings == 2) {
                            let bit = 1 << (op & 15);
                            let old = current.load(Ordering::SeqCst);
                            current.store(
                                if op < 0x70 { old | bit } else { old & !bit },
                                Ordering::SeqCst,
                            );
                        }
                        if f == 9 && settings == 1 {
                            write.write_all(b"L008001\n").await.unwrap();
                            signal.notify_one();
                            resume.notified().await;
                        }
                        if f == 7 && settings == 2 {
                            b"X\n".to_vec()
                        } else {
                            b"N\n".to_vec()
                        }
                    }
                    other => panic!("unexpected group peer command {other:?}"),
                };
                if two && line[0] == b't' {
                    response.extend_from_slice(b"N\n");
                }
                write.write_all(&response).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            mask,
            held,
            release,
            low_reads,
            commands,
            task,
        }
    }
    fn settings(&self) -> Vec<Vec<u8>> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s[0] == b't')
            .cloned()
            .collect()
    }
}
fn assignment(member: bool) -> Action {
    Action::Assign {
        group: GroupAddress::new(0).unwrap(),
        member,
    }
}

#[tokio::test]
async fn group_literal_probe_keeps_missing_halves_collision_and_both_repeat_profiles() {
    for (fault, quality, low, high) in [
        (0, Quality::Complete, Some(255), Some(255)),
        (1, Quality::Partial, Some(255), None),
        (2, Quality::Missing, None, None),
        (3, Quality::Collision, Some(255), None),
    ] {
        let fixture = Fixture::new();
        let peer = Peer::new(65535, false, fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let before = handle.snapshot().unwrap().outputs;
        let report = handle
            .read_groups(
                "Bus1.Outputs.Light1",
                RequestContext::default().with_traceparent(None),
                deadline(),
            )
            .await
            .unwrap();
        let probe = report.probe.unwrap();
        assert_eq!(
            probe.quality, quality,
            "incorrect captured group probe quality"
        );
        assert_eq!((probe.low, probe.high), (low, high));
        assert_eq!(
            probe.mask,
            if fault == 0 { Some(65535) } else { None },
            "incomplete group halves manufactured a full mask"
        );
        assert_eq!(
            handle.snapshot().unwrap().outputs,
            before,
            "group queries manufactured light State"
        );
        if fault != 0 {
            assert!(
                handle
                    .change_groups(
                        "Bus1.Outputs.Light1",
                        Action::Adopt {
                            reviewed_mask: 65535
                        },
                        1,
                        context(1),
                        deadline()
                    )
                    .await
                    .is_err(),
                "incomplete group evidence authorized adoption"
            );
            assert_eq!(fixture.disk().revision, 1);
        }
        assert!(peer.settings().is_empty());
        assert!(running.stop().await);
    }
    for two in [false, true] {
        let fixture = Fixture::new();
        let peer = Peer::new(0, two, 0);
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
        let report = handle
            .change_groups(
                "Bus1.Outputs.Light1",
                assignment(true),
                1,
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            report.outcome, "verified",
            "known repeat profile did not verify group write"
        );
        assert_eq!(peer.settings(), vec![b"t0160\n".to_vec()]);
        assert_eq!(peer.mask.load(Ordering::SeqCst), 1);
        assert_eq!(report.revision, 1);
        assert!(!report.configuration_changed);
        assert_eq!(
            peer.low_reads.load(Ordering::SeqCst),
            3,
            "no-op desired state skipped physical verification"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn group_adoption_creates_only_observed_slots_preserves_other_fixtures_and_never_writes() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Other");
    let mut inventory = fixture.disk();
    inventory.buses[0].groups[0].members = vec![0, 1];
    inventory.buses[0].groups[0].description = Some("keep".into());
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let peer = Peer::new(0x8002, false, 0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
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
    let before = handle.snapshot().unwrap().model.projections();
    assert!(
        handle
            .change_groups(
                "Bus1.Outputs.Light1",
                Action::Adopt { reviewed_mask: 0 },
                1,
                context(1),
                deadline()
            )
            .await
            .is_err(),
        "unreviewed group mask authorized adoption"
    );
    let result=call(&broker,json!({"command":"adopt_groups","target":"Bus1.Outputs.Light1","reviewed_mask":32770,"expected_revision":1}),1,"test/group-adopt").await;
    assert_eq!(
        result["value"]["outcome"], "adopted",
        "reviewed adoption failed: {result}"
    );
    assert_eq!(result["value"]["revision"], 2);
    let disk = fixture.disk();
    let groups = &disk.buses[0].groups;
    assert_eq!(groups.len(), 3, "adoption invented unobserved group slots");
    assert_eq!(
        groups[0].members,
        vec![1],
        "adoption changed another fixture's memberships"
    );
    assert_eq!(groups[0].description.as_deref(), Some("keep"));
    for slot in [1, 15] {
        assert_eq!(
            groups
                .iter()
                .find(|g| g.group_address == slot)
                .unwrap()
                .members,
            vec![0]
        );
    }
    let after = handle.snapshot().unwrap().model.projections();
    for target in ["Bus1.Outputs.Light1", "Bus1.Outputs.Other"] {
        assert_eq!(
            before.iter().find(|p| p.target == target),
            after.iter().find(|p| p.target == target)
        );
    }
    let old = before
        .iter()
        .find(|p| p.target == "Bus1.Groups.All")
        .unwrap();
    let new = after
        .iter()
        .find(|p| p.target == "Bus1.Groups.All")
        .unwrap();
    assert_eq!(
        (old.target.as_str(), old.location),
        (new.target.as_str(), new.location)
    );
    assert_ne!(old.binding_revision, new.binding_revision);
    let repeated = handle
        .change_groups(
            "Bus1.Outputs.Light1",
            Action::Adopt {
                reviewed_mask: 32770,
            },
            2,
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(repeated.revision, 2);
    assert!(!repeated.configuration_changed);
    assert!(
        peer.settings().is_empty(),
        "hardware-to-config adoption wrote physical settings"
    );
    assert_eq!(peer.mask.load(Ordering::SeqCst), 32770);
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_repair_requires_review_and_clears_unmanaged_bits_only_explicitly() {
    for fault in [0, 6] {
        let fixture = Fixture::new();
        let peer = Peer::new(0x8002, false, fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        assert!(
            handle
                .change_groups(
                    "Bus1.Outputs.Light1",
                    Action::Repair { reviewed_mask: 0 },
                    1,
                    context(1),
                    deadline()
                )
                .await
                .is_err(),
            "unreviewed group mask authorized repair"
        );
        assert!(peer.settings().is_empty());
        peer.low_reads.store(0, Ordering::SeqCst);
        let result = handle
            .change_groups(
                "Bus1.Outputs.Light1",
                Action::Repair {
                    reviewed_mask: 0x8002,
                },
                1,
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        if fault == 0 {
            assert_eq!(result.outcome, "verified");
            assert_eq!(result.drift_mask, Some(0));
            assert_eq!(
                peer.settings(),
                vec![
                    b"t0171\n".to_vec(),
                    b"t017F\n".to_vec(),
                    b"t0160\n".to_vec()
                ],
                "full repair did not remove unmanaged bits before additions"
            );
        } else {
            assert_eq!(
                result.outcome, "unconfirmed",
                "changed reviewed mask was reported verified"
            );
            assert!(
                peer.settings().is_empty(),
                "repair wrote after pre-program mask changed"
            );
        }
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn group_post_save_verification_failure_keeps_desired_intent_and_truthful_report() {
    for fault in [4, 5] {
        let fixture = Fixture::new();
        let mut inventory = fixture.disk();
        inventory.buses[0].groups[0].members.clear();
        fs::write(
            &fixture.paths.current,
            serde_json::to_vec(&inventory).unwrap(),
        )
        .unwrap();
        let peer = Peer::new(0, false, fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .change_groups(
                "Bus1.Outputs.Light1",
                assignment(true),
                1,
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            report.outcome, "unconfirmed",
            "failed group verification was reported successful"
        );
        assert_eq!(report.revision, 2);
        assert!(report.configuration_changed);
        assert_eq!(
            fixture.disk().buses[0].groups[0].members,
            vec![0],
            "failed physical verification discarded durable group intent"
        );
        let probe = report.probe.unwrap();
        if fault == 4 {
            assert_eq!(probe.mask, Some(0));
            assert_eq!(report.drift_mask, Some(1));
        } else {
            assert_eq!(probe.quality, Quality::Partial);
            assert_eq!(probe.mask, None);
            assert_eq!(report.drift_mask, None);
        }
        assert!(report.reason.is_some());
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn group_unknown_repeat_profile_and_bad_authority_refuse_before_probe_or_save() {
    let fixture = Fixture::new();
    let peer = Peer::new(0, false, 0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for action in [assignment(false), Action::Repair { reviewed_mask: 0 }] {
        assert!(
            handle
                .change_groups("Bus1.Outputs.Light1", action, 1, context(1), deadline())
                .await
                .is_err(),
            "unknown repeat policy admitted group programming"
        );
    }
    for (expected, token) in [(0, 1), (1, 2)] {
        assert!(handle
            .change_groups(
                "Bus1.Outputs.Light1",
                Action::Adopt { reviewed_mask: 0 },
                expected,
                context(token),
                deadline()
            )
            .await
            .is_err());
    }
    assert_eq!(
        peer.low_reads.load(Ordering::SeqCst),
        0,
        "refused group request reached physical probing"
    );
    assert!(peer.settings().is_empty());
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_creation_strict_mqtt_and_rename_preserve_fixture_bindings_without_uart() {
    let fixture = Fixture::new();
    fixture.add_button();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let before = handle.snapshot().unwrap().model.projections();
    let valid =
        json!({"command":"create_group","bus":0,"group":15,"name":"Evening","expected_revision":1});
    let mut invalid = Vec::new();
    for (field, value) in [
        ("extra", json!(true)),
        ("bus", json!(4)),
        ("group", json!(16)),
        ("group", json!(-1)),
        ("group", json!(1.5)),
        ("group", json!(0)),
        ("name", json!("")),
        ("target", json!("Bus1.Outputs.Light1")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        invalid.push(body);
    }
    for field in ["bus", "group", "name", "expected_revision"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        invalid.push(body);
    }
    for (i, body) in invalid.into_iter().enumerate() {
        let result = call(&broker, body, 1, &format!("test/group-create-invalid/{i}")).await;
        assert!(
            result.get("error").is_some(),
            "invalid create_group payload was accepted: {result}"
        );
        assert_eq!(fixture.disk().revision, 1);
    }
    let result = call(&broker, valid, 1, "test/group-create").await;
    assert_eq!(
        result["value"]["revision"], 2,
        "saved group creation did not complete with original MQTT context: {result}"
    );
    assert_eq!(fixture.disk().buses[0].groups.len(), 2);
    assert!(fixture.disk().buses[0].groups[1].members.is_empty());
    for projection in &before {
        assert!(
            handle
                .snapshot()
                .unwrap()
                .model
                .projections()
                .contains(projection),
            "group creation changed existing fixture binding"
        );
    }
    let renamed=call(&broker,json!({"command":"rename","target":"Bus1.Groups.Evening","name":"Accent","expected_revision":2}),2,"test/group-rename").await;
    assert_eq!(renamed["value"]["revision"], 3);
    assert!(handle
        .snapshot()
        .unwrap()
        .model
        .projections()
        .iter()
        .any(|p| p.target == "Bus1.Groups.Accent" && p.binding_revision == 3));
    let unchanged=call(&broker,json!({"command":"rename","target":"Bus1.Groups.Accent","name":"Accent","expected_revision":3}),3,"test/group-rename-noop").await;
    assert_eq!(
        unchanged["value"]["revision"], 3,
        "unchanged group rename rotated the binding"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_strict_assignment_review_and_read_payloads_reject_without_physical_queries() {
    let fixture = Fixture::new();
    fixture.add_button();
    let peer = Peer::new(0, false, 0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
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
    let valid = json!({"command":"assign_group","target":"Bus1.Outputs.Light1","group":0,"member":false,"expected_revision":1});
    let mut invalid = Vec::new();
    for (field, value) in [
        ("extra", json!(true)),
        ("group", json!(16)),
        ("group", json!(-1)),
        ("group", json!(1)),
        ("member", json!(1)),
        ("member", json!(null)),
        ("expected_revision", json!(0)),
        ("target", json!("Bus1.Groups.All")),
        ("target", json!("Bus1.Inputs.Panel")),
        ("target", json!("Bus1.Inputs.Panel.Instances.Button")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        invalid.push(body);
    }
    for field in ["target", "group", "member", "expected_revision"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        invalid.push(body);
    }
    for command in ["repair_groups", "adopt_groups"] {
        for reviewed in [json!(-1), json!(65536), json!(1.5), json!(null), json!("0")] {
            invalid.push(json!({"command":command,"target":"Bus1.Outputs.Light1","reviewed_mask":reviewed,"expected_revision":1}));
        }
        invalid
            .push(json!({"command":command,"target":"Bus1.Outputs.Light1","expected_revision":1}));
    }
    invalid.push(json!({"command":"read_groups","target":"Bus1.Outputs.Light1","group":0}));
    for (i, body) in invalid.into_iter().enumerate() {
        let result = call(&broker, body, 1, &format!("test/group-invalid/{i}")).await;
        assert!(
            result.get("error").is_some(),
            "invalid group payload was accepted: {result}"
        );
    }
    assert_eq!(peer.low_reads.load(Ordering::SeqCst), 0);
    assert!(peer.settings().is_empty());
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_save_failure_and_uncertainty_never_program_and_recovery_only_adopts_intent() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let mut inventory = fixture.disk();
        inventory.buses[0].groups[0].members.clear();
        fs::write(
            &fixture.paths.current,
            serde_json::to_vec(&inventory).unwrap(),
        )
        .unwrap();
        let peer = Peer::new(0, false, 0);
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected group save failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .change_groups(
                "Bus1.Outputs.Light1",
                assignment(true),
                1,
                context(1),
                deadline(),
            )
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
            let recovered = recover_when_idle(&handle).await;
            assert_eq!(
                recovered.model.inventory().buses[0].groups[0].members,
                vec![0]
            );
            assert_eq!(recovered.model.inventory().revision, 2);
        }
        assert!(
            peer.settings().is_empty(),
            "unconfirmed group save programmed physical settings"
        );
        assert_eq!(peer.mask.load(Ordering::SeqCst), 0);
        assert!(running.stop().await);
    }
}
async fn recover_when_idle(handle: &ServiceHandle) -> Snapshot {
    let until = deadline();
    tokio::time::timeout_at(until, async {
        loop {
            match handle.recover(context(1), until).await {
                Ok(snapshot) => break snapshot,
                Err(ServiceError::Busy) => tokio::time::sleep(Duration::from_millis(1)).await,
                Err(ServiceError::Uncertain(reason))
                    if reason == "configuration worker is busy" =>
                {
                    tokio::time::sleep(Duration::from_millis(1)).await
                }
                result => panic!("group recovery failed: {result:?}"),
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn group_held_save_keeps_permit_and_refuses_original_candidate_after_bus_cycle() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses[0].groups[0].members.clear();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"fixture","short_address":0,"level":100,"groups":0}]}]})).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::BeforeRename(Stage::Mapping));
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
    let pending = tokio::spawn(async move {
        client
            .change_groups(
                "Bus1.Outputs.Light1",
                assignment(true),
                1,
                context(1),
                Instant::now() + Duration::from_secs(5),
            )
            .await
    });
    hold.entered().await;
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["groups"], 0,
        "group settings preceded durable desired intent"
    );
    assert!(
        matches!(
            handle
                .read_groups("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "save handoff released group permit"
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
        "save continuation substituted fresh physical authority"
    );
    assert_eq!(report.revision, 2);
    assert_eq!(fixture.disk().buses[0].groups[0].members, vec![0]);
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["groups"], 0,
        "stale save continuation programmed group settings"
    );
    assert!(report.probe.is_none());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_canceled_save_freezes_until_explicit_recovery_without_programming() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses[0].groups[0].members.clear();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let peer = Peer::new(0, false, 0);
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .change_groups(
                "Bus1.Outputs.Light1",
                assignment(true),
                1,
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    assert!(peer.settings().is_empty());
    pending.abort();
    let _ = pending.await;
    wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
    hold.release.store(true, Ordering::SeqCst);
    let recovered = recover_when_idle(&handle).await;
    assert_eq!(recovered.model.inventory().revision, 2);
    assert_eq!(
        recovered.model.inventory().buses[0].groups[0].members,
        vec![0]
    );
    assert!(
        peer.settings().is_empty(),
        "recovery resumed canceled group programming"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_adoption_confirmed_save_keeps_metadata_and_marks_changed_bus_evidence_stale() {
    let fixture = Fixture::new();
    let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"fixture","short_address":0,"groups":0}]}]})).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
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
    let pending = tokio::spawn(async move {
        client
            .change_groups(
                "Bus1.Outputs.Light1",
                Action::Adopt { reviewed_mask: 0 },
                1,
                context(1),
                Instant::now() + Duration::from_secs(5),
            )
            .await
    });
    hold.entered().await;
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
    assert_eq!(report.outcome, "adopted");
    assert_eq!(report.revision, 2);
    assert!(report.configuration_changed);
    assert!(
        report.probe.unwrap().stale,
        "adoption presented pre-save bus evidence as current after epoch change"
    );
    assert!(fixture.disk().buses[0].groups[0].members.is_empty());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_partial_physical_repair_failure_does_not_rollback_or_claim_verification() {
    let fixture = Fixture::new();
    let peer = Peer::new(0x8002, false, 7);
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
        .change_groups(
            "Bus1.Outputs.Light1",
            Action::Repair {
                reviewed_mask: 0x8002,
            },
            1,
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        result.outcome, "unconfirmed",
        "partial group programming claimed verification"
    );
    assert_eq!(result.revision, 1);
    assert!(!result.configuration_changed);
    assert!(
        result.probe.is_none(),
        "preparation probe masqueraded as final group verification"
    );
    assert_eq!(
        peer.mask.load(Ordering::SeqCst),
        0x8000,
        "partial physical group repair was rolled back or continued"
    );
    assert_eq!(peer.settings().len(), 2);
    assert_eq!(fixture.disk().buses[0].groups[0].members, vec![0]);
    assert!(
        matches!(handle.snapshot().unwrap().hardware, Hardware::Ready { .. }),
        "correlated DALI setting collision unnecessarily reset the UART"
    );
    let captured = handle
        .read_groups("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    assert_eq!(captured.probe.unwrap().mask, Some(0x8000));
    assert_eq!(
        peer.settings().len(),
        2,
        "inspection retried failed group settings"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_canceled_and_expired_work_keeps_native_input_and_blocks_new_physical_queries() {
    for (fault, expire) in [(8, false), (9, false), (8, true), (9, true)] {
        let fixture = Fixture::new();
        fixture.add_button();
        let peer = Peer::new(0, false, fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let mut service = service.with_uart(peer.path.clone(), RepeatPolicy::OneTerminal);
        // Exercise normal paced recovery within the fixture's two-second bound.
        service.uart.as_mut().unwrap().timing.retry = Duration::from_millis(20);
        let running = start(service, &handle, &broker).await;
        let target = "Bus1.Inputs.Panel.Instances.Button";
        wait_for(&handle, |s| s.input_listeners.contains(target)).await;
        let client = handle.clone();
        let pending = tokio::spawn(async move {
            client
                .change_groups(
                    "Bus1.Outputs.Light1",
                    assignment(true),
                    1,
                    context(1),
                    Instant::now()
                        + if expire {
                            Duration::from_millis(800)
                        } else {
                            Duration::from_secs(5)
                        },
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
            .await
            .unwrap();
        assert!(
            broker
                .wait_until(Duration::from_secs(1), |b| b
                    .received_on("DALI/Event/Controller")
                    .iter()
                    .any(|r| {
                        let event: serde_json::Value = serde_json::from_slice(&r.payload).unwrap();
                        event["target"] == target && event["action"] == "pressed"
                    }))
                .await,
            "group transaction blocked native input action"
        );
        if expire {
            assert!(
                matches!(pending.await.unwrap(), Err(ServiceError::Uncertain(_))),
                "group operation renewed its caller deadline"
            );
        } else {
            pending.abort();
            let _ = pending.await;
        }
        let blocked = handle
            .read_groups("Bus1.Outputs.Light1", context(1), deadline())
            .await;
        if expire {
            assert!(
                matches!(blocked, Err(ServiceError::Busy))
                    || matches!(blocked,Err(ServiceError::Rejected(ref reason)) if reason=="UART or bus is not ready"),
                "expired group work admitted a new query before safe recovery: {blocked:?}"
            );
        } else {
            assert!(
                matches!(blocked, Err(ServiceError::Busy)),
                "canceled group operation released permit before request settlement"
            );
        }
        peer.release.notify_one();
        let until = deadline();
        let observed = tokio::time::timeout_at(until, async {
            loop {
                match handle
                    .read_groups("Bus1.Outputs.Light1", context(1), until)
                    .await
                {
                    Ok(report) => break report,
                    Err(ServiceError::Busy) => tokio::time::sleep(Duration::from_millis(1)).await,
                    Err(ServiceError::Rejected(reason))
                        if expire && reason == "UART or bus is not ready" =>
                    {
                        tokio::time::sleep(Duration::from_millis(1)).await
                    }
                    result => panic!("group operation failed to settle {result:?}"),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            observed.probe.unwrap().mask,
            Some(if fault == 9 { 1 } else { 0 })
        );
        assert_eq!(
            peer.settings().len(),
            if fault == 9 { 1 } else { 0 },
            "inspection retried canceled group settings"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
    }
}
