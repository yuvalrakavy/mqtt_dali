//! Literal addressed-write peer, independent of the bridge and emulator codecs.
use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn individual_clear_literal_requires_presence_addressed_repetition_and_final_absence() {
    for input in [false, true] {
        for two in [false, true] {
            for fault in 0..3 {
                let fixture = Fixture::new();
                fixture.add_button();
                let mut pty = crate::service::hat::tests::Pty::new();
                let path = pty.path.clone();
                let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
                let recorded = commands.clone();
                let peer = tokio::spawn(async move {
                    let (read, mut write) = tokio::io::split(&mut pty.peer);
                    let mut read = BufReader::new(read);
                    let mut line = Vec::new();
                    let mut clears = 0;
                    loop {
                        if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                            return;
                        }
                        recorded.lock().unwrap().push(line.clone());
                        let response:&[u8]=match line.as_slice(){
                            b"v\n"=>b"V013204\n",b"d\n"=>b"D24\n",
                            b"h0190\n"|b"l01FE30\n"=>if fault==1||(clears>0&&fault!=2){b"N\n"}else{b"J00\n"},
                            b"h01A0\n"=>b"J64\n",b"l01FE35\n"=>b"J01\n",b"l010080\n"=>b"J01\n",
                            b"hA3FF\n" if !input=>b"N\n",
                            b"lC130FF\n" if input=>b"N\n",
                            b"t0180\n" if !input=>{clears+=1;if two {b"N\nN\n"}else{b"N\n"}},
                            b"l01FE14\n" if input=>{clears+=1;b"N\n"},
                            other=>panic!("individual clear sent broadcast, commissioning or unexpected frame: {other:?}"),
                        };
                        if write.write_all(response).await.is_err() {
                            return;
                        }
                        line.clear();
                    }
                });
                let broker = FakeBroker::start().await;
                let (service, handle) = fixture.service();
                let running = start(
                    service.with_uart(
                        path,
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
                let report = handle
                    .clear_short_address(target(input), 1, context(1), limit())
                    .await
                    .unwrap();
                assert_eq!(
                    report.outcome,
                    if fault == 0 {
                        ResultKind::Cleared
                    } else {
                        ResultKind::Unconfirmed
                    },
                    "individual clear accepted missing presence or absent final proof"
                );
                let writes = commands
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|c| matches!(c.as_slice(), b"t0180\n" | b"l01FE14\n"))
                    .count();
                assert_eq!(
                    writes,
                    if fault == 1 {
                        0
                    } else if input {
                        2
                    } else {
                        1
                    },
                    "individual clear lost its addressed repeat or wrote without presence"
                );
                assert!(running.stop().await);
                peer.abort();
                let _ = peer.await;
            }
        }
    }
}
