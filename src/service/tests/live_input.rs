use super::*;
use crate::atx::transactions::RepeatPolicy;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Notify};

struct Peer {
    path: PathBuf,
    raw: mpsc::Sender<(Vec<u8>, oneshot::Sender<()>)>,
    task: tokio::task::JoinHandle<()>,
    held: Arc<Notify>,
    commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    fn start() -> Self {
        let pty = super::super::hat::tests::Pty::new();
        let path = pty.path.clone();
        let (raw, mut receive) = mpsc::channel::<(Vec<u8>, oneshot::Sender<()>)>(4);
        let held = Arc::new(Notify::new());
        let signal = held.clone();
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = commands.clone();
        let task = tokio::spawn(async move {
            // Capture the whole owner: capturing only peer would drop the retained
            // slave before the bridge opens it, producing EOF and removing its path.
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            let mut level = 100;
            loop {
                tokio::select! {
                    raw=receive.recv()=>match raw {
                        Some((bytes,reply))=>{write.write_all(&bytes).await.unwrap();let _=reply.send(());},
                        None=>return,
                    },
                    count=read.read_until(b'\n',&mut line)=>{
                        if count.unwrap()==0 {return;}
                        assert!(line.len()<32);
                        recorded.lock().unwrap().push(line.clone());
                        let response=match line.as_slice() {
                            b"v\n"=>Some(b"V013204\n".to_vec()),
                            b"d\n"=>Some(b"D24\n".to_vec()),
                            b"h01A0\n"=>Some(format!("J{level:02X}\n").into_bytes()),
                            b"h0190\n"=>Some(b"J04\n".to_vec()),
                            b"h0040\n"=>{level=64;signal.notify_one();None},
                            other=>panic!("unexpected independent input-test HAT request: {other:?}"),
                        };
                        line.clear();
                        if let Some(bytes)=response {write.write_all(&bytes).await.unwrap();}
                    },
                }
            }
        });
        Self {
            path,
            raw,
            task,
            held,
            commands,
        }
    }
    async fn send(&self, bytes: &[u8]) {
        let (send, receive) = oneshot::channel();
        self.raw.try_send((bytes.to_vec(), send)).unwrap();
        tokio::time::timeout(Duration::from_secs(1), receive)
            .await
            .unwrap()
            .unwrap();
    }
}
fn input_fixture() -> Fixture {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses[0].inputs = serde_json::from_value(json!([
        {"short_address":0,"name":"Panel","instances":[
            {"number":0,"name":"First","instance_type":1},
            {"number":1,"name":"Position","instance_type":2},
            {"number":2,"name":"Occupancy","instance_type":3},
            {"number":3,"name":"Light","instance_type":4}]}]))
    .unwrap();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    fixture
}
async fn listening(handle: &ServiceHandle, peer: &Peer, target: &str) {
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if handle
                .snapshot()
                .is_some_and(|s| s.input_listeners.contains(target))
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
    assert!(
        result.is_ok(),
        "input listener did not activate: snapshot={:?}, commands={:?}, peer_done={}",
        handle
            .snapshot()
            .map(|s| (s.phase, s.hardware, s.input_listeners)),
        peer.commands.lock().unwrap(),
        peer.task.is_finished()
    );
}
fn actions(
    broker: &FakeBroker,
    target: &str,
) -> Vec<(serde_json::Value, mqtt_test_broker::Received)> {
    broker
        .received_on("DALI/Event/Controller")
        .into_iter()
        .filter_map(|record| {
            let value: serde_json::Value = serde_json::from_slice(&record.payload).unwrap();
            (value["target"] == target).then_some((value, record))
        })
        .collect()
}
async fn state(
    broker: &FakeBroker,
    target: &str,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let topic = format!("DALI/State/Controller/{target}");
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&topic)
                .last()
                .is_some_and(|m| predicate(
                    &serde_json::from_slice(&m.payload).unwrap()
                )))
            .await,
        "missing expected live input State for {target}"
    );
    serde_json::from_slice(&broker.received_on(&topic).last().unwrap().payload).unwrap()
}

