use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
    fn new(handle: ServiceHandle, path: PathBuf, input: bool, two: bool, failure: u8) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let device = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            let mut first = true;
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                assert!(line.len() < 32);
                recorded.lock().unwrap().push(line.clone());
                let mut response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    command => {
                        assert!(matches!(command[0], b'h' | b't' | b'l'));
                        let dtr = command == b"hA3FF\n" || command == b"lC130FF\n";
                        if dtr && first {
                            first = false;
                            let disk: Inventory =
                                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                            let s = handle.snapshot().unwrap();
                            assert_eq!(
                                disk.revision, 2,
                                "address clear preceded durable retirement"
                            );
                            assert_eq!(
                                s.phase,
                                Phase::Ready,
                                "address clear preceded Ready activation"
                            );
                            assert_eq!(s.model.inventory().revision, 2);
                            assert!(
                                if input {
                                    disk.buses[0].inputs.is_empty()
                                } else {
                                    disk.buses[0].outputs.is_empty()
                                },
                                "old saved identities remained during clear"
                            );
                            assert_eq!(
                                routing::generations(&s.model).instance,
                                context(2).generation().unwrap()
                            );
                        }
                        let setting = command == b"tFF80\n" || command == b"lFFFE14\n";
                        if setting && failure == 1 {
                            b"X\n".to_vec()
                        } else if failure == 2
                            && ((!input && command == b"h0990\n")
                                || (input && command == b"l09FE30\n"))
                        {
                            b"J00\n".to_vec()
                        } else {
                            b"N\n".to_vec()
                        }
                    }
                };
                if two && line[0] == b't' {
                    response.extend_from_slice(b"N\n");
                }
                if write.write_all(&response).await.is_err() {
                    return;
                }
                line.clear();
            }
        });
        Self {
            path: device,
            commands,
            task,
        }
    }
}

#[tokio::test]
async fn reassignment_literal_clear_follows_activation_and_requires_all_status_absences() {
    for input in [false, true] {
        for failure in [0, 1, 2] {
            for two in [false, true] {
                let fixture = Fixture::new();
                fixture.add_button();
                let (service, handle) = fixture.service();
                let peer = Peer::new(
                    handle.clone(),
                    fixture.paths.current.clone(),
                    input,
                    two,
                    failure,
                );
                let broker = FakeBroker::start().await;
                let running = start(
                    service.with_uart(
                        &peer.path,
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
                let report = handle
                    .reset_address_space(
                        0,
                        if input { Space::Inputs } else { Space::Outputs },
                        1,
                        context(1),
                        limit(),
                    )
                    .await
                    .unwrap();
                assert_eq!(report.revision, 2);
                assert_eq!(
                    report.outcome,
                    if failure == 0 {
                        ResetOutcome::Cleared
                    } else {
                        ResetOutcome::RetiredButUnconfirmed
                    },
                    "incomplete status proof claimed address clearing"
                );
                assert_eq!(
                    report.checked_addresses,
                    match failure {
                        0 => 64,
                        1 => 0,
                        _ => 5,
                    },
                    "incomplete status proof was reported as complete"
                );
                let commands = peer.commands.lock().unwrap().clone();
                let native: Vec<_> = commands
                    .iter()
                    .filter(|c| matches!(c[0], b'h' | b't' | b'l'))
                    .map(Vec::as_slice)
                    .collect();
                if input {
                    assert_eq!(native[0], b"lC130FF\n");
                    assert_eq!(native[1], b"lFFFE14\n");
                    if failure != 1 {
                        assert_eq!(native[2], b"lFFFE14\n");
                    }
                } else {
                    assert_eq!(native[0], b"hA3FF\n");
                    assert_eq!(native[1], b"tFF80\n");
                }
                assert!(report.reason.is_some() == (failure != 0));
                assert!(running.stop().await);
            }
        }
    }
}
