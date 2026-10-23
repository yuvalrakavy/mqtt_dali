use super::*;
use crate::atx::transactions::RepeatPolicy;
use std::sync::{atomic::AtomicUsize, Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{mpsc, oneshot},
};

enum Action {
    Inject(Vec<u8>),
    HoldNextQuery,
    Disconnect,
}
struct Peer {
    address: std::net::SocketAddr,
    send: mpsc::Sender<(Action, oneshot::Sender<()>)>,
    task: tokio::task::JoinHandle<()>,
    connections: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
    held: Arc<tokio::sync::Notify>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (send, mut actions) = mpsc::channel::<(Action, oneshot::Sender<()>)>(4);
        let connections = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let held = Arc::new(tokio::sync::Notify::new());
        let holding = held.clone();
        let accepted = connections.clone();
        let disconnected = closed.clone();
        let task = tokio::spawn(async move {
            let mut level = 100;
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                accepted.fetch_add(1, Ordering::SeqCst);
                let mut stream = BufReader::new(stream);
                stream.get_mut().write_all(b"B000004\n").await.unwrap();
                let mut line = Vec::new();
                let mut hold_next = false;
                loop {
                    tokio::select! {
                        action = actions.recv() => match action {
                            Some((Action::Inject(bytes), reply)) => { stream.get_mut().write_all(&bytes).await.unwrap(); let _ = reply.send(()); }
                            Some((Action::HoldNextQuery, reply)) => { hold_next = true; let _ = reply.send(()); }
                            Some((Action::Disconnect, reply)) => { stream.get_mut().shutdown().await.unwrap(); let _ = reply.send(()); break; }
                            None => return,
                        },
                        result = stream.read_until(b'\n', &mut line) => {
                            if result.unwrap() == 0 { break; }
                            assert!(line.len() <= 32);
                            if hold_next && line == b"h01A0\n" {
                                hold_next = false;
                                line.clear();
                                holding.notify_one();
                                continue;
                            }
                            let response = match line.as_slice() {
                                b"v\n" => b"V000004\n".to_vec(),
                                b"d\n" => b"D24\n".to_vec(),
                                b"h0040\n" => { level = 64; b"N\n".to_vec() },
                                b"h0000\n" => { level = 0; b"N\n".to_vec() },
                                b"h01A0\n" => format!("0L008001\nJ{level:02X}\n").into_bytes(),
                                b"h0190\n" => b"J04\n".to_vec(),
                                b"h01C0\n" => b"J01\n".to_vec(),
                                b"h01C1\n" => b"J00\n".to_vec(),
                                b"l010080\n" | b"l010081\n" => b"J01\n".to_vec(),
                                b"l01008C\n" => b"JFF\n".to_vec(),
                                other => panic!("unexpected literal TCP HAT command {other:?}"),
                            };
                            line.clear();
                            // Deliberately split protocol records; writes are byte streams.
                            stream.get_mut().write_all(&response[..1]).await.unwrap();
                            tokio::task::yield_now().await;
                            stream.get_mut().write_all(&response[1..]).await.unwrap();
                        }
                    }
                }
                disconnected.fetch_add(1, Ordering::SeqCst);
            }
        });
        Self {
            address,
            send,
            task,
            connections,
            closed,
            held,
        }
    }
    async fn action(&self, action: Action) {
        let (send, reply) = oneshot::channel();
        self.send.try_send((action, send)).unwrap();
        tokio::time::timeout(Duration::from_secs(1), reply)
            .await
            .unwrap()
            .unwrap();
    }
}
fn tcp(service: DaliService, address: std::net::SocketAddr, policy: RepeatPolicy) -> DaliService {
    let mut service = service.with_hat_tcp(address, policy);
    let settings = service.uart.as_mut().unwrap();
    settings.quiet = Duration::from_millis(10);
    settings.timing.retry = Duration::from_millis(20);
    settings.timing.poll = Duration::from_secs(60);
    service
}

