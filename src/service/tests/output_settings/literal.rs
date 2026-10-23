//! The wire oracle is literal and independent from both codec and emulator.
use super::*;
use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
mod switching;
struct Peer {
    path: PathBuf,
    commands: Arc<std::sync::Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    fn new(script: Vec<(String, String)>) -> Self {
        Self::paused(script, Arc::new(Pause::default()))
    }
    fn paused(script: Vec<(String, String)>, pause: Arc<Pause>) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = commands.clone();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut script = VecDeque::from(script);
            loop {
                let mut line = String::new();
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                seen.lock().unwrap().push(line.clone());
                if line == "h0190\n" {
                    pause.point().await;
                }
                let reply = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        let (expected, reply) = script
                            .pop_front()
                            .expect("unexpected literal output settings command");
                        assert_eq!(line, expected, "literal output settings command mismatch");
                        reply
                    }
                };
                write.write_all(reply.as_bytes()).await.unwrap();
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
async fn managed_output_settings_cut_actual_and_groups_before_native_preflight() {
    let fixture = Fixture::new();
    let mut script = vec![
        ("h01A0\n".into(), "J64\n".into()),
        ("h0190\n".into(), "J04\n".into()),
        ("h01C0\n".into(), "J01\n".into()),
        ("h01C1\n".into(), "J00\n".into()),
        ("h01A0\n".into(), "J64\n".into()),
        ("h0190\n".into(), "J04\n".into()),
    ];
    script.extend(probe(1, 254, 7));
    script.extend([
        ("hA305\n".into(), "N\n".into()),
        ("t012B\n".into(), "N\n".into()),
    ]);
    script.extend(probe(5, 254, 7));
    let pause = Arc::new(Pause::default());
    let peer = Peer::paused(script, pause.clone());
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    handle
        .read_output(TARGET, context(1), limit())
        .await
        .unwrap();
    handle
        .read_group_output("Bus1.Groups.All", context(1), limit())
        .await
        .unwrap();
    pause.armed.store(true, Ordering::Release);
    let h = handle.clone();
    let operation = tokio::spawn(async move {
        h.configure_output(
            TARGET,
            1,
            Patch {
                minimum_level: Some(5),
                ..Default::default()
            },
            context(1),
            limit(),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), pause.entered.notified())
        .await
        .unwrap();
    let snapshot = handle.snapshot().unwrap();
    assert_eq!(
        snapshot.outputs[TARGET].quality,
        output::Quality::Stale,
        "settings left actual output current before native preflight"
    );
    assert!(
        snapshot.group_outputs["Bus1.Groups.All"].stale,
        "settings left group evidence current before native preflight"
    );
    assert!(
        writes(&peer).is_empty(),
        "paused preflight already sent a setting"
    );
    assert_eq!(
        handle.permit.available_permits(),
        0,
        "settings released ownership while the physical helper was running"
    );
    pause.release.notify_one();
    assert_eq!(operation.await.unwrap().unwrap().outcome, "configured");
    assert!(running.stop().await);
}
fn probe(min: u8, max: u8, fade: u8) -> Vec<(String, String)> {
    [
        (0x90, 4),
        (0x97, 8),
        (0xa0, 100),
        (0x9a, 1),
        (0xa2, min),
        (0xa1, max),
        (0xa3, 255),
        (0xa4, 255),
        (0xa5, fade),
        (0x99, 6),
        (0x90, 4),
    ]
    .into_iter()
    .map(|(q, v)| (format!("h01{q:02X}\n"), format!("J{v:02X}\n")))
    .collect()
}
fn writes(peer: &Peer) -> Vec<String> {
    peer.commands
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.starts_with("hA3") || c.starts_with('t'))
        .cloned()
        .collect()
}

