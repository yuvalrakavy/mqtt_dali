//! Exact ATX script, independent of production codecs and the relay model.
use super::*;
use std::{collections::VecDeque, sync::Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Peer {
    path: PathBuf,
    pending: Arc<Mutex<VecDeque<(String, String)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    fn new(script: Vec<(String, String)>) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let pending = Arc::new(Mutex::new(VecDeque::from(script)));
        let remaining = pending.clone();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            loop {
                let mut line = String::new();
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let reply = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        let (expected, reply) = remaining
                            .lock()
                            .unwrap()
                            .pop_front()
                            .expect("unexpected DT7 capture wire command");
                        assert_eq!(line, expected, "DT7 capture wire plan mismatch");
                        reply
                    }
                };
                write.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        Self {
            path,
            pending,
            task,
        }
    }
}
fn probe(first: &str, next: &[&str]) -> Vec<(String, String)> {
    let mut script = vec![
        ("h01A0\n".into(), "JFE\n".into()),
        ("h0190\n".into(), "J04\n".into()),
        ("h0199\n".into(), first.into()),
    ];
    script.extend(next.iter().map(|reply| ("h01A7\n".into(), (*reply).into())));
    script
}
fn virtual_read(script: &mut Vec<(String, String)>, two: bool, reply: &str) {
    script.extend([
        ("hA3FF\n".into(), "N\n".into()),
        ("t0121\n".into(), if two { "N\nN\n" } else { "N\n" }.into()),
        ("h0198\n".into(), reply.into()),
    ]);
}
fn stored(script: &mut Vec<(String, String)>) {
    script.push(("h01B0\n".into(), "J2A\n".into()));
}
async fn capture(
    script: Vec<(String, String)>,
    policy: RepeatPolicy,
    stale: bool,
) -> Result<crate::service::scene_settings::Report, ServiceError> {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let original = fs::read(&fixture.paths.current).unwrap();
    let peer = Peer::new(script);
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    if stale {
        service.output_acceptance_hook =
            Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
    }
    let running = start(service.with_uart(&peer.path, policy), &handle, &broker).await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .change_scene_row(
            OUTPUT,
            Action::Capture {
                slot: crate::dali::scenes::Slot::new(0).unwrap(),
            },
            context(1),
            deadline(),
        )
        .await;
    assert!(
        !peer.task.is_finished(),
        "DT7 capture peer saw unexpected native traffic"
    );
    assert!(
        peer.pending.lock().unwrap().is_empty(),
        "DT7 capture skipped required native wire steps"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert!(
        handle.snapshot().unwrap().outputs.is_empty(),
        "capture manufactured public output State"
    );
    assert!(running.stop().await);
    result
}

#[tokio::test]
async fn dt7_scene_capture_literal_exact_plan_and_missing_virtual_evidence() {
    for two in [false, true] {
        for (reply, expected) in [
            ("J64\n", Some(100)),
            ("J00\n", Some(0)),
            ("JFE\n", Some(254)),
            ("N\n", None),
            ("X4\n", None),
            ("Z8\n", None),
            ("JFF\n", None),
        ] {
            let mut script = probe("J07\n", &[]);
            virtual_read(&mut script, two, reply);
            stored(&mut script);
            let result = capture(
                script,
                if two {
                    RepeatPolicy::TwoTerminals
                } else {
                    RepeatPolicy::OneTerminal
                },
                false,
            )
            .await
            .unwrap();
            assert_eq!(
                serde_json::to_value(result.candidate).unwrap(),
                expected.map(base_setting).unwrap_or(json!(null)),
                "DT7 virtual capture fabricated or lost correlated DTR0 level"
            );
            if expected.is_none() {
                assert!(result.reason.is_some());
            }
        }
    }
}

#[tokio::test]
async fn dt7_scene_capture_literal_type_proofs_and_unverified_ordinary_compatibility() {
    for (first, next, expected, relay) in [
        ("J06\n", vec![], Some(254), false),
        ("JFE\n", vec![], Some(254), false),
        ("N\n", vec![], None, false),
        ("X4\n", vec![], None, false),
        ("Z8\n", vec![], None, false),
        ("JFF\n", vec!["JFE\n"], None, false),
        ("JFF\n", vec!["J06\n", "JFE\n"], None, false),
        ("JFF\n", vec!["J00\n", "J06\n", "JFE\n"], Some(254), false),
        ("JFF\n", vec!["J06\n", "J06\n"], None, false),
        ("JFF\n", vec!["J06\n", "J05\n"], None, false),
        ("JFF\n", vec!["J00\n", "N\n"], None, false),
        (
            "JFF\n",
            vec!["J00\n", "J06\n", "J07\n", "J09\n"],
            Some(100),
            true,
        ),
        // DT7 is proved, but a failed later DT8 query still prevents a colour-safe candidate.
        ("JFF\n", vec!["J06\n", "J07\n", "N\n"], None, true),
        ("JFF\n", vec!["J09\n"], Some(254), false),
    ] {
        let mut script = probe(first, &next);
        if relay {
            virtual_read(&mut script, false, "J64\n");
        }
        stored(&mut script);
        let result = capture(
            script,
            if relay {
                RepeatPolicy::OneTerminal
            } else {
                RepeatPolicy::Unverified
            },
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(result.candidate).unwrap(),
            expected.map(base_setting).unwrap_or(json!(null)),
            "DT7 capture type proof misclassified unknown or ordinary fixture"
        );
    }
}

#[tokio::test]
async fn dt7_scene_capture_literal_send_failures_and_unverified_repeat_stop_plan() {
    for (poison, policy) in [
        ("X4\n", RepeatPolicy::OneTerminal),
        ("N\n", RepeatPolicy::Unverified),
    ] {
        let mut script = probe("J07\n", &[]);
        script.push(("hA3FF\n".into(), poison.into()));
        assert!(
            capture(script, policy, false).await.is_err(),
            "DT7 capture continued after failed scratch write or unverified repeat"
        );
    }
    let mut script = probe("J07\n", &[]);
    script.extend([
        ("hA3FF\n".into(), "N\n".into()),
        ("t0121\n".into(), "Z8\n".into()),
    ]);
    assert!(
        capture(script, RepeatPolicy::OneTerminal, false)
            .await
            .is_err(),
        "DT7 capture continued after collided repeated STORE"
    );
}

#[tokio::test]
async fn dt7_scene_capture_literal_final_stamp_rejects_candidate() {
    let mut script = probe("J07\n", &[]);
    virtual_read(&mut script, false, "J64\n");
    stored(&mut script);
    assert!(
        capture(script, RepeatPolicy::OneTerminal, true)
            .await
            .is_err(),
        "DT7 capture accepted candidate with retired physical stamp"
    );
}

#[tokio::test]
async fn dt7_scene_capture_literal_partial_base_never_supplies_virtual_candidate() {
    for index in [0, 1] {
        let mut script = probe("J07\n", &[]);
        script[index].1 = "N\n".into();
        stored(&mut script);
        let report = capture(script, RepeatPolicy::OneTerminal, false)
            .await
            .unwrap();
        assert!(
            report.candidate.is_none(),
            "DT7 capture promoted partial base evidence"
        );
        assert!(report.reason.is_some());
    }
}
