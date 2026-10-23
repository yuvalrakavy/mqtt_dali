use super::*;
use crate::atx::{
    serial::Serial,
    transactions::{
        BusReply, RepeatPolicy, Response, SessionState, TransactionError, Transactions,
    },
    wire::{Command as Tx, DaliFrame, Kind},
};
use tokio::io::{AsyncRead, AsyncReadExt};

fn scenario() -> Scenario {
    serde_json::from_str(r#"{"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[{"id":"physical-A","short_address":3,"level":100}]}]}"#).unwrap()
}
async fn control(emulator: &Emulator, action: Control) -> serde_json::Value {
    emulator
        .control(action, Duration::from_secs(1))
        .await
        .unwrap()
}
async fn line<R: AsyncRead + Unpin>(reader: &mut R) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut bytes = Vec::new();
        for _ in 0..128 {
            let byte = reader.read_u8().await.unwrap();
            bytes.push(byte);
            if byte == b'\n' {
                return bytes;
            }
        }
        panic!("unbounded emulator record");
    })
    .await
    .unwrap()
}
fn send(value: u16) -> Tx {
    Tx::Send {
        bus: 0,
        frame: DaliFrame::Bits16(value),
        repeat: false,
    }
}

#[tokio::test]
async fn literal_uart_transcript_changes_independent_fixture_state() {
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge.write_all(b"v\nh07a0\nh062a\nh07a0\n").await.unwrap();
    for expected in [
        b"V000004\n".as_slice(),
        b"H07A0\n",
        b"J64\n",
        b"H062A\n",
        b"N\n",
        b"H07A0\n",
        b"J2A\n",
    ] {
        assert_eq!(line(&mut bridge).await, expected);
    }
    let snapshot = control(&emulator, Control::Snapshot).await;
    assert_eq!(snapshot["buses"][0]["outputs"][0]["level"], 42);
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn literal_input_settings_uart_reboot_preserves_disabled_instance() {
    let scenario =
        serde_json::from_value(serde_json::json!({"version":1,"clock":"frozen","buses":[
            {"number":0,"inputs":[{"id":"panel","short_address":0,"instances":[1]}]}
        ]}))
        .unwrap();
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario).unwrap();
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge
        .write_all(b"l010063\nl010063\nl010086\n")
        .await
        .unwrap();
    for expected in [
        b"L010063\n".as_slice(),
        b"N\n",
        b"L010063\n",
        b"N\n",
        b"L010086\n",
        b"N\n",
    ] {
        assert_eq!(line(&mut bridge).await, expected);
    }
    let snapshot = control(&emulator, Control::Snapshot).await;
    assert_eq!(
        snapshot["buses"][0]["inputs"][0]["settings"][0]["enabled"], false,
        "24bit UART settings did not reach physical model"
    );
    control(&emulator, Control::Reboot).await;
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge.write_all(b"l010080\nl010086\n").await.unwrap();
    for expected in [b"L010080\n".as_slice(), b"J01\n", b"L010086\n", b"N\n"] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "HAT reboot changed physical input settings"
        );
    }
    // Raw test injection bypasses enable/filter state and does not re-enable it.
    control(
        &emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x008001,
        },
    )
    .await;
    assert_eq!(line(&mut bridge).await, b"L008001\n");
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn literal_button_timing_uart_and_hat_reboot_retains_physical_settings() {
    let scenario =
        serde_json::from_value(serde_json::json!({"version":1,"clock":"frozen","buses":[
            {"number":0,"inputs":[{"id":"panel","short_address":0,"instances":[1]}]}
        ]}))
        .unwrap();
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario).unwrap();
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge
        .write_all(b"lC130FF\nl010000\nl010000\nl01000A\nlC13032\nl010001\nl010001\nl01000C\n")
        .await
        .unwrap();
    for expected in [
        b"LC130FF\n".as_slice(),
        b"N\n",
        b"L010000\n",
        b"N\n",
        b"L010000\n",
        b"N\n",
        b"L01000A\n",
        b"JFF\n",
        b"LC13032\n",
        b"N\n",
        b"L010001\n",
        b"N\n",
        b"L010001\n",
        b"N\n",
        b"L01000C\n",
        b"J32\n",
    ] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "literal UART button timing mismatch"
        );
    }
    control(&emulator, Control::Reboot).await;
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge
        .write_all(b"l01000A\nl01000C\nl01000B\nl01000D\n")
        .await
        .unwrap();
    for expected in [
        b"L01000A\n".as_slice(),
        b"JFF\n",
        b"L01000C\n",
        b"J32\n",
        b"L01000B\n",
        b"J19\n",
        b"L01000D\n",
        b"J0A\n",
    ] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "HAT reboot changed physical button timing"
        );
    }
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn literal_sensor_settings_uart_and_reboot_preserves_physical_profile() {
    let scenario =
        serde_json::from_value(serde_json::json!({"version":1,"clock":"frozen","buses":[
            {"number":0,"inputs":[{"id":"sensor","short_address":0,"instances":[4]}]}
        ]}))
        .unwrap();
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario).unwrap();
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge
        .write_all(b"lC130FF\nl010030\nl010030\nl01003E\n")
        .await
        .unwrap();
    for expected in [
        b"LC130FF\n".as_slice(),
        b"N\n",
        b"L010030\n",
        b"N\n",
        b"L010030\n",
        b"N\n",
        b"L01003E\n",
        b"JFF\n",
    ] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "literal sensor UART setting failed"
        );
    }
    control(&emulator, Control::Reboot).await;
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    bridge.write_all(b"l01003E\nl01FE30\n").await.unwrap();
    for expected in [b"L01003E\n".as_slice(), b"JFF\n", b"L01FE30\n", b"J00\n"] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "HAT reboot changed sensor settings or device status"
        );
    }
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn production_transactions_use_actual_pty_and_receive_input_during_a_delayed_reply() {
    let pty = super::super::pty::spawn(scenario()).await.unwrap();
    let (link, mut events) = Transactions::start(
        Serial::open(&pty.path).await.unwrap(),
        RepeatPolicy::OneTerminal,
    );
    let mut boot_seen = false;
    // The initial B precedes explicit initialization; use the actual reader event.
    tokio::time::timeout(Duration::from_secs(1), async {
        while !boot_seen {
            boot_seen = matches!(
                events.recv().await.unwrap(),
                crate::atx::monitor::Event::Packet {
                    packet: Ok(crate::atx::wire::Packet {
                        kind: Kind::Boot(_),
                        ..
                    }),
                    ..
                }
            );
        }
    })
    .await
    .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    control(
        &pty.emulator,
        Control::NextFault {
            effect: Fault::Delay { millis: 100 },
        },
    )
    .await;
    let clone = link.clone();
    let query =
        tokio::spawn(async move { clone.execute(send(0x07a0), Duration::from_secs(1)).await });
    let mut status = link.observe_status();
    tokio::time::timeout(
        Duration::from_secs(1),
        status.wait_for(|s| matches!(s, SessionState::Awaiting { .. })),
    )
    .await
    .unwrap()
    .unwrap();
    control(
        &pty.emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x020401,
        },
    )
    .await;
    let mut input_seen = false;
    tokio::time::timeout(Duration::from_millis(90), async {
        while !input_seen {
            input_seen = matches!(
                events.recv().await.unwrap(),
                crate::atx::monitor::Event::Packet {
                    packet: Ok(crate::atx::wire::Packet {
                        kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                        ..
                    }),
                    ..
                }
            );
        }
    })
    .await
    .expect("delayed terminal blocked input event injection");
    assert!(
        !query.is_finished(),
        "input event falsely completed the query"
    );
    assert_eq!(
        query.await.unwrap().unwrap(),
        Response::Dali(vec![BusReply::Value(100)])
    );
    link.execute(send(0x062a), Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        control(&pty.emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["level"],
        42
    );
    link.close().await.unwrap();
    pty.emulator.close().await.unwrap();
    assert!(
        Serial::open(&pty.path).await.is_err(),
        "successful close retained a PTY descriptor"
    );
}