#[tokio::test]
async fn managed_output_settings_literal_whole_patch_and_physical_preflight_send_no_writes() {
    for mode in [
        "physical",
        "missing_limit",
        "missing_late_family",
        "bad_late_field",
        "crossed",
        "no_profile",
        "missing_presence",
    ] {
        let fixture = Fixture::new();
        let mut script = probe(5, 200, 7);
        if mode == "physical" {
            script[3].1 = "J05\n".into();
        }
        if mode == "missing_limit" {
            script[5].1 = "N\n".into();
        }
        if mode == "missing_late_family" {
            script[6].1 = "N\n".into();
        }
        if mode == "missing_presence" {
            script[10].1 = "N\n".into();
        }
        let peer = Peer::new(script);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &peer.path,
                if mode == "no_profile" {
                    RepeatPolicy::Unverified
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let mut patch = Patch {
            minimum_level: Some(if mode == "physical" { 2 } else { 10 }),
            ..Patch::default()
        };
        if mode == "bad_late_field" {
            patch.fade_rate = Some(0);
        }
        if mode == "missing_late_family" {
            patch.power_on_level = Some(0);
        }
        if mode == "crossed" {
            patch.maximum_level = Some(5);
        }
        let result = handle
            .configure_output(TARGET, 1, patch, context(1), limit())
            .await;
        assert!(
            writes(&peer).is_empty(),
            "output settings preflight wrote part of a rejected patch: {mode}"
        );
        assert!(
            result.is_err() || result.as_ref().unwrap().outcome == "rejected",
            "invalid output patch accepted: {mode}: {result:?}"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_output_settings_literal_raw_ff_reserved_rate_and_partial_families_remain_usable() {
    for repairing in [false, true] {
        let fixture = Fixture::new();
        let mut before = probe(1, 254, if repairing { 0xf0 } else { 0xff });
        // Unrequested diagnostics and limits can be absent without blocking a
        // readable startup/fade family. Missing evidence remains explicit.
        before[1].1 = "N\n".into();
        before[5].1 = "X2\n".into();
        let mut after = before.clone();
        let patch = if repairing {
            after[8].1 = "JF1\n".into();
            Patch {
                fade_rate: Some(1),
                ..Patch::default()
            }
        } else {
            after[6].1 = "J00\n".into();
            Patch {
                power_on_level: Some(0),
                ..Patch::default()
            }
        };
        let mut script = before;
        script.extend(if repairing {
            [
                ("hA301\n".into(), "N\n".into()),
                ("t012F\n".into(), "N\n".into()),
            ]
        } else {
            [
                ("hA300\n".into(), "N\n".into()),
                ("t012D\n".into(), "N\n".into()),
            ]
        });
        script.extend(after);
        let peer = Peer::new(script);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let r = handle
            .configure_output(TARGET, 1, patch, context(1), limit())
            .await
            .unwrap();
        assert_eq!(
            r.outcome, "configured",
            "unrequested missing data or reserved rate prevented a valid settings write"
        );
        let values = r.settings.unwrap();
        assert_eq!(values.quality, "partial");
        assert_eq!(values.maximum_level, None);
        assert_eq!(values.system_failure_level, Some(255));
        assert_eq!(values.fade_raw, Some(if repairing { 0xf1 } else { 0xff }));
        assert_eq!(values.fade_time, Some(15));
        assert_eq!(values.fade_rate, Some(if repairing { 1 } else { 15 }));
        assert_eq!(
            r.readings.unwrap()["maximum_level"],
            crate::service::scenes::Reading::ReceiveCollision { code: Some(2) }
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_output_settings_literal_sends_never_replace_actual_confirmation() {
    for mode in ["ignored", "missing_after", "collision_after", "mask_limit"] {
        let fixture = Fixture::new();
        let mut script = probe(1, 254, 0xf1);
        script.extend([
            ("hA305\n".into(), "N\n".into()),
            ("t012B\n".into(), "N\n".into()),
        ]);
        let mut after = probe(5, 254, 0xf1);
        match mode {
            "ignored" => after[4].1 = "J01\n".into(),
            "missing_after" => after[10].1 = "N\n".into(),
            "collision_after" => after[4].1 = "Z9\n".into(),
            _ => after[4].1 = "JFF\n".into(),
        };
        script.extend(after);
        let peer = Peer::new(script);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let r = handle
            .configure_output(
                TARGET,
                1,
                Patch {
                    minimum_level: Some(5),
                    ..Patch::default()
                },
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(
            r.outcome, "unconfirmed",
            "native output settings send was mistaken for confirmation: {mode}"
        );
        assert_eq!(writes(&peer).len(), 2);
        if mode == "missing_after" {
            assert!(r.settings.is_none());
        }
        if mode == "collision_after" {
            assert_eq!(
                r.readings.unwrap()["minimum_level"],
                crate::service::scenes::Reading::TransmitCollision { code: Some(9) }
            );
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_output_settings_literal_crossed_limits_are_ordered_and_confirmed() {
    for raising in [false, true] {
        let fixture = Fixture::new();
        let (oldmin, oldmax, newmin, newmax) = if raising {
            (5, 50, 100, 200)
        } else {
            (100, 200, 5, 50)
        };
        let mut script = probe(oldmin, oldmax, 0xf1);
        let native = if raising {
            ["hA3C8\n", "t012A\n", "hA364\n", "t012B\n"]
        } else {
            ["hA305\n", "t012B\n", "hA332\n", "t012A\n"]
        };
        for command in native {
            script.push((command.into(), "N\n".into()));
        }
        script.extend(probe(newmin, newmax, 0xf1));
        let peer = Peer::new(script);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle
            .configure_output(
                TARGET,
                1,
                Patch {
                    minimum_level: Some(newmin),
                    maximum_level: Some(newmax),
                    ..Patch::default()
                },
                context(1),
                limit(),
            )
            .await;
        assert!(
            result.is_ok(),
            "native output settings crossing operation was not executed: {result:?}"
        );
        assert_eq!(
            result.unwrap().outcome,
            "configured",
            "native output settings crossing order or readback was not confirmed"
        );
        let commands = peer.commands.lock().unwrap().clone();
        assert!(
            native.iter().all(|x| commands.iter().any(|c| c == x)),
            "native settings setter sequence missing"
        );
        assert!(running.stop().await);
    }
}
