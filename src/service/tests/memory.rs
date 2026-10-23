//! Managed reads through the real owner and independent TCP HAT model.
use super::*;
use crate::service::scenes;
use crate::{
    atx::transactions::RepeatPolicy,
    dali::memory::{Layout, Window},
    emulator::session::{Control, Emulator},
};
const OUTPUT: &str = "Bus1.Outputs.Light1";
const INPUT: &str = "Bus1.Inputs.Panel";
fn limit() -> Instant {
    Instant::now() + Duration::from_secs(8)
}
fn scenario(profile: &str) -> crate::emulator::scenario::Scenario {
    serde_json::from_value(json!({"version":1,"clock":"frozen","repeat":profile,"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":91,"groups":1,"memory":{"version":1,"banks":[
            {"bank":0,"bytes":[14,255,255,0,1,2,3,4,5,6,7,137,171,205,239]},
            {"bank":255,"bytes":[255,null,0,127]}]}}],
        "inputs":[{"id":"panel","short_address":0,"instances":[1],"memory":{"version":8,"banks":[
            {"bank":0,"bytes":[26,0,255,0,1,2,3,4,5,6,7,254,220,186,152,118,84,50,16,2,3,0,0,0,0,0,0]}]}}]}]})).unwrap()
}
async fn native(emulator: &Emulator) -> serde_json::Value {
    let mut value = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    // Scratch DTR changes are intrinsic to native memory reads. All persistent
    // settings, values, identities and other runtime state must remain equal.
    for bus in value["buses"].as_array_mut().unwrap() {
        for gear in bus["outputs"].as_array_mut().unwrap() {
            gear.as_object_mut().unwrap().remove("dtr");
        }
        for input in bus["inputs"].as_array_mut().unwrap() {
            for key in ["dtr0", "dtr1", "dtr2"] {
                input.as_object_mut().unwrap().remove(key);
            }
        }
    }
    value
}
#[tokio::test]
async fn memory_managed_tcp_keeps_spaces_ff_holes_identity_and_existing_observations() {
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
    ] {
        let fixture = Fixture::new();
        fixture.add_button();
        let original = fixture.current_bytes();
        let (emulator, address) =
            Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario(profile))
                .await
                .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service.with_hat_tcp(address, policy), &handle, &broker).await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        handle
            .read_output(OUTPUT, context(1), limit())
            .await
            .unwrap();
        handle
            .read_group_output("Bus1.Groups.All", context(1), limit())
            .await
            .unwrap();
        let before = handle.snapshot().unwrap();
        let physical = native(&emulator).await;
        let raw = handle
            .read_memory(
                OUTPUT,
                255,
                Window::new(0, 4).unwrap(),
                RequestContext::default(),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(raw.quality, "partial");
        assert_eq!(raw.target_generation, context(1).generation().unwrap());
        assert_eq!(
            serde_json::to_value(&raw.readings).unwrap(),
            json!([
            {"address":0,"reading":{"kind":"value","raw":255}},
            {"address":1,"reading":{"kind":"no_response"}},
            {"address":2,"reading":{"kind":"value","raw":0}},
            {"address":3,"reading":{"kind":"value","raw":127}}]),
            "memory raw bytes shifted or FF became MASK"
        );
        let old = handle
            .read_identity(OUTPUT, context(1), limit())
            .await
            .unwrap();
        assert_eq!(old.quality, "complete");
        assert_eq!(old.count, 15);
        let old = old.identity.unwrap();
        assert_eq!(old.layout, Layout::Legacy);
        assert_eq!(old.serial_number.as_deref(), Some("89ABCDEF"));
        assert_eq!(old.serial_bits, Some(32));
        assert_eq!(old.hardware_version, None);
        assert_eq!(old.firmware_version, Some([6, 7]));
        let current = handle
            .read_identity(INPUT, context(1), limit())
            .await
            .unwrap();
        assert_eq!(current.quality, "complete");
        assert_eq!(current.count, 27);
        let current = current.identity.unwrap();
        assert_eq!(current.layout, Layout::Modern);
        assert_eq!(current.serial_number.as_deref(), Some("FEDCBA9876543210"));
        assert_eq!(current.serial_bits, Some(64));
        assert_eq!(current.hardware_version, Some([2, 3]));
        assert_eq!(
            current.last_memory_bank,
            Some(255),
            "single-byte FF was globally masked"
        );
        let absent = handle
            .read_memory(
                INPUT,
                255,
                Window::new(254, 1).unwrap(),
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(absent.quality, "unavailable");
        assert_eq!(
            absent.readings[0].reading,
            Some(scenes::Reading::NoResponse {})
        );
        let after = handle.snapshot().unwrap();
        assert_eq!(
            before.outputs, after.outputs,
            "memory read invalidated output evidence"
        );
        assert_eq!(
            before.group_outputs, after.group_outputs,
            "memory read invalidated group evidence"
        );
        assert_eq!(
            native(&emulator).await,
            physical,
            "memory changed native state other than scratch DTRs"
        );
        assert_eq!(
            fixture.current_bytes(),
            original,
            "memory changed saved identity/bindings"
        );
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
#[tokio::test]
async fn memory_identity_unknown_truncated_reserved_and_absent_evidence_stays_honest() {
    let fixture = Fixture::new();
    for (a, name) in [(1, "Truncated"), (2, "Reserved"), (3, "Absent")] {
        fixture.add_output(a, name);
    }
    let mut scenario = serde_json::to_value(scenario("one_terminal")).unwrap();
    scenario["buses"][0]["outputs"][0]["memory"]["version"] = json!(7);
    let gear = |id, address, bytes| json!({"id":id,"short_address":address,"memory":{"banks":[{"bank":0,"bytes":bytes}]}});
    scenario["buses"][0]["outputs"]
        .as_array_mut()
        .unwrap()
        .extend([
            gear("truncated", 1, json!([4, 255, 0, 0, 1])),
            gear("reserved", 2, json!([255])),
            json!({"id":"absent","short_address":3}),
        ]);
    let (emulator, address) = Emulator::listen_tcp(
        "127.0.0.1:0".parse().unwrap(),
        serde_json::from_value(scenario).unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_hat_tcp(address, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let r = handle
        .read_identity(OUTPUT, context(1), limit())
        .await
        .unwrap();
    assert_eq!(r.quality, "partial");
    assert_eq!(r.count, 11);
    let identity = r.identity.unwrap();
    assert_eq!(identity.layout, Layout::Unknown);
    assert_eq!(identity.serial_number, None);
    assert_eq!(identity.serial_bits, None);
    assert!(identity.gtin.is_some());
    let r = handle
        .read_identity("Bus1.Outputs.Truncated", context(1), limit())
        .await
        .unwrap();
    assert_eq!(r.quality, "partial");
    assert_eq!(r.count, 5);
    assert_eq!(r.identity.unwrap().serial_number, None);
    let r = handle
        .read_identity("Bus1.Outputs.Reserved", context(1), limit())
        .await
        .unwrap();
    assert_eq!(r.quality, "partial");
    assert!(r.reason.is_some());
    assert!(r.identity.is_none());
    assert_eq!(
        r.readings[0].reading,
        Some(scenes::Reading::Value { raw: 255 })
    );
    let r = handle
        .read_identity("Bus1.Outputs.Absent", context(1), limit())
        .await
        .unwrap();
    assert_eq!(r.quality, "unavailable");
    assert!(r.identity.is_none());
    assert_eq!(r.readings[0].reading, Some(scenes::Reading::NoResponse {}));
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}
#[tokio::test]
async fn memory_ready_reply_keeps_original_deadline_revision_and_physical_epoch() {
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    for reason in ["deadline", "revision", "epoch"] {
        let fixture = Fixture::new();
        let (emulator, address) =
            Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario("one_terminal"))
                .await
                .unwrap();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        if reason == "epoch" {
            service.output_acceptance_hook =
                Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        }
        let running = start(
            service.with_hat_tcp(address, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let until = Instant::now() + Duration::from_millis(350);
        let mut read = Box::pin(handle.read_memory(
            OUTPUT,
            255,
            Window::new(0, 1).unwrap(),
            context(1),
            until,
        ));
        poll_fn(|cx| {
            assert!(read.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        wait_for(&handle, |_| handle.permit.available_permits() == 1).await;
        if reason == "deadline" {
            tokio::time::sleep_until(until + Duration::from_millis(10)).await;
        }
        if reason == "revision" {
            handle
                .edit(
                    1,
                    MetadataEdit::Description(Some("new revision".into())),
                    context(1),
                    limit(),
                )
                .await
                .unwrap();
        }
        assert!(
            read.await.is_err(),
            "memory accepted retired {reason} evidence"
        );
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
#[tokio::test]
async fn memory_mqtt_strict_read_class_and_original_headers_without_mutation_completion() {
    let fixture = Fixture::new();
    fixture.add_button();
    let (emulator, address) =
        Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario("one_terminal"))
            .await
            .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_hat_tcp(address, RepeatPolicy::Unverified),
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
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    for (index, (target, command, params, success)) in [
        (
            OUTPUT,
            "read_memory",
            json!({"bank":255,"start":0,"count":4}),
            true,
        ),
        (INPUT, "read_identity", json!({}), true),
        (OUTPUT, "read_identity", json!({"extra":true}), false),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":0,"start":254,"count":2}),
            false,
        ),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":0,"start":0,"count":0}),
            false,
        ),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":0,"start":0,"count":65}),
            false,
        ),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":256,"start":0,"count":1}),
            false,
        ),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":0,"start":0.5,"count":1}),
            false,
        ),
        (
            OUTPUT,
            "read_memory",
            json!({"bank":0,"start":0,"count":1,"extra":true}),
            false,
        ),
        (OUTPUT, "read_memory", json!({"start":0,"count":1}), false),
        ("Bus1.Groups.All", "read_identity", json!({}), false),
        (
            "Bus1.Inputs.Panel.Instances.Button",
            "read_identity",
            json!({}),
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let topic = format!("test/memory/{index}");
        let mut call = json!({"command":command,"target":target});
        call.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
        let options = SendOptions::default()
            .response_topic(&topic)
            .user_property("traceparent", trace);
        let options = if index == 0 {
            options
        } else {
            options.user_property("target-generation", context(1).generation().unwrap())
        };
        assert!(broker.publish("DALI/Command/Controller", call.to_string(), &options));
        assert!(
            broker
                .wait_until(Duration::from_secs(3), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await
        );
        let replies = broker.received_on(&topic);
        assert_eq!(replies.len(), 1);
        let value: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(
            value.get("error").is_none(),
            success,
            "memory read-class/strict parameter mismatch: {value}"
        );
        if success {
            assert_eq!(value["value"]["outcome"], "observed");
            assert_eq!(replies[0].user_property("traceparent"), Some(trace));
            let original = context(1);
            assert_eq!(
                replies[0].user_property("target-generation"),
                if index == 0 {
                    None
                } else {
                    original.generation()
                }
            );
        }
    }
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn memory_literal_uart_reloads_every_offset_and_retains_collision_evidence() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut pty = crate::service::hat::tests::Pty::new();
    let path = pty.path.clone();
    let done = Arc::new(tokio::sync::Notify::new());
    let finished = done.clone();
    let peer = tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(&mut pty.peer);
        let mut read = BufReader::new(read);
        let mut script = std::collections::VecDeque::from([
            ("hC3FF\n", "N\n"),
            ("hA30A\n", "N\n"),
            ("h01C5\n", "JFF\n"),
            ("hA30B\n", "N\n"),
            ("h01C5\n", "N\n"),
            ("hA30C\n", "N\n"),
            ("h01C5\n", "X2\n"),
            ("hA30D\n", "N\n"),
            ("h01C5\n", "Z9\n"),
            ("hA30E\n", "N\n"),
            ("h01C5\n", "J00\n"),
        ]);
        loop {
            let mut line = String::new();
            if read.read_line(&mut line).await.unwrap() == 0 {
                return;
            }
            let response = match line.as_str() {
                "v\n" => "V013204\n",
                "d\n" => "D24\n",
                _ => {
                    let (expected, response) =
                        script.pop_front().expect("unexpected memory HAT command");
                    assert_eq!(
                        line, expected,
                        "memory wire plan selected wrong bank/location"
                    );
                    response
                }
            };
            write.write_all(response.as_bytes()).await.unwrap();
            if script.is_empty() {
                finished.notify_one();
            }
        }
    });
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .read_memory(
            OUTPUT,
            255,
            Window::new(10, 5).unwrap(),
            context(1),
            limit(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), done.notified())
        .await
        .unwrap();
    assert_eq!(
        result
            .readings
            .into_iter()
            .map(|item| item.reading.unwrap())
            .collect::<Vec<_>>(),
        vec![
            scenes::Reading::Value { raw: 255 },
            scenes::Reading::NoResponse {},
            scenes::Reading::ReceiveCollision { code: Some(2) },
            scenes::Reading::TransmitCollision { code: Some(9) },
            scenes::Reading::Value { raw: 0 }
        ],
        "memory replies lost raw/collision distinctions"
    );
    assert!(running.stop().await);
    peer.abort();
    assert!(peer.await.unwrap_err().is_cancelled());
}
