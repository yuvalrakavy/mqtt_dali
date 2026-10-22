use super::*;
use crate::service::commissioning::AssignmentOutcome;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Default)]
struct Options {
    random: u32,
    exact_fault: u8,
    fail_status: bool,
    hold_compare: bool,
    pair_delays: [u64; 2],
}
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
    /// Literal Part 103 implementation, independent of bridge and emulator codecs.
    fn start(mut options: Options) -> Self {
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
            let mut search = 0xffffff;
            let mut initialized = false;
            let mut address = None;
            let mut init_count = 0;
            let mut random_count = 0;
            let mut ready = Instant::now();
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                recorded.lock().unwrap().push(line.clone());
                let response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    b"h01A0\n" => b"J64\n".to_vec(),
                    b"h0190\n" => b"J04\n".to_vec(),
                    command if command.len() == 8 && command[0] == b'l' => {
                        let raw =
                            u32::from_str_radix(std::str::from_utf8(&command[1..7]).unwrap(), 16)
                                .unwrap();
                        let mut literal = None;
                        let value = match raw {
                            0xc10000 => {
                                initialized = false;
                                None
                            }
                            0xc1017f => {
                                let delay = options.pair_delays[init_count.min(1)];
                                init_count += 1;
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                                initialized = init_count == 2;
                                None
                            }
                            0xc10200 => {
                                random_count += 1;
                                if random_count == 2 {
                                    ready = Instant::now() + Duration::from_millis(200);
                                }
                                None
                            }
                            0xc10300 => {
                                if options.hold_compare {
                                    options.hold_compare = false;
                                    if write.write_all(b"L008000\n").await.is_err() {
                                        return;
                                    }
                                    signal.notify_one();
                                    resume.notified().await;
                                }
                                if initialized
                                    && random_count == 2
                                    && Instant::now() >= ready
                                    && search >= options.random
                                {
                                    if search == options.random && options.exact_fault != 0 {
                                        literal = Some(match options.exact_fault {
                                            1 => b"X\n".as_slice(),
                                            2 => b"Z\n",
                                            _ => b"J01\n",
                                        });
                                    } else if search != options.random {
                                        // A range collision is positive evidence; exact collision is not.
                                        literal = Some(b"X\n");
                                    }
                                    Some(255)
                                } else {
                                    None
                                }
                            }
                            0xc10a00 => {
                                if initialized && search == options.random {
                                    Some(address.unwrap_or(255))
                                } else {
                                    None
                                }
                            }
                            0xc10400 => None,
                            frame if frame >> 8 == 0xc105 => {
                                search = (search & 0xffff) | ((frame & 255) << 16);
                                None
                            }
                            frame if frame >> 8 == 0xc106 => {
                                search = (search & 0xff00ff) | ((frame & 255) << 8);
                                None
                            }
                            frame if frame >> 8 == 0xc107 => {
                                search = (search & 0xffff00) | (frame & 255);
                                None
                            }
                            frame if frame >> 8 == 0xc108 => {
                                assert!(initialized && search == options.random);
                                assert!(
                                    frame & 255 <= 63,
                                    "PROGRAM did not use raw input short address"
                                );
                                address = Some((frame & 255) as u8);
                                None
                            }
                            frame if frame >> 8 == 0xc109 => {
                                if address == Some((frame & 255) as u8) {
                                    Some(255)
                                } else {
                                    None
                                }
                            }
                            frame if frame & 0xffff == 0xfe30 => {
                                if address == Some(((frame >> 16) as u8) >> 1)
                                    && !options.fail_status
                                {
                                    Some(0)
                                } else {
                                    None
                                }
                            }
                            _ => panic!("unexpected input assignment frame {raw:06X}"),
                        };
                        literal.map_or_else(
                            || {
                                value.map_or_else(
                                    || b"N\n".to_vec(),
                                    |v| format!("J{v:02X}\n").into_bytes(),
                                )
                            },
                            |v| v.to_vec(),
                        )
                    }
                    other => panic!("unexpected input assignment wire command {other:?}"),
                };
                if write.write_all(&response).await.is_err() {
                    return;
                }
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
    fn programmed(&self) -> bool {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with(b"lC108"))
    }
}
async fn assign(
    handle: &ServiceHandle,
) -> Result<crate::service::commissioning::Report, ServiceError> {
    handle
        .assign_next_input(0, 1, context(1), Instant::now() + Duration::from_secs(5))
        .await
}