#[tokio::test]
async fn live_input_mqtt_actions_survive_a_pending_light_command_and_publish_truthful_state() {
    let fixture = input_fixture();
    let mut peer = Peer::start();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    let first = "Bus1.Inputs.Panel.Instances.First";
    listening(&handle, &peer, first).await;
    let initial = state(&broker, first, |v| v["Quality"] == "unobserved").await;
    assert_eq!(initial["Status"], serde_json::Value::Null);
    assert_eq!(initial["Available"], false);
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light1",
        json!({"Brightness":64.0/254.0}).to_string(),
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property(
                "traceparent",
                "00-11111111111111111111111111111111-2222222222222222-01"
            )
    ));
    tokio::time::timeout(Duration::from_secs(2), peer.held.notified())
        .await
        .unwrap();
    peer.send(b"L008001\nL008000\n").await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| actions(b, first).len() == 2)
            .await,
        "pending light acknowledgement blocked native input actions"
    );
    let events = actions(&broker, first);
    assert_eq!(events[0].0["action"], "pressed");
    assert_eq!(events[1].0["action"], "released");
    for (_, event) in &events {
        assert!(!event.retain, "native action was retained");
        assert_eq!(
            event.user_property("target-generation"),
            context(1).generation()
        );
        assert_eq!(
            event.user_property("traceparent"),
            None,
            "input inherited the light command trace"
        );
    }
    let released = state(&broker, first, |v| {
        v["Status"] == false && v["Quality"] == "current"
    })
    .await;
    assert_eq!(released["Available"], true);
    peer.send(b"N\n").await;
    state(&broker, "Bus1.Outputs.Light1", |v| {
        v["Brightness"] == json!(64.0 / 254.0)
    })
    .await;
    peer.send(b"L008001\nL008002\n").await;
    state(&broker, first, |v| {
        v["Status"].is_null() && v["Quality"] == "partial" && v["Available"] == true
    })
    .await;
    peer.send(b"L008001\nL008103\n").await;
    state(&broker, first, |v| {
        v["Status"].is_null() && v["Quality"] == "unsupported" && v["Available"] == false
    })
    .await;
    // Literal sensor frames: instance 1 position=513, 2 occupancy flags=15,
    // instance 3 relative light=777. No production frame encoder is used.
    peer.send(b"L008601\nL00880F\nL008F09\nL010080\n").await;
    state(&broker, "Bus1.Inputs.Panel.Instances.Position", |v| {
        v["PositionRaw"] == 513
    })
    .await;
    state(&broker, "Bus1.Inputs.Panel.Instances.Occupancy", |v| {
        v["Occupied"] == true && v["Movement"] == true
    })
    .await;
    let light = state(&broker, "Bus1.Inputs.Panel.Instances.Light", |v| {
        v["LightLevelRaw"] == 777
    })
    .await;
    assert!(light.get("Lux").is_none());
    let commands = peer.commands.lock().unwrap().clone();
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"refresh","target":"Bus1.Inputs.Panel.Instances.First"}"#,
        &SendOptions::default().response_topic("test/input-refresh")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/input-refresh")
                .is_empty())
            .await
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&broker.received_on("test/input-refresh")[0].payload).unwrap();
    assert!(
        reply.get("error").is_none(),
        "cached input refresh failed: {reply}"
    );
    assert_eq!(
        peer.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|b| b.starts_with(b"h") || b.starts_with(b"l"))
            .count(),
        commands
            .iter()
            .filter(|b| b.starts_with(b"h") || b.starts_with(b"l"))
            .count(),
        "input refresh sent a hardware query"
    );
    assert!(running.stop().await);
    peer.task.abort();
    assert!((&mut peer.task).await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn live_input_rename_excludes_a_partial_old_uart_line_before_new_generation() {
    let fixture = input_fixture();
    let mut peer = Peer::start();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    let old = "Bus1.Inputs.Panel.Instances.First";
    let new = "Bus1.Inputs.Panel.Instances.Door";
    listening(&handle, &peer, old).await;
    peer.send(b"L0080").await;
    handle
        .edit(
            1,
            MetadataEdit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Door".into(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    listening(&handle, &peer, new).await;
    let mut notices = handle.subscribe_inputs();
    // Complete the pre-rename line, then a distinct post-boundary native release.
    peer.send(b"01\nL008000\n").await;
    let notice = tokio::time::timeout(Duration::from_secs(2), notices.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notice.target, new);
    assert_eq!(
        notice.value()["action"],
        "released",
        "old partial press acquired a fresh mapping token"
    );
    assert_eq!(notice.context.generation(), context(2).generation());
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !actions(b, new).is_empty())
            .await
    );
    let events = actions(&broker, new);
    assert_eq!(
        events.len(),
        1,
        "pre-rename partial line became a new endpoint action"
    );
    assert_eq!(events[0].0["action"], "released");
    assert_eq!(
        events[0].1.user_property("target-generation"),
        context(2).generation()
    );
    assert!(!handle.snapshot().unwrap().input_listeners.contains(old));
    assert!(running.stop().await);
    peer.task.abort();
    assert!((&mut peer.task).await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn live_input_events_from_the_existing_atx_emulator_reach_mqtt() {
    use crate::emulator::{pty, session::Control};
    let fixture = input_fixture();
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    let target = "Bus1.Inputs.Panel.Instances.First";
    wait_for(&handle, |s| s.input_listeners.contains(target)).await;
    for (frame, action, status) in [(0x008001, "pressed", true), (0x008000, "released", false)] {
        emulator
            .emulator
            .control(
                Control::InputEvent { bus: 0, frame },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        state(&broker, target, |v| {
            v["Status"] == status && v["Quality"] == "current"
        })
        .await;
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| actions(b, target)
                    .iter()
                    .any(|(v, _)| v["action"] == action))
                .await
        );
    }
    assert_eq!(
        actions(&broker, target).len(),
        2,
        "emulator action was duplicated"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn live_input_state_changed_during_all_refresh_is_projected_after_the_last_await() {
    let fixture = input_fixture();
    let mut peer = Peer::start();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let pause = Arc::new(Pause::default());
    service.proxy_read_pause = Some(pause.clone());
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    let target = "Bus1.Inputs.Panel.Instances.First";
    listening(&handle, &peer, target).await;
    peer.send(b"L008000\n").await;
    state(&broker, target, |v| v["Status"] == false).await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Status/Controller")
                .iter()
                .any(|r| r.payload == "connected"))
            .await
    );
    pause.armed.store(true, Ordering::Release);
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"refresh"}"#,
        &SendOptions::default().response_topic("test/all-input-refresh")
    ));
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
    peer.send(b"L008001\n").await;
    state(&broker, target, |v| v["Status"] == true).await;
    pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/all-input-refresh")
                .is_empty())
            .await
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&broker.received_on("test/all-input-refresh")[0].payload).unwrap();
    assert!(reply.get("error").is_none(), "All refresh failed: {reply}");
    let final_state: serde_json::Value = serde_json::from_slice(
        &broker
            .received_on(&format!("DALI/State/Controller/{target}"))
            .last()
            .unwrap()
            .payload,
    )
    .unwrap();
    assert_eq!(
        final_state["Status"], true,
        "aggregate refresh overwrote a newer input with its old snapshot"
    );
    assert_eq!(
        actions(&broker, target).len(),
        2,
        "refresh reconstructed native actions from State"
    );
    assert!(running.stop().await);
    peer.task.abort();
    assert!((&mut peer.task).await.unwrap_err().is_cancelled());
}