#[tokio::test]
async fn tcp_hat_literal_peer_splits_replies_interleaves_inputs_and_reconnects() {
    let fixture = Fixture::new();
    fixture.add_button();
    let peer = Peer::start().await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        tcp(service, peer.address, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    wait_for(&handle, |s| {
        s.input_listeners
            .contains("Bus1.Inputs.Panel.Instances.Button")
    })
    .await;
    let mut events = handle.subscribe_inputs();
    let observed = handle
        .apply_output(
            "Bus1.Outputs.Light1",
            crate::dali::gear::Intent::new(None, Some(64.0 / 254.0)).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        observed.brightness(),
        Some(64.0 / 254.0),
        "TCP reply lost native output evidence"
    );
    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        event.value()["action"],
        "pressed",
        "async L frame was confused with TCP command reply"
    );
    assert_eq!(event.target, "Bus1.Inputs.Panel.Instances.Button");
    assert!(event.current(&handle.snapshot().unwrap()));

    peer.action(Action::HoldNextQuery).await;
    let pending_handle = handle.clone();
    let pending = tokio::spawn(async move {
        pending_handle
            .read_output("Bus1.Outputs.Light1", context(1), deadline())
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    peer.action(Action::Disconnect).await;
    assert!(
        pending.await.unwrap().is_err(),
        "disconnect confirmed a TCP query with no terminal reply"
    );
    wait_for(&handle, |s| !matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        !event.current(&handle.snapshot().unwrap()),
        "old TCP session input action survived disconnect"
    );
    wait_for(&handle, |s| {
        peer.connections.load(Ordering::SeqCst) >= 2 && matches!(s.hardware, Hardware::Ready { .. })
    })
    .await;
    assert!(
        !event.current(&handle.snapshot().unwrap()),
        "old input action revived on TCP reconnect"
    );
    let mut events = handle.subscribe_inputs();
    wait_for(&handle, |s| {
        s.input_listeners
            .contains("Bus1.Inputs.Panel.Instances.Button")
    })
    .await;
    peer.action(Action::Inject(b"0L008000\n".to_vec())).await;
    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.value()["action"], "released");
    let observation = handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        observation.brightness(),
        Some(64.0 / 254.0),
        "TCP reconnect lost native fixture state"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(
        !peer.task.is_finished(),
        "literal TCP peer panicked before shutdown"
    );
    assert!(running.stop().await);
    wait_for(&handle, |_| peer.closed.load(Ordering::SeqCst) >= 2).await;
    assert!(
        !peer.task.is_finished(),
        "literal peer failed while proving TCP EOF"
    );
}

#[tokio::test]
async fn tcp_hat_uses_independent_emulator_core_for_both_repeat_profiles() {
    use crate::emulator::{
        scenario::Scenario,
        session::{Control, Emulator},
    };
    for (policy, repeat) in [
        (RepeatPolicy::OneTerminal, "one_terminal"),
        (RepeatPolicy::TwoTerminals, "two_terminals"),
    ] {
        let fixture = Fixture::new();
        fixture.add_button();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let scenario: Scenario = serde_json::from_value(json!({"version":1,"repeat":repeat,"buses":[{"number":0,
            "outputs":[{"id":"lamp","short_address":0,"level":100}],
            "inputs":[{"id":"panel","short_address":0,"instances":[1],"feedback":[{"instance":0}]}]}]})).unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            tcp(service, listener.local_addr().unwrap(), policy),
            &handle,
            &broker,
        )
        .await;
        let (socket, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let emulator = Emulator::start(socket, scenario).unwrap();
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let observed = handle
            .apply_output(
                "Bus1.Outputs.Light1",
                crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(observed.power(), Some(false));
        let report = handle
            .configure_output(
                "Bus1.Outputs.Light1",
                1,
                serde_json::from_value(json!({"minimum_level":2})).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .expect("TCP repeated settings did not honor the selected terminal profile");
        assert_eq!(
            report.outcome, "configured",
            "TCP repeated settings did not honor the selected terminal profile"
        );
        let feedback = handle
            .set_input_feedback(
                "Bus1.Inputs.Panel.Instances.Button",
                1,
                true,
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(feedback.outcome, "set");
        let native = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            native["buses"][0]["outputs"][0]["level"], 0,
            "TCP emulator native brightness did not change"
        );
        assert_eq!(
            native["buses"][0]["outputs"][0]["min_level"], 2,
            "TCP repeated settings did not change the independent native model"
        );
        assert_eq!(fixture.disk().revision, 1);
        assert!(running.stop().await);
        // EOF can already have stopped the independent emulator; close joins it.
        let _ = emulator.close().await;
    }
}

#[tokio::test]
async fn tcp_hat_refused_endpoint_keeps_metadata_available_and_stop_bounded() {
    let fixture = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        tcp(service, address, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| {
        matches!(s.hardware, Hardware::Unavailable { retrying: true, .. })
    })
    .await;
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    assert!(
        tokio::time::timeout(Duration::from_secs(2), running.stop())
            .await
            .unwrap(),
        "refused TCP endpoint blocked managed stop"
    );
}
