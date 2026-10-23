use super::*;
use crate::dali::gear::Intent;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
const TARGET: &str = "Bus2.Groups.Last";
struct Peer {
    path: PathBuf,
    commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    fn new(fault: u8) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let level = Arc::new(AtomicU8::new(100));
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                recorded.lock().unwrap().push(line.clone());
                let response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"1d\n" => b"1D24\n".to_vec(),
                    b"1h7FC0\n" => {
                        if fault == 9 {
                            b"1N\n".to_vec()
                        } else {
                            b"1J00\n".to_vec()
                        }
                    }
                    b"1h7FC1\n" => match fault {
                        1 | 9 => b"1N\n".to_vec(),
                        2 => b"1X\n".to_vec(),
                        10 => b"1J00\n".to_vec(),
                        _ => b"1J80\n".to_vec(),
                    },
                    b"1h7FA0\n" => format!(
                        "1J{:02X}\n",
                        if fault == 3 {
                            255
                        } else {
                            level.load(Ordering::SeqCst)
                        }
                    )
                    .into_bytes(),
                    b"1h7F90\n" => {
                        if fault == 4 {
                            b"1N\n".to_vec()
                        } else {
                            b"1JFF\n".to_vec()
                        }
                    }
                    cmd @ (b"1h9F00\n" | b"1h9F05\n" | b"1h9E7F\n") => {
                        if fault == 6 {
                            b"1X\n".to_vec()
                        } else if fault == 7 {
                            b"1JFF\n".to_vec()
                        } else {
                            if fault != 5 {
                                level.store(
                                    if cmd == b"1h9F00\n" {
                                        0
                                    } else if cmd == b"1h9F05\n" {
                                        254
                                    } else {
                                        127
                                    },
                                    Ordering::SeqCst,
                                );
                            }
                            b"1N\n".to_vec()
                        }
                    }
                    other => panic!("unexpected native group command {other:?}"),
                };
                write.write_all(&response).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            commands,
            task,
        }
    }
    fn mutations(&self) -> Vec<Vec<u8>> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.starts_with(b"1h9"))
            .cloned()
            .collect()
    }
}
fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    let bus = &mut inventory.buses[0];
    bus.bus = 1;
    bus.name = "Bus2".into();
    bus.outputs[0].short_address = 63;
    bus.groups[0].group_address = 15;
    bus.groups[0].name = "Last".into();
    bus.groups[0].members = vec![63];
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    fixture
}
#[tokio::test]
async fn group_output_literal_boundaries_missing_collided_mask_and_ignored_evidence() {
    for (fault, quality, power) in [
        (0, "uniform", Some(true)),
        (1, "partial", None),
        (2, "partial", None),
        (3, "partial", None),
        (4, "partial", None),
        (9, "unavailable", None),
        (10, "empty", None),
    ] {
        let fixture = fixture();
        let peer = Peer::new(fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .read_group_output(
                TARGET,
                RequestContext::default().with_traceparent(None),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            (report.bus, report.group, report.members[0].address),
            (1, 15, 63)
        );
        assert_eq!(report.target_generation, context(1).generation().unwrap());
        let values = report.values.unwrap();
        assert_eq!(
            (values.quality, values.power_state),
            (quality, power),
            "group raw evidence became usable: fault {fault}"
        );
        assert!(peer.mutations().is_empty(), "group read sent an action");
        assert_eq!(
            report.membership_matches,
            if fault == 1 || fault == 2 || fault == 9 {
                None
            } else {
                Some(fault != 10)
            }
        );
        if fault != 0 {
            assert_eq!(values.brightness, None);
        }
        assert!(running.stop().await);
    }
    for fault in [0, 5, 6, 7] {
        let fixture = fixture();
        let peer = Peer::new(fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .set_group_output(
                TARGET,
                1,
                Intent::new(Some(false), None).unwrap(),
                context(1),
                deadline(),
            )
            .await;
        assert_eq!(
            peer.mutations(),
            [b"1h9F00\n".to_vec()],
            "native group action was not single or used the wrong destination"
        );
        let r = result.unwrap();
        assert_eq!(r.outcome, if fault >= 6 { "unconfirmed" } else { "sent" });
        if fault >= 6 {
            assert!(r.values.is_none());
        } else {
            assert_eq!(
                r.values.unwrap().power_state,
                Some(fault == 5),
                "group report echoed requested Off instead of actual"
            );
        }
        assert!(running.stop().await);
    }
}
#[tokio::test]
async fn group_output_literal_power_and_dimming_are_native_single_sends() {
    let fixture = fixture();
    let peer = Peer::new(0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for (power, brightness, expected) in [
        (Some(true), None, 254),
        (None, Some(0.5), 127),
        (Some(false), None, 0),
    ] {
        let r = handle
            .set_group_output(
                TARGET,
                1,
                Intent::new(power, brightness).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(r.outcome, "sent");
        assert_eq!(
            r.values.unwrap().brightness,
            Some(f64::from(expected) / 254.0)
        );
    }
    assert_eq!(
        peer.mutations(),
        [
            b"1h9F05\n".to_vec(),
            b"1h9E7F\n".to_vec(),
            b"1h9F00\n".to_vec()
        ]
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn group_output_strict_authority_parser_and_disabled_bus_send_no_actions() {
    let fixture = fixture();
    let peer = Peer::new(0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let intent = Intent::new(Some(true), None).unwrap();
    for (target, expected, cause) in [
        (TARGET, 0, context(1)),
        (TARGET, 1, context(2)),
        (TARGET, 1, RequestContext::default()),
        ("Bus2.Outputs.Light1", 1, context(1)),
        ("missing", 1, context(1)),
    ] {
        assert!(
            matches!(
                handle
                    .set_group_output(target, expected, intent, cause, deadline())
                    .await,
                Err(ServiceError::Rejected(_))
            ),
            "invalid group authority reached hardware"
        );
    }
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (index, params) in [
        json!({"expected_revision":1}),
        json!({"expected_revision":1,"power":null}),
        json!({"expected_revision":1,"power":1}),
        json!({"expected_revision":1,"power":"true"}),
        json!({"expected_revision":1,"brightness":null}),
        json!({"expected_revision":1,"brightness":-0.1}),
        json!({"expected_revision":1,"brightness":1.1}),
        json!({"expected_revision":1,"power":true,"brightness":0}),
        json!({"expected_revision":1,"power":false,"brightness":0.5}),
        json!({"expected_revision":1,"power":true,"unknown":1}),
        json!({"power":true}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut call = json!({"command":"set_group_output","target":TARGET});
        call.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
        let reply = groups::call(
            &broker,
            call,
            1,
            &format!("test/group-output/invalid/{index}"),
        )
        .await;
        assert!(
            reply.get("error").is_some(),
            "invalid group parameters accepted: {reply}"
        );
    }
    let reply = groups::call(
        &broker,
        json!({"command":"read_group_output","target":TARGET,"power":true}),
        1,
        "test/group-output/invalid-read",
    )
    .await;
    assert!(reply.get("error").is_some());
    assert!(
        peer.mutations().is_empty(),
        "invalid group requests sent an action"
    );
    assert!(running.stop().await);
    // A new disabled saved-bus service must refuse before trying that address space.
    let mut inventory = fixture.disk();
    inventory.buses[0].enabled = false;
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let emulator =
        pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0}]})).unwrap())
            .await
            .unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| s.phase == Phase::Ready).await;
    assert!(
        matches!(handle.read_group_output(TARGET,context(1),deadline()).await,Err(ServiceError::Rejected(ref e))if e.contains("disabled"))
    );
    assert!(
        matches!(handle.set_group_output(TARGET,1,intent,context(1),deadline()).await,Err(ServiceError::Rejected(ref e))if e.contains("disabled"))
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
