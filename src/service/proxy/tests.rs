use super::*;
use crate::atx::transactions::RepeatPolicy;
use crate::emulator::{pty, session::Control};
use crate::service::tests::{context, start, wait_for, Fixture};
use mqtt_test_broker::{FakeBroker, SendOptions};
use serde_json::json;
use std::time::Duration;
mod colour;
mod commissioning;
mod discovery;
mod feedback;
mod group_scalar;

async fn refresh(broker: &FakeBroker, topic: &str) -> Value {
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"target":"Bus1.Outputs.Light1","command":"refresh"}"#,
        &SendOptions::default().response_topic(topic)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b.received_on(topic).is_empty())
            .await
    );
    serde_json::from_slice(&broker.received_on(topic)[0].payload).unwrap()
}

async fn wait_brightness(broker: &FakeBroker, target: &str, brightness: f64) {
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(&format!("DALI/State/Controller/{target}"))
                .last()
                .is_some_and(|r| {
                    let value: Value = serde_json::from_slice(&r.payload).unwrap();
                    value["Brightness"] == json!(brightness) && value["Available"] == true
                }))
            .await,
        "missing fixture failure blocked a healthy MQTT output or produced incorrect brightness"
    );
}

#[tokio::test]
async fn unchanged_clamped_readback_precedes_ack_and_remains_the_reconnect_basis() {
    use crate::service::tests::{deadline, Pause};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fixture = Fixture::new();
    let mut pty = crate::service::hat::tests::Pty::new();
    let path = pty.path.clone();
    let writes = Arc::new(AtomicUsize::new(0));
    let counted = writes.clone();
    // Independent literal HAT: this fixture's maximum is 127. A requested 254
    // succeeds on the wire but actual-level queries continue to answer 127.
    let peer = tokio::spawn(async move {
        loop {
            let line = tokio::time::timeout(Duration::from_secs(12), async {
                let mut bytes = Vec::new();
                loop {
                    let byte = pty.peer.read_u8().await.unwrap();
                    bytes.push(byte);
                    assert!(bytes.len() < 32);
                    if byte == b'\n' {
                        break bytes;
                    }
                }
            })
            .await
            .expect("clamp test HAT stopped receiving bounded requests");
            let response: &[u8] = match line.as_slice() {
                b"v\n" => b"V013204\n",
                b"d\n" => b"D24\n",
                b"h01A0\n" => b"J7F\n",
                b"h0190\n" => b"J04\n",
                b"h00FE\n" => {
                    counted.fetch_add(1, Ordering::SeqCst);
                    b"N\n"
                }
                other => panic!("unexpected independent HAT request: {other:?}"),
            };
            pty.peer.write_all(response).await.unwrap();
        }
    });
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let pause = Arc::new(Pause::default());
    service.proxy_snapshot_pause = Some(pause.clone());
    let running = start(
        service.with_uart(path, RepeatPolicy::Unverified),
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
    assert!(refresh(&broker, "test/clamp-initial")
        .await
        .get("error")
        .is_none());
    wait_brightness(&broker, "Bus1.Outputs.Light1", 0.5).await;
    pause.armed.store(true, Ordering::Release);
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("pause snapshot helper".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
    let desired = "DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness";
    let options = SendOptions::default()
        .expiry(3600)
        .user_property("target-generation", context(1).generation().unwrap())
        .user_property("conflict", "DeviceWins");
    assert!(broker.send_with(desired, "1.0", &options));
    // The command reply is a worker-order barrier after apply and its written
    // bookkeeping. The paused helper cannot repair a missing apply report.
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"describe","description":"after clamp","expected_revision":2}"#,
        &SendOptions::default()
            .user_property("target-generation", context(2).generation().unwrap())
            .response_topic("test/clamp-applied")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/clamp-applied")
                .is_empty())
            .await
    );
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    // Retain only for the next subscription. A new connection is causally after
    // the old pump's synchronous broker_lost snapshot of known/written state.
    broker.retain(desired, "1.0", &options);
    let connections = broker
        .events()
        .iter()
        .filter(|e| matches!(e, mqtt_test_broker::BrokerEvent::Connected))
        .count();
    broker.drop_connection();
    assert!(
        broker
            .wait_until(Duration::from_secs(8), |b| b
                .events()
                .iter()
                .filter(|e| matches!(e, mqtt_test_broker::BrokerEvent::Connected))
                .count()
                > connections)
            .await
    );
    assert!(broker.wait_until(Duration::from_secs(8), |_| writes.load(Ordering::SeqCst) == 2).await,
        "unchanged actual readback was replaced by inferred intent before ack; DeviceWins superseded a valid replay");
    wait_brightness(&broker, "Bus1.Outputs.Light1", 0.5).await;
    pause.release.notify_one();
    assert!(running.stop().await);
    peer.abort();
    assert!(peer.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn mqtt_output_keeps_original_read_write_trace_and_clears_unsolicited_cause() {
    let fixture = Fixture::new();
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let read_trace = "00-11111111111111111111111111111111-2222222222222222-01";
    let write_trace = "00-33333333333333333333333333333333-4444444444444444-01";
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"target":"Bus1.Outputs.Light1","command":"refresh"}"#,
        &SendOptions::default()
            .response_topic("test/traced-read")
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property("traceparent", read_trace)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/traced-read")
                .is_empty())
            .await
    );
    wait_brightness(&broker, "Bus1.Outputs.Light1", 100.0 / 254.0).await;
    assert_eq!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
            .last()
            .unwrap()
            .user_property("traceparent"),
        Some(read_trace),
        "physical refresh lost original trace across the service queue without OTel"
    );
    assert_eq!(
        broker.received_on("test/traced-read")[0].user_property("traceparent"),
        Some(read_trace)
    );
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness",
        "0.5",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property("traceparent", write_trace)
    ));
    wait_brightness(&broker, "Bus1.Outputs.Light1", 0.5).await;
    assert_eq!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
            .last()
            .unwrap()
            .user_property("traceparent"),
        Some(write_trace),
        "physical write readback lost original trace across the service queue without OTel"
    );
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|r| serde_json::from_slice::<Value>(&r.payload)
                    .unwrap()["Quality"]
                    == "stale"))
            .await
    );
    assert_eq!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
            .last()
            .unwrap()
            .user_property("traceparent"),
        None,
        "unsolicited bus invalidation inherited an earlier request cause"
    );
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: true,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    wait_for(&handle, |s| matches!(&s.hardware, Hardware::Ready { buses, .. } if buses.iter().any(|bus|bus.bus==0&&bus.status==2))).await;
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/PowerState",
        "false",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
    ));
    wait_brightness(&broker, "Bus1.Outputs.Light1", 0.0).await;
    assert_eq!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
            .last()
            .unwrap()
            .user_property("traceparent"),
        None,
        "headerless physical write fabricated or inherited a trace"
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 0);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn missing_mqtt_fixture_keeps_last_known_values_and_does_not_block_healthy_fixture() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(refresh(&broker, "test/present-fixture")
        .await
        .get("error")
        .is_none());
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "ceiling-a".into(),
                short_address: None,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(refresh(&broker, "test/missing-fixture")
        .await
        .get("error")
        .is_none());
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|r| {
                    let value: Value = serde_json::from_slice(&r.payload).unwrap();
                    value["Available"] == false
                        && value["Quality"] == "unavailable"
                        && value["PowerState"] == true
                        && value["Brightness"] == json!(100.0 / 254.0)
                }))
            .await,
        "missing fixture fabricated off or lost its explicitly last-known data"
    );
    let before = broker.received_on("DALI/Error/Controller").len();
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/PowerState",
        "false",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Error/Controller")
                .len()
                > before)
            .await,
        "missing fixture write was incorrectly acknowledged as applied"
    );
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light2/Brightness",
        "1.0",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
    ));
    wait_brightness(&broker, "Bus1.Outputs.Light2", 1.0).await;
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().buses[0]
            .outputs
            .len(),
        2
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][1]["level"], 254);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn invalid_atomic_maps_and_retired_tokens_never_partially_change_a_fixture() {
    let fixture = Fixture::new();
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+", Duration::from_secs(2))
            .await
    );
    let current =
        SendOptions::default().user_property("target-generation", context(1).generation().unwrap());
    let stale = SendOptions::default().user_property("target-generation", "retired.1");
    for (index, (payload, options)) in [
        (r#"{"Brightness":0.25,"Other":1}"#, current.clone()),
        (r#"{"PowerState":false,"Brightness":0.25}"#, current.clone()),
        (r#"{"PowerState":null}"#, current),
        (r#"{"Brightness":0.25}"#, SendOptions::default()),
        (r#"{"Brightness":0.25}"#, stale),
    ]
    .into_iter()
    .enumerate()
    {
        let before = broker.received_on("DALI/Error/Controller").len();
        assert!(broker.send_with(
            "DALI/Desired/Controller/Bus1.Outputs.Light1",
            payload,
            &options
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on("DALI/Error/Controller")
                    .len()
                    > before)
                .await,
            "invalid complete request was accepted: {payload}"
        );
        // Rejections report one Error per property. The first Error cannot
        // synchronize this loop: later pending values coalesce by property.
        // Finish each request before submitting the next invalid complete map.
        assert!(
            refresh(&broker, &format!("test/after-invalid-intent/{index}"))
                .await
                .get("error")
                .is_none()
        );
        if index == 0 {
            assert!(
                broker
                    .wait_until(Duration::from_secs(1), |b| b
                        .received_on("DALI/Error/Controller")
                        .len()
                        >= before + 2)
                    .await,
                "two-property rejection did not expose both property errors"
            );
        }
    }
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 100,
        "invalid or retired atomic intent partially changed the physical fixture"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[test]
fn answered_mask_is_not_current_mqtt_availability_or_invented_power() {
    use crate::atx::transactions::BusReply;
    use crate::dali::gear::Query;
    let mut snapshot =
        crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    snapshot.hardware = Hardware::Ready {
        version: crate::atx::wire::Version {
            hardware: "01".into(),
            firmware: "32".into(),
            variant: "04".into(),
        },
        buses: vec![BusHealth {
            bus: 0,
            status: 2,
            channels: 4,
        }],
    };
    let observation = output::Observation {
        colour: None,
        context: context(1),
        actual_level: Some(Query::ActualLevel.decode(BusReply::Value(255))),
        status: Some(Query::Status.decode(BusReply::Value(4))),
        quality: output::Quality::Partial,
        reason: None,
        stamp: output::Stamp {
            session: 1,
            actor: 1,
            bus: 1,
            observed_bus: 1,
        },
        bus: 0,
    };
    assert!(
        observation.available(),
        "fixture did not establish the answered-MASK distinction"
    );
    snapshot
        .outputs
        .insert("Bus1.Outputs.Light1".into(), observation);
    let values = &state::projections(&snapshot)[0].state.values;
    assert_eq!(
        values["Available"], false,
        "answered MASK became usable MQTT availability"
    );
    assert_eq!(values["Quality"], "partial");
    assert!(
        !values.contains_key("PowerState"),
        "MASK invented a power value"
    );
    assert!(
        !values.contains_key("Brightness"),
        "MASK invented brightness"
    );
}

#[test]
fn mqtt_output_intent_rejects_the_whole_invalid_map() {
    for payload in [
        "{}",
        r#"{"Brightness":null}"#,
        r#"{"PowerState":0}"#,
        r#"{"Brightness":"0.5"}"#,
        r#"{"Brightness":2}"#,
        r#"{"PowerState":false,"Brightness":0.5}"#,
        r#"{"Brightness":0.5,"Other":1}"#,
    ] {
        let values: PropertyMap = serde_json::from_str(payload).unwrap();
        assert!(
            state::intent(&values, None).is_err(),
            "invalid output map was partially accepted: {payload}"
        );
    }
}

#[tokio::test]
async fn all_refresh_without_ready_uart_never_takes_physical_admission() {
    use std::{future::Future, task::Poll};
    let fixture = Fixture::new();
    let (mut service, handle) = fixture.service();
    let snapshot = crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    service.state.send_replace(Some(snapshot));
    let mut proxy = DaliProxy {
        initial: routing::generations(&handle.snapshot().unwrap().model),
        handle: handle.clone(),
        reporter: None,
        read_cursor: 0,
        read_pause: None,
        snapshot_pause: None,
    };
    let cause = RequestContext::default();
    let mut reading = Box::pin(proxy.read_all(&cause));
    let polled = std::future::poll_fn(|cx| Poll::Ready(reading.as_mut().poll(cx))).await;
    assert!(
        matches!(polled, Poll::Ready(Err(OpError::Rejected(_)))),
        "All refresh queued physical work despite unavailable UART: {polled:?}"
    );
    drop(reading);
    assert!(service.receive.try_recv().is_err());
    assert_eq!(
        handle.permit.available_permits(),
        1,
        "unavailable-UART polling blocked metadata admission"
    );
}

#[tokio::test]
async fn disabled_bus_inventory_remains_readable_without_physical_admission() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses[0].enabled = false;
    let (mut service, handle) = fixture.service();
    service
        .state
        .send_replace(Some(crate::service::tests::snapshot(
            Model::new(inventory).unwrap(),
            Phase::Ready,
        )));
    let mut proxy = DaliProxy {
        initial: routing::generations(&handle.snapshot().unwrap().model),
        handle: handle.clone(),
        reporter: None,
        read_cursor: 0,
        read_pause: None,
        snapshot_pause: None,
    };
    let states = proxy
        .read_all(&RequestContext::default())
        .await
        .expect("disabled bus queued physical work instead of returning metadata");
    let light = states
        .iter()
        .find(|value| value.state.target == "Bus1.Outputs.Light1")
        .unwrap();
    assert_eq!(light.state.values["Available"], false);
    assert!(!light.state.values.contains_key("PowerState"));
    assert!(
        service.receive.try_recv().is_err(),
        "disabled bus queued physical work"
    );
    assert_eq!(handle.permit.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn all_read_clips_each_fixture_to_the_original_aggregate_ceiling() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
    fixture.add_output(2, "Light3");
    let (mut service, handle) = fixture.service();
    let mut snapshot =
        crate::service::tests::snapshot(Model::new(fixture.disk()).unwrap(), Phase::Ready);
    snapshot.hardware = Hardware::Ready {
        version: crate::atx::wire::Version {
            hardware: "01".into(),
            firmware: "32".into(),
            variant: "04".into(),
        },
        buses: vec![BusHealth {
            bus: 0,
            status: 2,
            channels: 4,
        }],
    };
    service.state.send_replace(Some(snapshot));
    let mut proxy = DaliProxy {
        initial: routing::generations(&handle.snapshot().unwrap().model),
        handle,
        reporter: None,
        read_cursor: 0,
        read_pause: None,
        snapshot_pause: None,
    };
    let original = Instant::now() + Duration::from_secs(15);
    let actor = tokio::spawn(async move {
        let mut deadlines = Vec::new();
        for seconds in [14, 2] {
            let Message::Output(request) = service.receive.recv().await.unwrap() else {
                panic!("fixture did not get physical work")
            };
            deadlines.push(request.deadline);
            let output::Request { reply, _permit, .. } = request;
            // Model actual owner retirement before waking an expired frontend.
            drop(_permit);
            tokio::time::advance(Duration::from_secs(seconds)).await;
            let _ = reply.send(Err(ServiceError::Expired));
        }
        deadlines
    });
    let result = proxy.read_all(&RequestContext::default()).await;
    assert!(matches!(result, Err(OpError::Rejected(_))));
    let deadlines = tokio::time::timeout(Duration::from_secs(1), actor)
        .await
        .expect("aggregate fixture did not receive its two real service requests")
        .unwrap();
    assert_eq!(deadlines.len(), 2);
    assert_eq!(
        deadlines[1], original,
        "next fixture received a fresh window beyond the aggregate deadline"
    );
    assert_eq!(
        proxy.read_cursor, 2,
        "expired aggregate attempted the third fixture"
    );
}

#[tokio::test]
async fn a_held_mqtt_read_cannot_overwrite_an_already_published_bus_outage() {
    use crate::service::tests::Pause;
    let fixture = Fixture::new();
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let pause = std::sync::Arc::new(Pause::default());
    service.proxy_read_pause = Some(pause.clone());
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
    assert!(refresh(&broker, "test/before-outage")
        .await
        .get("error")
        .is_none());
    pause
        .armed
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"target":"Bus1.Outputs.Light1","command":"refresh"}"#,
        &SendOptions::default().response_topic("test/held-read")
    ));
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|r| {
                    let value: Value = serde_json::from_slice(&r.payload).unwrap();
                    value["Available"] == false
                        && value["Quality"] == "stale"
                        && value["PowerState"] == true
                }))
            .await,
        "snapshot helper did not publish a bus outage while the driver read was suspended"
    );
    let before_release = broker
        .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
        .len();
    pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/held-read")
                .is_empty())
            .await
    );
    let reply: Value =
        serde_json::from_slice(&broker.received_on("test/held-read")[0].payload).unwrap();
    assert!(reply["error"].as_str().is_some_and(|text| text.contains("evidence changed")),
        "held read accepted retired physical evidence after MQTT marked the output unavailable: {reply}");
    assert!(
        broker
            .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
            .iter()
            .skip(before_release)
            .all(|r| { serde_json::from_slice::<Value>(&r.payload).unwrap()["Available"] != true }),
        "held read resurrected current MQTT availability"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn cancellation_during_the_first_all_read_keeps_cursor_progress_across_disconnect() {
    use crate::service::tests::Pause;
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Status/Controller")
                .iter()
                .any(|r| r.payload == "connected"))
            .await
    );
    let pause = std::sync::Arc::new(Pause::default());
    pause
        .armed
        .store(true, std::sync::atomic::Ordering::Release);
    let mut proxy = DaliProxy {
        handle: handle.clone(),
        initial: routing::generations(&handle.snapshot().unwrap().model),
        reporter: None,
        read_cursor: 0,
        read_pause: Some(pause.clone()),
        snapshot_pause: None,
    };
    let cause = RequestContext::default().with_traceparent(None);
    let mut reading = Box::pin(proxy.read_all(&cause));
    tokio::select! {
        result=&mut reading=>panic!("first fixture did not hold the actual driver await: {result:?}"),
        result=tokio::time::timeout(Duration::from_secs(1),pause.entered.notified())=>result.unwrap(),
    }
    drop(reading);
    assert_eq!(
        proxy.read_cursor, 1,
        "canceled first fixture stranded the All-read cursor"
    );
    proxy.disconnect().await;
    assert_eq!(
        proxy.read_cursor, 1,
        "disconnect erased All-read cursor progress"
    );
    let states = proxy.read_all(&cause).await.unwrap();
    assert_eq!(
        states[1].state.target, "Bus1.Outputs.Light2",
        "next pass restarted from the canceled first fixture"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn all_read_revalidates_earlier_samples_after_its_final_fixture_await() {
    use crate::service::tests::Pause;
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Status/Controller")
                .iter()
                .any(|r| r.payload == "connected"))
            .await
    );
    let pause = std::sync::Arc::new(Pause::default());
    pause
        .armed
        .store(true, std::sync::atomic::Ordering::Release);
    let mut proxy = DaliProxy {
        handle: handle.clone(),
        initial: routing::generations(&handle.snapshot().unwrap().model),
        reporter: None,
        read_cursor: 0,
        read_pause: Some(pause.clone()),
        snapshot_pause: None,
    };
    let cause = RequestContext::default().with_traceparent(None);
    let mut reading = Box::pin(proxy.read_all(&cause));
    tokio::select! {
        result=&mut reading=>panic!("first fixture did not reach controlled suspension: {result:?}"),
        result=tokio::time::timeout(Duration::from_secs(1),pause.entered.notified())=>result.unwrap(),
    }
    for powered in [false, true] {
        emulator
            .emulator
            .control(
                Control::BusPower { bus: 0, powered },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
    }
    wait_for(&handle, |s| s.outputs.get("Bus1.Outputs.Light1").is_some_and(|value| value.quality == output::Quality::Stale)
        && matches!(&s.hardware, Hardware::Ready { buses, .. } if buses.iter().any(|bus|bus.bus==0&&bus.status==2))).await;
    pause.release.notify_one();
    let result = reading.await;
    assert!(
        matches!(result, Err(OpError::Rejected(_))),
        "All read accepted a retired early sample after later fixture I/O: {result:?}"
    );
    assert_eq!(
        handle.snapshot().unwrap().outputs["Bus1.Outputs.Light2"].quality,
        output::Quality::Current,
        "fixture did not actually sample the later fixture after the bus cycle"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn snapshot_helper_cannot_skip_a_return_to_its_previous_value_after_driver_readback() {
    use crate::service::tests::{deadline, Pause};
    let fixture = Fixture::new();
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let pause = std::sync::Arc::new(Pause::default());
    service.proxy_snapshot_pause = Some(pause.clone());
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
    assert!(refresh(&broker, "test/initial")
        .await
        .get("error")
        .is_none());
    pause
        .armed
        .store(true, std::sync::atomic::Ordering::Release);
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("wake helper".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
    handle
        .apply_output(
            "Bus1.Outputs.Light1",
            crate::dali::gear::Intent::new(None, Some(0.5)).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(refresh(&broker, "test/newer-driver-value")
        .await
        .get("error")
        .is_none());
    assert!(
        broker
            .wait_until(Duration::from_secs(1), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|r| {
                    serde_json::from_slice::<Value>(&r.payload).unwrap()["Brightness"] == 0.5
                }))
            .await
    );
    handle
        .apply_output(
            "Bus1.Outputs.Light1",
            crate::dali::gear::Intent::new(None, Some(100.0 / 254.0)).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    pause.release.notify_one();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|r| {
                    serde_json::from_slice::<Value>(&r.payload).unwrap()["Brightness"]
                        == json!(100.0 / 254.0)
                }))
            .await,
        "helper's private projection cache hid a newer physical return to its previous value"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn mqtt_output_refresh_returns_actual_fixture_state() {
    let fixture = Fixture::new();
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        "DALI/Command/Controller",
        r#"{"target":"Bus1.Outputs.Light1","command":"refresh"}"#,
        &SendOptions::default().response_topic("test/output-refresh"),
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/output-refresh")
                .is_empty())
            .await
    );
    let replies = broker.received_on("test/output-refresh");
    let reply: Value = serde_json::from_slice(&replies[0].payload).unwrap();
    assert!(
        reply.get("error").is_none(),
        "valid fixture refresh was refused: {reply}"
    );
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| {
                b.received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                    .iter()
                    .any(|r| {
                        let Ok(value) = serde_json::from_slice::<Value>(&r.payload) else {
                            return false;
                        };
                        value["Brightness"] == json!(100.0 / 254.0)
                            && value["PowerState"] == true
                            && value["Available"] == true
                            && value["Quality"] == "current"
                            && r.user_property("target-generation") == context(1).generation()
                    })
            })
            .await,
        "fixture refresh did not publish actual current readback"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn mqtt_output_brightness_changes_independent_physical_state() {
    let fixture = Fixture::new();
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
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness",
        "1.0",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap()),
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| {
                b.received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                    .iter()
                    .any(|r| {
                        let Ok(value) = serde_json::from_slice::<Value>(&r.payload) else {
                            return false;
                        };
                        value["Brightness"] == 1.0
                            && value["PowerState"] == true
                            && value["Available"] == true
                    })
            })
            .await,
        "valid MQTT brightness did not reach physical readback"
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"],
        json!(254),
        "MQTT state echoed brightness without changing the independent fixture"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
mod web_commands;
