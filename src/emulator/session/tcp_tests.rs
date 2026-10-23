use super::*;
use tokio::net::TcpStream;

#[tokio::test]
async fn production_dispatcher_uses_tcp_listener_and_native_semantic_events_in_both_profiles() {
    for (repeat, policy) in [
        (Repeat::OneTerminal, RepeatPolicy::OneTerminal),
        (Repeat::TwoTerminals, RepeatPolicy::TwoTerminals),
    ] {
        let mut physical = scenario();
        physical.repeat = repeat;
        physical.buses[0].inputs.push(
            serde_json::from_value(
                serde_json::json!({"id":"panel","short_address":1,"instances":[1]}),
            )
            .unwrap(),
        );
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), physical)
            .await
            .unwrap();
        let (link, mut events) =
            Transactions::start(TcpStream::connect(address).await.unwrap(), policy);
        tokio::time::timeout(Duration::from_secs(1), async {
            while !matches!(
                events.recv().await.unwrap(),
                crate::atx::monitor::Event::Packet {
                    packet: Ok(crate::atx::wire::Packet {
                        kind: Kind::Boot(_),
                        ..
                    }),
                    ..
                }
            ) {}
        })
        .await
        .unwrap();
        link.initialize(Duration::from_secs(1)).await.unwrap();
        // A native repeated setter actually exercises terminal cardinality.
        link.execute(send(0xa33c), Duration::from_secs(1))
            .await
            .unwrap();
        link.execute(
            Tx::Send {
                bus: 0,
                frame: DaliFrame::Bits16(0x072b),
                repeat: true,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(
            link.execute(send(0x07a2), Duration::from_secs(1))
                .await
                .unwrap(),
            Response::Dali(vec![BusReply::Value(60)])
        );
        link.execute(send(0x0637), Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            link.execute(send(0x07a0), Duration::from_secs(1))
                .await
                .unwrap(),
            Response::Dali(vec![BusReply::Value(60)])
        );
        let stimulus=serde_json::from_value(serde_json::json!({"op":"stimulate","id":"panel","instance":0,"stimulus":{"kind":"button","pressed":true}})).unwrap();
        control(&emulator, stimulus).await;
        tokio::time::timeout(Duration::from_secs(1), async {
            while !matches!(
                events.recv().await.unwrap(),
                crate::atx::monitor::Event::Packet {
                    packet: Ok(crate::atx::wire::Packet {
                        kind: Kind::Dali(DaliFrame::Bits24(0x028001)),
                        ..
                    }),
                    ..
                }
            ) {}
        })
        .await
        .unwrap();
        link.close().await.unwrap();
        emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn pty_style_scenario_reset_fences_unread_commands_and_partial_records() {
    let (stream, mut peer) = tokio::io::duplex(65536);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    assert_eq!(line(&mut peer).await, b"B000004\n");
    peer.write_all(b"h062a\n".repeat(300).as_slice())
        .await
        .unwrap();
    peer.write_all(b"h06").await.unwrap();
    control(&emulator, Control::ResetScenario).await;
    let mut boot = false;
    for _ in 0..1000 {
        if line(&mut peer).await == b"B000004\n" {
            boot = true;
            break;
        }
    }
    assert!(boot, "reset Boot missing after receive fence");
    assert_eq!(
        control(&emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["level"],
        100
    );
    peer.write_all(b"2a\nh07a0\n").await.unwrap();
    assert_eq!(line(&mut peer).await, b"X4\n");
    assert_eq!(line(&mut peer).await, b"H07A0\n");
    assert_eq!(line(&mut peer).await, b"J64\n");
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn tcp_reconnect_keeps_native_state_and_discards_old_session_output() {
    let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario())
        .await
        .unwrap();
    let mut peer = TcpStream::connect(address).await.unwrap();
    assert_eq!(line(&mut peer).await, b"B000004\n");
    peer.write_all(b"h06").await.unwrap();
    peer.write_all(b"2a\nv\n").await.unwrap();
    for expected in [b"H062A\n".as_slice(), b"N\n", b"V000004\n"] {
        assert_eq!(line(&mut peer).await, expected);
    }
    let mut rival = TcpStream::connect(address).await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), rival.read_u8())
            .await
            .unwrap()
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    control(
        &emulator,
        Control::NextFault {
            effect: Fault::Delay { millis: 200 },
        },
    )
    .await;
    peer.write_all(b"h07a0\n").await.unwrap();
    assert_eq!(line(&mut peer).await, b"H07A0\n");
    drop(peer);
    // A control queued after observed EOF is not required: snapshot remains
    // available with no bridge, and subsequent accepted client gets fresh Boot.
    tokio::time::timeout(Duration::from_secs(1), async {
        while control(&emulator, Control::Snapshot).await["transport"]["connected"] != false {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut peer = TcpStream::connect(address).await.unwrap();
    assert_eq!(line(&mut peer).await, b"B000004\n");
    peer.write_all(b"h07a0\n").await.unwrap();
    assert_eq!(line(&mut peer).await, b"H07A0\n");
    assert_eq!(line(&mut peer).await, b"J2A\n");
    assert!(
        tokio::time::timeout(Duration::from_millis(250), peer.read_u8())
            .await
            .is_err()
    );
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn scenario_invalid_apply_is_atomic_live_add_preserves_gear_and_reset_disconnects() {
    let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario())
        .await
        .unwrap();
    let mut peer = TcpStream::connect(address).await.unwrap();
    assert_eq!(line(&mut peer).await, b"B000004\n");
    peer.write_all(b"h062a\n").await.unwrap();
    line(&mut peer).await;
    line(&mut peer).await;
    let mut invalid = scenario();
    invalid.version = 2;
    assert!(emulator
        .control(
            Control::ApplyScenario { scenario: invalid },
            Duration::from_secs(1)
        )
        .await
        .is_err());
    let output =
        serde_json::from_value(serde_json::json!({"id":"new","short_address":null})).unwrap();
    control(&emulator, Control::AddOutput { bus: 0, output }).await;
    let snapshot = control(&emulator, Control::Snapshot).await;
    assert_eq!(snapshot["buses"][0]["outputs"][0]["level"], 42);
    assert_eq!(snapshot["buses"][0]["outputs"].as_array().unwrap().len(), 2);
    let captured = control(&emulator, Control::CaptureScenario).await;
    let saved: Scenario = serde_json::from_value(captured).unwrap();
    saved.validate().unwrap();
    control(
        &emulator,
        Control::SetAddress {
            id: "physical-A".into(),
            short_address: None,
        },
    )
    .await;
    control(&emulator, Control::ResetScenario).await;
    assert!(peer.read_u8().await.is_err());
    let snapshot = control(&emulator, Control::Snapshot).await;
    assert_eq!(snapshot["buses"][0]["outputs"][0]["short_address"], 3);
    assert_eq!(snapshot["buses"][0]["outputs"][0]["level"], 42);
    emulator.close().await.unwrap();
}
