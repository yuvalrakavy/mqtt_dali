//! Fixed query-only UART peer; no production codec/model dependencies.
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
    fn new(fault: u8) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
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
                let reply:&[u8]=match line.as_slice(){
                    b"v\n"=>b"V013204\n",b"d\n"=>b"D24\n",
                    b"h0190\n"|b"l01FE30\n"=>if fault==1{b"J00\n"}else{b"N\n"},
                    b"h0390\n"|b"l03FE30\n"=>if fault==2{b"N\n"}else{b"J00\n"},
                    b"l03FE35\n"=>if fault==3{b"J21\n"}else{b"J01\n"},
                    b"l030080\n"=>if fault==4{b"N\n"}else{b"J01\n"},
                    other=>panic!("identified recovery sent a native address write or unexpected query: {other:?}"),
                };
                if write.write_all(reply).await.is_err() {
                    return;
                }
                line.clear();
            }
        });
        Self {
            path,
            commands,
            task,
        }
    }
}

#[tokio::test]
async fn restoration_identified_literal_only_queries_then_rebinds_saved_identity() {
    for input in [false, true] {
        for repeat in [
            RepeatPolicy::Unverified,
            RepeatPolicy::OneTerminal,
            RepeatPolicy::TwoTerminals,
        ] {
            for fault in 0..if input { 6 } else { 3 } {
                let fixture = Fixture::new();
                if input {
                    fixture.add_button();
                    let mut disk = fixture.disk();
                    let mut candidate = disk.buses[0].inputs[0].clone();
                    candidate.short_address = 1;
                    candidate.name = "Candidate".into();
                    if fault == 5 {
                        disk.buses[0].inputs[0].instances[0].number = 31;
                    }
                    disk.buses[0].inputs.push(candidate);
                    fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
                } else {
                    fixture.add_output(1, "Candidate");
                }
                let peer = Peer::new(fault);
                let broker = FakeBroker::start().await;
                let (service, handle) = fixture.service();
                let running = start(service.with_uart(&peer.path, repeat), &handle, &broker).await;
                wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
                let before = handle.snapshot().unwrap().model;
                let result = handle
                    .restore_address(
                        target(input),
                        Action::Identified {
                            candidate: format!(
                                "Bus1.{}.Candidate",
                                if input { "Inputs" } else { "Outputs" }
                            ),
                            candidate_generation: context(1).generation().unwrap().into(),
                            expected_revision: 1,
                        },
                        context(1),
                        limit(),
                    )
                    .await;
                if fault == 0 {
                    let report = result.unwrap();
                    assert_eq!(
                        report.outcome,
                        ResultKind::Restored,
                        "query-only recovery did not adopt the identified address"
                    );
                    assert_eq!(
                        (report.previous_address, report.address, report.revision),
                        (0, 1, 2)
                    );
                    assert_eq!(report.current_generation, context(2).generation().unwrap());
                    let after = handle.snapshot().unwrap().model;
                    assert_eq!(
                        after.origin(location(input, 1)),
                        before.origin(location(input, 0)),
                        "address adoption replaced the original saved identity"
                    );
                    assert!(
                        after.origin(location(input, 0)).is_none(),
                        "previous address stayed reserved after adoption"
                    );
                } else {
                    assert!(
                        result.is_err(),
                        "missing/occupied/incompatible candidate was adopted"
                    );
                    assert_eq!(fixture.disk().revision, 1);
                }
                assert!(running.stop().await);
                assert!(
                    peer.commands.lock().unwrap().iter().all(|c| matches!(
                        c.as_slice(),
                        b"v\n"
                            | b"d\n"
                            | b"h0190\n"
                            | b"h0390\n"
                            | b"l01FE30\n"
                            | b"l03FE30\n"
                            | b"l03FE35\n"
                            | b"l030080\n"
                    )),
                    "identified recovery programmed hardware instead of adopting its address"
                );
            }
        }
    }
}
