//! Expected UART frames are literal and independent of the native plan builder.
use super::*;
use crate::atx::transactions::RepeatPolicy;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
type Script = std::collections::VecDeque<(String, String)>;
fn frame(script: &mut Script, value: u16, reply: &str) {
    script.push_back((format!("h{value:04X}\n"), reply.into()));
}
fn probe(script: &mut Script, level: u8, case: &str) {
    frame(script, 0x0190, "J04\n");
    frame(
        script,
        0x0199,
        if case == "unsupported" {
            "J07\n"
        } else if case == "malformed_types" {
            "JFF\n"
        } else {
            "J01\n"
        },
    );
    if case == "unsupported" {
        return;
    }
    if case == "malformed_types" {
        frame(script, 0x01a7, "J00\n");
        frame(script, 0x01a7, "JFE\n");
        return;
    }
    // Battery FF is unknown; full bitmap FF and saturated counters are data.
    for (opcode, raw) in [
        (0xf1, 255),
        (0xf3, 255),
        (0xf4, 255),
        (0xf5, 255),
        (0xf6, level),
        (0xf7, 1),
        (0xf8, 254),
        (0xf9, 255),
        (0xfa, 2),
        (0xfb, 27),
        (0xfc, 255),
        (0xfd, 255),
        (0xff, 1),
    ] {
        frame(script, 0xc101, "N\n");
        let reply = if case == "partial" && opcode == 0xf1 {
            "X2\n".to_owned()
        } else {
            format!("J{raw:02X}\n")
        };
        frame(script, 0x0100 | opcode, &reply);
    }
    for (selector, raw) in [(4, 255), (5, 97), (6, 0), (7, 255)] {
        frame(script, 0xa300 | selector, "N\n");
        frame(script, 0xc101, "N\n");
        frame(script, 0x01f2, &format!("J{raw:02X}\n"));
    }
    for (selector, high, low) in [(0, 0x12, 0x34), (2, 0x23, 0x45)] {
        frame(script, 0xa300 | selector, "N\n");
        frame(script, 0xc101, "N\n");
        frame(script, 0x01f2, &format!("J{high:02X}\n"));
        frame(script, 0x019c, &format!("J{low:02X}\n"));
    }
    frame(script, 0x0190, "J04\n");
}
#[tokio::test]
async fn emergency_literal_uart_enable_repeat_atomic_timing_saturation_partial_and_type_refusal() {
    for (index, case) in [
        "read",
        "configure",
        "ignored_setting",
        "partial",
        "unsupported",
        "malformed_types",
    ]
    .into_iter()
    .enumerate()
    {
        let policy = if index % 2 == 0 {
            RepeatPolicy::OneTerminal
        } else {
            RepeatPolicy::TwoTerminals
        };
        let terminal = if policy == RepeatPolicy::TwoTerminals {
            "N\nN\n"
        } else {
            "N\n"
        };
        let mut script = Script::new();
        probe(&mut script, 254, case);
        if matches!(case, "configure" | "ignored_setting") {
            frame(&mut script, 0xa37b, "N\n");
            frame(&mut script, 0xc101, "N\n");
            script.push_back(("t01E9\n".into(), terminal.into()));
            probe(
                &mut script,
                if case == "configure" { 123 } else { 254 },
                case,
            );
        }
        let mut pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let done = Arc::new(tokio::sync::Notify::new());
        let completed = done.clone();
        let peer = tokio::spawn(async move {
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
                        let (expected, reply) =
                            script.pop_front().expect("unexpected DT1 native command");
                        assert_eq!(
                            line, expected,
                            "DT1 literal enable/repeat/atomic sequence changed:{case}"
                        );
                        reply
                    }
                };
                write.write_all(reply.as_bytes()).await.unwrap();
                if script.is_empty() {
                    completed.notify_one();
                }
            }
        });
        let fixture = Fixture::new();
        let saved = fixture.current_bytes();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service.with_uart(path, policy), &handle, &broker).await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let target = "Bus1.Outputs.Light1";
        let report = if matches!(case, "configure" | "ignored_setting") {
            handle
                .configure_emergency(
                    target,
                    1,
                    serde_json::from_value(json!({"emergency_level":123})).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap()
        } else {
            handle
                .read_emergency(target, context(1), deadline())
                .await
                .unwrap()
        };
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
        assert!(
            !peer.is_finished(),
            "literal DT1 peer exited before acceptance"
        );
        if case == "unsupported" {
            assert_eq!(report.support, Some(false));
            assert!(report.observed.is_none());
        } else if case == "malformed_types" {
            assert_eq!(report.support, None);
            assert!(report.observed.is_none());
        } else {
            let values = report.observed.unwrap();
            assert_eq!(
                values.fields["battery_charge"].status,
                if case == "partial" {
                    "unavailable"
                } else {
                    "unknown"
                }
            );
            assert_eq!(values.fields["total_runtime_hours"].value, Some(1020));
            assert!(values.fields["total_runtime_hours"].at_least);
            assert_eq!(values.fields["last_duration_seconds"].value, Some(30600));
            assert_eq!(
                values.fields["test_execution_timeout_minutes"].value,
                Some(15)
            );
            assert_eq!(values.fields["prolong_seconds"].value, Some(7650));
            assert_eq!(
                values.fields["next_function_test_minutes"].value,
                Some(0x1234 * 15)
            );
            assert_eq!(
                values.fields["next_duration_test_minutes"].value,
                Some(0x2345 * 15)
            );
            assert!(values.failures.values().all(|value| *value == Some(true)));
            assert_eq!(values.status.len(), 7);
            assert_eq!(
                report.outcome,
                match case {
                    "configure" => "applied",
                    "ignored_setting" => "unconfirmed",
                    _ => "observed",
                }
            );
            assert_eq!(
                values.quality,
                if case == "partial" {
                    "partial"
                } else {
                    "complete"
                }
            );
        }
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        peer.abort();
    }
}
