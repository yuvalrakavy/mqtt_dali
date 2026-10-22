use super::*;
use crate::atx::transactions::RepeatPolicy;
use crate::emulator::{pty, scenario::Scenario, session::Control};
use crate::service::input::{self, Evidence};
mod peer;

#[tokio::test]
async fn input_readback_reply_and_cache_use_the_same_final_acceptance_evidence() {
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.output_acceptance_hook =
        Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .read_input(&target("Button"), context(1), deadline())
        .await;
    assert!(
        handle.snapshot().unwrap().inputs[&target("Button")].stale,
        "final input cache kept pre-validation evidence"
    );
    assert!(
        matches!(result, Err(ServiceError::Rejected(_))),
        "input caller accepted a sample that the final cache invalidated"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

fn configured() -> (Fixture, Scenario) {
    let fixture = Fixture::new();
    let types = [1, 2, 2, 3, 4, 4, 31];
    let names = [
        "Button",
        "Contact",
        "Position",
        "Occupancy",
        "Light",
        "Wide",
        "Unknown",
    ];
    let resolutions = [1, 1, 10, 2, 16, 255, 8];
    let bytes = vec![
        vec![255],
        vec![0],
        vec![171, 192],
        vec![170],
        vec![18, 52],
        vec![255; 32],
        vec![42],
    ];
    let mut inventory = fixture.disk();
    let instances: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| json!({"number":i,"name":name,"instance_type":types[i]}))
        .collect();
    inventory.buses[0].inputs =
        serde_json::from_value(json!([{"short_address":0,"name":"Panel","instances":instances}]))
            .unwrap();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let values: Vec<_> = bytes
        .into_iter()
        .enumerate()
        .map(|(i, bytes)| json!({"instance":i,"resolution":resolutions[i],"bytes":bytes}))
        .collect();
    let scenario = serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":100}],
        "inputs":[{"id":"panel","short_address":0,"instances":types,"values":values}]}]}))
    .unwrap();
    (fixture, scenario)
}
fn target(name: &str) -> String {
    format!("Bus1.Inputs.Panel.Instances.{name}")
}
fn projected(handle: &ServiceHandle, name: &str) -> serde_json::Value {
    serde_json::to_value(
        input::projections(&handle.snapshot().unwrap())
            .into_iter()
            .find(|s| s.state.target == target(name))
            .unwrap()
            .state
            .values,
    )
    .unwrap()
}
async fn control(emulator: &crate::emulator::session::Emulator, action: Control) {
    emulator
        .control(action, Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn input_readback_common_types_and_wide_values_are_current_without_native_actions() {
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let mut events = handle.subscribe_inputs();
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    for (name, hex, resolution) in [
        ("Button", "01".to_string(), 1),
        ("Contact", "00".to_string(), 1),
        ("Position", "02AF".to_string(), 10),
        ("Occupancy", "02".to_string(), 2),
        ("Light", "1234".to_string(), 16),
        ("Wide", format!("7F{}", "FF".repeat(31)), 255),
        ("Unknown", "2A".to_string(), 8),
    ] {
        let observation = handle
            .read_input(
                &target(name),
                context(1).with_traceparent(Some(trace)),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(observation.context.traceparent(), Some(trace));
        let value = projected(&handle, name);
        assert_eq!(
            value["InputValueRaw"], hex,
            "input full-resolution value was lost"
        );
        assert_eq!(value["ValueResolution"], resolution);
        assert_eq!(value["Available"], name != "Unknown");
        assert_eq!(
            value["Quality"],
            if name == "Unknown" {
                "unsupported"
            } else {
                "current"
            }
        );
        assert!(value["EventInformation"].is_null());
        assert_eq!(value["ValueSource"], "query");
    }
    assert_eq!(projected(&handle, "Button")["Status"], true);
    assert_eq!(projected(&handle, "Contact")["InputState"], false);
    assert_eq!(
        projected(&handle, "Position")["Position"],
        json!(687.0 / 1023.0)
    );
    assert_eq!(
        projected(&handle, "Position")["PositionRaw"],
        serde_json::Value::Null
    );
    assert_eq!(projected(&handle, "Occupancy")["Occupied"], true);
    assert_eq!(projected(&handle, "Occupancy")["Movement"], false);
    assert!(projected(&handle, "Light")["LightLevelRaw"].is_null());
    assert!(projected(&handle, "Light").get("Lux").is_none());
    control(
        &emulator.emulator,
        Control::SetInputValue {
            id: "panel".into(),
            instance: 1,
            bytes: vec![255],
        },
    )
    .await;
    handle
        .read_input(&target("Contact"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        projected(&handle, "Contact")["InputState"],
        true,
        "maintained contact did not reflect physical readback"
    );
    assert!(
        events.try_recv().is_err(),
        "query generated a native input action"
    );
    let topic = format!("DALI/State/Controller/{}", target("Button"));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&topic)
                .iter()
                .any(|r| {
                    serde_json::from_slice::<serde_json::Value>(&r.payload).unwrap()
                        ["InputValueRaw"]
                        == "01"
                        && r.user_property("traceparent") == Some(trace)
                }))
            .await,
        "queried state lost its original request trace"
    );
    wait_for(&handle, |s| s.input_listeners.contains(&target("Button"))).await;
    control(
        &emulator.emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x008000,
        },
    )
    .await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&topic)
                .iter()
                .any(|r| {
                    let v: serde_json::Value = serde_json::from_slice(&r.payload).unwrap();
                    v["ValueSource"] == "event"
                        && v["Status"] == false
                        && v["InputValueRaw"].is_null()
                        && v["ValueResolution"].is_null()
                        && r.user_property("traceparent").is_none()
                }))
            .await,
        "native state retained old queried data or trace"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn input_readback_missing_collision_and_changed_type_clear_old_semantics() {
    let (fixture, mut scenario) = configured();
    let mut duplicate = scenario.buses[0].inputs[0].clone();
    duplicate.id = "other".into();
    duplicate.short_address = Some(1);
    duplicate.values[0].bytes = vec![0];
    scenario.buses[0].inputs.push(duplicate);
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(projected(&handle, "Button")["Status"], true);
    control(
        &emulator.emulator,
        Control::SetAddress {
            id: "other".into(),
            short_address: Some(0),
        },
    )
    .await;
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    let collision = projected(&handle, "Button");
    assert_eq!(collision["Quality"], "partial");
    assert_eq!(
        collision["Available"], false,
        "metadata-only query advertised an available input"
    );
    assert!(
        collision["Status"].is_null() && collision["InputValueRaw"].is_null(),
        "colliding query preserved a previous press"
    );
    control(
        &emulator.emulator,
        Control::SetAddress {
            id: "other".into(),
            short_address: None,
        },
    )
    .await;
    control(
        &emulator.emulator,
        Control::SetAddress {
            id: "panel".into(),
            short_address: None,
        },
    )
    .await;
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    let missing = projected(&handle, "Button");
    assert_eq!(missing["Quality"], "unavailable");
    assert!(missing["Status"].is_null());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();

    let (fixture, mut scenario) = configured();
    scenario.buses[0].inputs[0].instances[0] = 2;
    let emulator = pty::spawn(scenario).await.unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    handle
        .read_input(&target("Button"), context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        projected(&handle, "Button")["Quality"],
        "type_mismatch",
        "saved type overrode physical input type"
    );
    assert_eq!(projected(&handle, "Button")["Available"], false);
    assert!(projected(&handle, "Button")["Status"].is_null());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn input_readback_aggregate_reads_inputs_and_bus_loss_clears_merged_query_data() {
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"refresh"}"#,
        &SendOptions::default().response_topic("test/input-all")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/input-all")
                .is_empty())
            .await
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&broker.received_on("test/input-all")[0].payload).unwrap();
    assert!(
        reply.get("error").is_none(),
        "aggregate input query failed: {reply}"
    );
    assert_eq!(
        projected(&handle, "Light")["InputValueRaw"],
        "1234",
        "aggregate refresh skipped physical inputs"
    );
    let topic = format!("DALI/State/Controller/{}", target("Light"));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&topic)
                .iter()
                .any(|r| serde_json::from_slice::<serde_json::Value>(&r.payload)
                    .unwrap()["InputValueRaw"]
                    == "1234"))
            .await
    );
    control(
        &emulator.emulator,
        Control::BusPower {
            bus: 0,
            powered: false,
        },
    )
    .await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&topic)
                .iter()
                .any(|r| {
                    let v: serde_json::Value = serde_json::from_slice(&r.payload).unwrap();
                    v["Quality"] == "stale"
                        && v["Available"] == false
                        && v["InputValueRaw"].is_null()
                        && v["ValueResolution"].is_null()
                }))
            .await,
        "stale merged input query data survived bus loss"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn input_readback_paced_255_bit_mqtt_refresh_uses_one_sufficient_deadline() {
    let (fixture, _) = configured();
    let mut peer = peer::Peer::start(4, 255, vec![255; 32], Duration::from_millis(80), false);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"refresh","target":"Bus1.Inputs.Panel.Instances.Wide"}"#,
        &SendOptions::default().response_topic("test/wide-input")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b
                .received_on("test/wide-input")
                .is_empty())
            .await,
        "paced full-resolution input refresh never completed"
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&broker.received_on("test/wide-input")[0].payload).unwrap();
    assert!(
        reply.get("error").is_none(),
        "paced full-resolution input refresh failed its original deadline: {reply}"
    );
    assert_eq!(
        projected(&handle, "Wide")["InputValueRaw"],
        format!("7F{}", "FF".repeat(31))
    );
    let commands = peer.commands.lock().unwrap().clone();
    assert_eq!(
        commands
            .iter()
            .filter(|c| c.starts_with(b"l01058D"))
            .count(),
        31,
        "wide input did not read every latched byte"
    );
    assert!(running.stop().await);
    peer.close().await;
}

