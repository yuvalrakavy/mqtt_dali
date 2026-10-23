//! Part251 reads through real managed MQTT and the independent TCP HAT.
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::session::{Control, Emulator},
};
const TARGET: &str = "Bus1.Outputs.Light1";
#[tokio::test]
async fn luminaire_data_literal_uart_keeps_format_proof_holes_and_collision_locations() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    for format_collision in [false, true] {
        let scenario: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/emulator/luminaire-i4.json"))
                .unwrap();
        let bytes = scenario["buses"][0]["outputs"][0]["memory"]["banks"][1]["bytes"]
            .as_array()
            .unwrap();
        let count = if format_collision { 19 } else { 120 };
        let mut script =
            std::collections::VecDeque::from([("hC301\n".to_string(), "N\n".to_string())]);
        for (offset, byte) in bytes.iter().enumerate().take(count) {
            script.push_back((format!("hA3{offset:02X}\n"), "N\n".into()));
            let response = match offset {
                5 => "N\n".into(),
                18 if format_collision => "X2\n".into(),
                34 => "Z9\n".into(),
                70 => "X2\n".into(),
                _ => format!("J{:02X}\n", byte.as_u64().unwrap()),
            };
            script.push_back(("h01C5\n".into(), response));
        }
        let mut pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let done = Arc::new(tokio::sync::Notify::new());
        let finished = done.clone();
        let peer = tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            loop {
                let mut line = String::new();
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let response = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        let (expected, response) = script
                            .pop_front()
                            .expect("unexpected literal luminaire memory command");
                        assert_eq!(line, expected, "luminaire bank/offset/read literal changed");
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
        let saved = fixture.current_bytes();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .read_luminaire_data(TARGET, context(1), Instant::now() + Duration::from_secs(8))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
        assert_eq!(usize::from(report.count), count);
        assert_eq!(report.quality, "partial");
        assert_eq!(
            report.readings[5].reading,
            Some(crate::service::scenes::Reading::NoResponse {})
        );
        assert_eq!(
            report.readings[6].reading,
            Some(crate::service::scenes::Reading::Value { raw: 3 }),
            "silent byte shifted subsequent luminaire bytes"
        );
        let data = report.luminaire.unwrap();
        if format_collision {
            assert_eq!(
                data.format_status,
                crate::dali::luminaire::FormatStatus::Unavailable
            );
            assert!(
                data.fields.is_empty(),
                "collided format authorized extension data"
            );
        } else {
            use crate::dali::luminaire::{Status, Value};
            assert_eq!(data.fields["manufacturer_gtin"].status, Status::Unavailable);
            assert_eq!(
                data.fields["luminaire_id"].value,
                Some(Value::Text("FEDCBA9876543210".into()))
            );
            assert_eq!(
                data.fields["input_power_nominal"].value,
                Some(Value::Number(42))
            );
            assert_eq!(data.fields["cct"].status, Status::Unavailable);
            assert_eq!(data.fields["identification"].status, Status::Unavailable);
            assert_eq!(
                report.readings[34].reading,
                Some(crate::service::scenes::Reading::TransmitCollision { code: Some(9) })
            );
            assert_eq!(
                report.readings[70].reading,
                Some(crate::service::scenes::Reading::ReceiveCollision { code: Some(2) })
            );
        }
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        peer.abort();
        assert!(peer.await.unwrap_err().is_cancelled());
    }
}
async fn native(emulator: &Emulator) -> serde_json::Value {
    let mut value = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    value.as_object_mut().unwrap().remove("time_ms");
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
async fn luminaire_data_mqtt_native_format_bounds_read_class_and_preserved_state() {
    let fixture = Fixture::new();
    fixture.add_button();
    for (address, name) in [(1, "UnknownFormat"), (2, "Truncated"), (3, "Absent")] {
        fixture.add_output(address, name);
    }
    let saved = fixture.current_bytes();
    let mut scenario: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/emulator/luminaire-i4.json")).unwrap();
    scenario["clock"] = json!("frozen");
    let gear = scenario["buses"][0]["outputs"][0].clone();
    assert_eq!(
        gear["memory"]["banks"][1]["bytes"]
            .as_array()
            .unwrap()
            .len(),
        120
    );
    for (address, id) in [(1, "unknown-format"), (2, "truncated"), (3, "absent")] {
        let mut output = gear.clone();
        output["id"] = json!(id);
        output["short_address"] = json!(address);
        output["groups"] = json!(0);
        output["level"] = json!(42 + address);
        if address == 1 {
            output["memory"]["banks"][1]["bytes"][18] = json!(4);
        }
        if address == 2 {
            output["memory"]["banks"][1]["bytes"][0] = json!(18);
        }
        if address == 3 {
            output.as_object_mut().unwrap().remove("memory");
        }
        scenario["buses"][0]["outputs"]
            .as_array_mut()
            .unwrap()
            .push(output);
    }
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
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    handle
        .read_output(TARGET, context(1), Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    handle
        .read_group_output(
            "Bus1.Groups.All",
            context(1),
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
    let before = handle.snapshot().unwrap();
    let physical = native(&emulator).await;
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    for (index, (target, params, success, count, quality, format)) in [
        (TARGET, json!({}), true, 120, "complete", "supported"),
        (
            "Bus1.Outputs.UnknownFormat",
            json!({}),
            true,
            19,
            "partial",
            "unrecognized",
        ),
        (
            "Bus1.Outputs.Truncated",
            json!({}),
            true,
            19,
            "partial",
            "supported",
        ),
        (
            "Bus1.Outputs.Absent",
            json!({}),
            true,
            1,
            "unavailable",
            "unavailable",
        ),
        ("Bus1.Inputs.Panel", json!({}), false, 0, "", ""),
        ("Bus1.Groups.All", json!({}), false, 0, "", ""),
        (TARGET, json!({"expected_revision":1}), false, 0, "", ""),
        (TARGET, json!({"bank":1}), false, 0, "", ""),
    ]
    .into_iter()
    .enumerate()
    {
        let mut call = json!({"command":"read_luminaire_data","target":target});
        call.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
        let topic = format!("test/luminaire/{index}");
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
                .wait_until(Duration::from_secs(5), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await
        );
        let messages = broker.received_on(&topic);
        assert_eq!(messages.len(), 1);
        let reply: serde_json::Value = serde_json::from_slice(&messages[0].payload).unwrap();
        assert_eq!(
            reply.get("error").is_none(),
            success,
            "native luminaire read-class/target/parameters failed: {reply}"
        );
        if success {
            let report = &reply["value"];
            assert_eq!(report["operation"], "read_luminaire_data");
            assert_eq!(report["bank"], 1);
            assert_eq!(
                report["count"], count,
                "luminaire header/format read bound was ignored"
            );
            assert_eq!(report["readings"].as_array().unwrap().len(), count as usize);
            assert_eq!(report["quality"], quality);
            assert_eq!(report["luminaire"]["format_status"], format);
            assert_eq!(report["configuration_changed"], false);
            if index == 0 {
                assert_eq!(
                    report["luminaire"]["fields"]["luminaire_id"]["value"],
                    "FEDCBA9876543210"
                );
                assert_eq!(
                    report["luminaire"]["fields"]["input_power_nominal"],
                    json!({"status":"value","value":42,"unit":"W"})
                );
                assert_eq!(
                    report["luminaire"]["fields"]["identification"]["value"],
                    "Garden fixture"
                );
                assert_eq!(messages[0].user_property("target-generation"), None);
            }
            if index == 1 {
                assert_eq!(report["luminaire"]["format_id"], 4);
                assert_eq!(report["luminaire"]["fields"], json!({}));
                let snapshot = emulator
                    .control(Control::Snapshot, Duration::from_secs(1))
                    .await
                    .unwrap();
                assert_eq!(
                    snapshot["buses"][0]["outputs"][1]["dtr"][0], 19,
                    "unrecognized format read beyond its proof prefix"
                );
            }
            if index == 2 {
                assert_eq!(
                    report["luminaire"]["fields"]["identification"]["status"],
                    "unavailable"
                );
            }
            assert_eq!(messages[0].user_property("traceparent"), Some(trace));
        }
    }
    let after = handle.snapshot().unwrap();
    assert_eq!(
        before.outputs, after.outputs,
        "luminaire read invalidated output observations"
    );
    assert_eq!(
        before.group_outputs, after.group_outputs,
        "luminaire read invalidated group observations"
    );
    assert_eq!(
        native(&emulator).await,
        physical,
        "luminaire read changed native state other than scratch DTRs"
    );
    assert_eq!(fixture.current_bytes(), saved);
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}