#[tokio::test]
async fn hat_reboot_preserves_fixture_dtr_and_initialization_and_clears_delayed_terminals() {
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    line(&mut bridge).await;
    bridge.write_all(b"tA500\nhA37B\n").await.unwrap();
    for _ in 0..5 {
        line(&mut bridge).await;
    }
    let before = control(&emulator, Control::Snapshot).await;
    control(
        &emulator,
        Control::NextFault {
            effect: Fault::Delay { millis: 30 },
        },
    )
    .await;
    bridge.write_all(b"h0798\n").await.unwrap();
    assert_eq!(line(&mut bridge).await, b"H0798\n");
    control(&emulator, Control::Reboot).await;
    assert_eq!(line(&mut bridge).await, b"B000004\n");
    let after = control(&emulator, Control::Snapshot).await;
    assert_eq!(
        after["buses"], before["buses"],
        "HAT reboot silently reset independently powered fixtures"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(60), bridge.read_u8())
            .await
            .is_err(),
        "old delayed terminal survived HAT reboot"
    );
    control(&emulator, Control::AdvanceTime { millis: 900000 }).await;
    assert!(
        control(&emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["initialize_until"]
            .is_null()
    );
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn unread_output_cannot_hold_inspection_or_shutdown() {
    let (stream, _unread_peer) = tokio::io::duplex(32);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    control(
        &emulator,
        Control::Raw {
            bytes: vec![b'X'; 4096],
        },
    )
    .await;
    let snapshot = control(&emulator, Control::Snapshot).await;
    assert!(snapshot["transport"]["queued_bytes"].as_u64().unwrap() > 0);
    tokio::time::timeout(Duration::from_millis(200), emulator.close())
        .await
        .expect("unread UART held shutdown")
        .unwrap();
}

#[tokio::test]
async fn malformed_command_and_unsupported_capability_never_claim_a_device_reply() {
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    line(&mut bridge).await;
    bridge.write_all(b"hBAD\nl0700FF\nh07E2\n").await.unwrap();
    for expected in [b"X4\n".as_slice(), b"L0700FF\n", b"N\n", b"H07E2\n", b"N\n"] {
        assert_eq!(line(&mut bridge).await, expected);
    }
    assert_eq!(
        control(&emulator, Control::Snapshot).await["unsupported_commands"],
        1
    );
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn delayed_terminal_after_timeout_retires_the_production_dispatcher_slot() {
    let (stream, bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    let (link, mut events) = Transactions::start(bridge, RepeatPolicy::OneTerminal);
    events.recv().await.unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    control(
        &emulator,
        Control::NextFault {
            effect: Fault::Delay { millis: 100 },
        },
    )
    .await;
    assert_eq!(
        link.execute(send(0x07a0), Duration::from_millis(30)).await,
        Err(TransactionError::Deadline)
    );
    let mut status = link.observe_status();
    tokio::time::timeout(
        Duration::from_secs(1),
        status.wait_for(|s| *s == SessionState::Ready),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        link.execute(send(0x07a0), Duration::from_secs(1))
            .await
            .unwrap(),
        Response::Dali(vec![BusReply::Value(100)])
    );
    link.close().await.unwrap();
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn a_truncated_record_is_delimited_before_a_reboot_so_transactions_can_reinitialize() {
    let (stream, bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    let (link, mut events) = Transactions::start(bridge, RepeatPolicy::OneTerminal);
    events.recv().await.unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    control(
        &emulator,
        Control::NextFault {
            effect: Fault::Truncate { length: 1 },
        },
    )
    .await;
    assert_eq!(
        link.execute(send(0x07a0), Duration::from_millis(30)).await,
        Err(TransactionError::Deadline)
    );
    control(&emulator, Control::Reboot).await;
    let mut status = link.observe_status();
    tokio::time::timeout(
        Duration::from_millis(200),
        status.wait_for(|s| *s == SessionState::Uninitialized),
    )
    .await
    .expect("truncated prefix swallowed the reboot record")
    .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    link.close().await.unwrap();
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn an_intervening_transmission_of_another_width_breaks_a_repeat_pair() {
    let (stream, mut bridge) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    line(&mut bridge).await;
    bridge
        .write_all(b"h0762\nl020401\nh0762\nh07c0\n")
        .await
        .unwrap();
    for expected in [
        b"H0762\n".as_slice(),
        b"N\n",
        b"L020401\n",
        b"N\n",
        b"H0762\n",
        b"N\n",
        b"H07C0\n",
        b"J00\n",
    ] {
        assert_eq!(
            line(&mut bridge).await,
            expected,
            "intervening non-16-bit traffic failed to invalidate the repeat latch"
        );
    }
    emulator.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn stalled_uart_expires_even_when_model_time_is_frozen() {
    let (stream, _unread) = tokio::io::duplex(16);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    control(
        &emulator,
        Control::Raw {
            bytes: vec![b'X'; 4096],
        },
    )
    .await;
    let mut status = emulator.observe_status();
    let terminal = tokio::time::timeout(Duration::from_secs(2), status.wait_for(Option::is_some))
        .await
        .expect("frozen model clock disabled the UART write deadline")
        .unwrap();
    assert!(terminal
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .contains("UART write deadline"));
}

#[tokio::test]
async fn canceled_and_expired_queued_controls_do_not_mutate_the_model() {
    let (stream, _peer) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    let (reply, receive) = oneshot::channel();
    drop(receive);
    emulator
        .shared
        .controls
        .try_send(Request {
            action: Control::SetAddress {
                id: "physical-A".into(),
                short_address: Some(20),
            },
            deadline: Instant::now() + Duration::from_secs(1),
            reply,
        })
        .unwrap();
    let (reply, _receive) = oneshot::channel();
    emulator
        .shared
        .controls
        .try_send(Request {
            action: Control::SetAddress {
                id: "physical-A".into(),
                short_address: Some(21),
            },
            deadline: Instant::now(),
            reply,
        })
        .unwrap();
    assert_eq!(
        control(&emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["short_address"],
        3,
        "retired queued control still mutated a fixture"
    );
    emulator.close().await.unwrap();
}

#[test]
fn timer_and_uart_output_overflow_fail_explicitly_instead_of_dropping_bytes() {
    let scenario = scenario();
    let mut state = State {
        model: Model::new(scenario.clone()).unwrap(),
        clock: scenario.clock,
        repeat: scenario.repeat,
        started: Instant::now(),
        offset_ms: 0,
        output: VecDeque::new(),
        scheduled: Vec::new(),
        order: 0,
        next_fault: None,
        framer: Framer::default(),
        unterminated_output: false,
    };
    for _ in 0..64 {
        state.next_fault = Some(Fault::Delay { millis: 1000 });
        state.terminal(0, Reply::Value(1)).unwrap();
    }
    state.next_fault = Some(Fault::Delay { millis: 1000 });
    assert!(state
        .terminal(0, Reply::Value(2))
        .unwrap_err()
        .contains("timer queue overflow"));
    for _ in 0..256 {
        state.enqueue(b"N\n".to_vec()).unwrap();
    }
    assert!(state
        .enqueue(b"N\n".to_vec())
        .unwrap_err()
        .contains("UART output queue overflow"));
}