async fn native_during_query(cancel: bool) {
    let (fixture, _) = configured();
    let mut peer = peer::Peer::start(1, 1, vec![255], Duration::ZERO, true);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| s.input_listeners.contains(&target("Button"))).await;
    let worker = handle.clone();
    let mut read = tokio::spawn(async move {
        worker
            .read_input(&target("Button"), context(1), deadline())
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    wait_for(&handle, |s| {
        s.inputs
            .get(&target("Button"))
            .is_some_and(|o| matches!(o.evidence, Evidence::Event { .. }))
    })
    .await;
    assert_eq!(projected(&handle, "Button")["Status"], false);
    if cancel {
        read.abort();
        assert!((&mut read).await.unwrap_err().is_cancelled());
    }
    assert!(
        matches!(
            handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await,
            Err(ServiceError::Busy)
        ),
        "input query released the shared permit before physical settlement"
    );
    peer.release.notify_one();
    if !cancel {
        let observed = read.await.unwrap().unwrap();
        assert!(
            matches!(observed.evidence, Evidence::Event { .. }),
            "latched query overwrote a newer native event"
        );
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match handle
                .read_output("Bus1.Outputs.Light1", context(1), deadline())
                .await
            {
                Err(ServiceError::Busy) => tokio::task::yield_now().await,
                Ok(_) => break,
                other => panic!("physical owner did not accept the next output: {other:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        projected(&handle, "Button")["Status"],
        false,
        "late query published an old press"
    );
    assert!(running.stop().await);
    peer.close().await;
}
#[tokio::test]
async fn input_readback_newer_native_event_wins_over_a_latched_query() {
    native_during_query(false).await;
}
#[tokio::test]
async fn input_readback_cancelled_caller_keeps_physical_ownership_until_settlement() {
    native_during_query(true).await;
}

#[tokio::test]
async fn input_readback_zero_resolution_and_missing_latch_never_publish_a_partial_value() {
    for (resolution, bytes) in [(0, vec![255]), (16, vec![255])] {
        let (fixture, _) = configured();
        let mut peer = peer::Peer::start(4, resolution, bytes, Duration::ZERO, false);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        handle
            .read_input(&target("Wide"), context(1), deadline())
            .await
            .unwrap();
        let value = projected(&handle, "Wide");
        assert_eq!(value["Quality"], "partial");
        assert_eq!(value["Available"], false);
        assert!(
            value["InputValueRaw"].is_null(),
            "incomplete latch sequence published a partial value"
        );
        assert!(running.stop().await);
        peer.close().await;
    }
}

#[tokio::test]
async fn input_readback_mapping_change_during_frontend_wait_refuses_the_old_refresh() {
    let (fixture, _) = configured();
    let mut peer = peer::Peer::start(1, 1, vec![255], Duration::ZERO, false);
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    pause.armed.store(true, Ordering::Release);
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"refresh","target":"Bus1.Inputs.Panel.Instances.Button"}"#,
        &SendOptions::default().response_topic("test/old-input-refresh")
    ));
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
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
    pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/old-input-refresh")
                .is_empty())
            .await
    );
    let reply: serde_json::Value =
        serde_json::from_slice(&broker.received_on("test/old-input-refresh")[0].payload).unwrap();
    assert!(
        reply.get("error").is_some(),
        "retired input query was reinterpreted under its new mapping"
    );
    assert_eq!(projected(&handle, "Door")["Quality"], "unobserved");
    assert!(
        matches!(
            handle
                .read_input(&target("Door"), context(1), deadline())
                .await,
            Err(ServiceError::Rejected(_))
        ),
        "old generation admitted a renamed input read"
    );
    assert!(running.stop().await);
    peer.close().await;
}

#[tokio::test]
async fn input_readback_ready_reply_is_rejected_if_caller_resumes_after_its_deadline() {
    use std::{future::Future, task::Poll};
    let (fixture, scenario) = configured();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let name = target("Button");
    let until = Instant::now() + Duration::from_millis(150);
    let mut read = Box::pin(handle.read_input(&name, context(1), until));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    wait_for(&handle, |s| s.inputs.contains_key(&name)).await;
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(read.await, Err(ServiceError::Rejected(_))),
        "ready input reply bypassed original caller expiry"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn input_readback_shutdown_cancels_and_joins_a_pending_query() {
    let (fixture, _) = configured();
    let mut peer = peer::Peer::start(1, 1, vec![255], Duration::ZERO, true);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| s.input_listeners.contains(&target("Button"))).await;
    let worker = handle.clone();
    let read = tokio::spawn(async move {
        worker
            .read_input(&target("Button"), context(1), deadline())
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    assert!(
        running.stop().await,
        "input helper prevented managed shutdown"
    );
    assert!(matches!(read.await.unwrap(), Err(ServiceError::Closed)));
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Closed);
    peer.close().await;
}