#[tokio::test]
async fn input_assignment_literal_boundaries_raw_addresses_and_range_collisions() {
    for (random, destination) in [
        (0, 0),
        (1, 63),
        (0x7fffff, 0),
        (0x800000, 63),
        (0xfffffe, 0),
        (0xffffff, 63),
    ] {
        let fixture = Fixture::new();
        if destination == 63 {
            let mut inventory = fixture.disk();
            inventory.buses[0].inputs = (0..63)
                .map(|a| {
                    serde_json::from_value(
                        json!({"short_address":a,"name":format!("Saved{a}"),"instances":[]}),
                    )
                    .unwrap()
                })
                .collect();
            fs::write(
                &fixture.paths.current,
                serde_json::to_vec(&inventory).unwrap(),
            )
            .unwrap();
        }
        let peer = Peer::start(Options {
            random,
            ..Default::default()
        });
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = assign(&handle).await;
        assert!(
            report.as_ref().is_ok_and(
                |r| r.outcome == AssignmentOutcome::Assigned && r.address == Some(destination)
            ),
            "input inclusive search or raw address verification failed: {report:?}"
        );
        let commands = peer.commands.lock().unwrap().clone();
        for command in [b"lC1017F\n".as_slice(), b"lC10200\n"] {
            assert_eq!(
                commands.iter().filter(|c| c.as_slice() == command).count(),
                2,
                "input settings did not use exactly two ordinary l exchanges"
            );
        }
        assert!(
            commands.contains(&format!("lC108{destination:02X}\n").into_bytes()),
            "input PROGRAM used shifted output encoding"
        );
        assert!(
            commands.contains(&format!("lC109{destination:02X}\n").into_bytes()),
            "input VERIFY used shifted output encoding"
        );
        assert!(!commands
            .iter()
            .any(|c| c.starts_with(b"hB7") || c.starts_with(b"t")));
        assert_eq!(fixture.disk().buses[0].outputs[0].short_address, 0);
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn input_assignment_exact_faults_never_reserve_or_program() {
    for fault in 1..=3 {
        let fixture = Fixture::new();
        let peer = Peer::start(Options {
            random: 0x800000,
            exact_fault: fault,
            ..Default::default()
        });
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
            assign(&handle).await.is_err(),
            "ambiguous exact input candidate was reserved"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(
            !peer.programmed(),
            "ambiguous exact input candidate reached PROGRAM"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn input_assignment_pair_uses_one_deadline_without_retry_or_save() {
    for (delays, max_sent) in [([120, 0], 1), ([70, 40], 2)] {
        let fixture = Fixture::new();
        let peer = Peer::start(Options {
            pair_delays: delays,
            ..Default::default()
        });
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
            assign(&handle).await.is_err(),
            "input pair renewed the 100 ms deadline"
        );
        assert_eq!(
            fixture.disk().revision,
            1,
            "late input pair authorized a save"
        );
        let commands = peer.commands.lock().unwrap().clone();
        assert_eq!(
            commands
                .iter()
                .filter(|c| c.as_slice() == b"lC1017F\n")
                .count(),
            max_sent,
            "input pair retried after its shared deadline"
        );
        assert!(!peer.programmed());
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn input_assignment_fresh_status_failure_retains_empty_module_reservation() {
    let fixture = Fixture::new();
    let peer = Peer::start(Options {
        fail_status: true,
        ..Default::default()
    });
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let report = assign(&handle).await.unwrap();
    assert_eq!(
        report.outcome,
        AssignmentOutcome::ReservedButUnconfirmed,
        "input final status failure claimed assignment success"
    );
    assert_eq!(report.revision, 2);
    assert!(report.reason.unwrap().contains("fresh status"));
    assert_eq!(fixture.disk().buses[0].inputs.len(), 1);
    assert!(fixture.disk().buses[0].inputs[0].instances.is_empty());
    assert!(running.stop().await);
}

#[tokio::test]
async fn input_assignment_cancelled_search_keeps_events_and_permit_until_settlement() {
    let fixture = Fixture::new();
    fixture.add_button();
    let peer = Peer::start(Options {
        hold_compare: true,
        ..Default::default()
    });
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
        "input search released ownership before settlement"
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
        "cancelled input search saved a reservation"
    );
    assert!(!peer.programmed());
    assert!(running.stop().await);
}
