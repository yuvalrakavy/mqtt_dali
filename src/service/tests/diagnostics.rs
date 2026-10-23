//! Standard diagnostics through managed MQTT and an independent TCP HAT.
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::session::{Control, Emulator},
};
const TARGET: &str = "Bus1.Outputs.Light1";
#[tokio::test]
async fn diagnostics_literal_uart_dynamic_bounds_holes_release_and_static_no_write() {
    use crate::service::scenes::Reading as R;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    type Script = std::collections::VecDeque<(String, String)>;
    fn query(script: &mut Script, offset: u8, reply: &str) {
        script.push_back((format!("hA3{offset:02X}\n"), "N\n".into()));
        script.push_back(("h01C5\n".into(), reply.into()));
    }
    fn latch(script: &mut Script, value: u8, reply: &str, repeated: &str) {
        script.push_back(("t0181\n".into(), repeated.into()));
        script.push_back(("hA302\n".into(), "N\n".into()));
        script.push_back((format!("hC9{value:02X}\n"), "N\n".into()));
        query(script, 2, reply);
    }
    for (index, (bank, case)) in [
        (205, "old_latch_holes"),
        (206, "complete"),
        (207, "static"),
        (205, "latch_collision"),
        (206, "release_collision"),
        (205, "unknown"),
        (207, "unknown_static"),
    ]
    .into_iter()
    .enumerate()
    {
        let scenario: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/emulator/diagnostics-i4.json"
        ))
        .unwrap();
        let bytes = scenario["buses"][0]["outputs"][0]["memory"]["banks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["bank"] == bank)
            .unwrap()["bytes"]
            .as_array()
            .unwrap();
        let policy = if bank == 207 {
            RepeatPolicy::Unverified
        } else if index % 2 == 0 {
            RepeatPolicy::OneTerminal
        } else {
            RepeatPolicy::TwoTerminals
        };
        let repeated = if policy == RepeatPolicy::TwoTerminals {
            "N\nN\n"
        } else {
            "N\n"
        };
        let unknown = case.starts_with("unknown");
        let old = case == "old_latch_holes";
        let mut script = Script::from([(format!("hC3{bank:02X}\n"), "N\n".to_string())]);
        query(
            &mut script,
            0,
            &format!("J{:02X}\n", bytes[0].as_u64().unwrap()),
        );
        query(&mut script, 1, "J00\n");
        query(&mut script, 2, if old { "JAA\n" } else { "JFF\n" });
        query(&mut script, 3, if unknown { "J02\n" } else { "J01\n" });
        if !unknown {
            if bank != 207 {
                if old {
                    latch(&mut script, 255, "JFF\n", repeated);
                }
                latch(
                    &mut script,
                    170,
                    if case == "latch_collision" {
                        "X2\n"
                    } else {
                        "JAA\n"
                    },
                    repeated,
                );
            }
            if case != "latch_collision" {
                for (offset, byte) in bytes.iter().enumerate().skip(4) {
                    let reply = if old && offset == 8 {
                        "N\n".into()
                    } else if old && offset == 27 {
                        "Z9\n".into()
                    } else {
                        format!("J{:02X}\n", byte.as_u64().unwrap())
                    };
                    query(&mut script, offset as u8, &reply);
                }
                if bank != 207 {
                    query(&mut script, 2, "JAA\n");
                }
            }
            if bank != 207 {
                latch(
                    &mut script,
                    255,
                    if case == "release_collision" {
                        "Z7\n"
                    } else {
                        "JFF\n"
                    },
                    repeated,
                );
            }
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
                let response = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        let (expected, response) = script
                            .pop_front()
                            .expect("unexpected diagnostic native command");
                        assert_eq!(
                            line, expected,
                            "diagnostic native snapshot sequence changed:{bank}/{case}"
                        );
                        response
                    }
                };
                write.write_all(response.as_bytes()).await.unwrap();
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
        let result = handle
            .read_diagnostics(
                TARGET,
                crate::dali::diagnostics::Bank::new(bank as u8).unwrap(),
                context(1),
                deadline(),
            )
            .await;
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
        assert!(
            !peer.is_finished(),
            "diagnostic literal peer exited before completion"
        );
        if matches!(case, "latch_collision" | "release_collision") {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(if case == "release_collision" {
                    "release"
                } else {
                    "latch"
                }),
                "failed diagnostic snapshot proof accepted or changed cause:{message}"
            );
        } else {
            let report = result.unwrap();
            let data = report.diagnostics.unwrap();
            assert_eq!(report.count, if unknown { 4 } else { bytes.len() as u8 });
            assert_eq!(
                report.quality,
                if old || unknown {
                    "partial"
                } else {
                    "complete"
                }
            );
            if unknown {
                assert!(data.data.fields.is_empty());
                assert!(data.snapshot.is_none());
            } else if bank == 207 {
                assert!(data.snapshot.is_none());
                assert_eq!(
                    data.data.fields["rated_life"].value,
                    Some(crate::dali::diagnostics::Value::Number("50000".into()))
                );
            } else {
                assert_eq!(data.snapshot.unwrap().release, R::Value { raw: 255 });
                if old {
                    assert_eq!(report.readings[8].reading, Some(R::NoResponse {}));
                    assert_eq!(
                        report.readings[27].reading,
                        Some(R::TransmitCollision { code: Some(9) })
                    );
                    assert_eq!(
                        data.data.fields["start_counter"].status,
                        crate::dali::diagnostics::Status::Unavailable
                    );
                    assert_eq!(
                        data.data.fields["temperature"].status,
                        crate::dali::diagnostics::Status::Unavailable
                    );
                } else {
                    assert_eq!(
                        data.data.fields["current"].value,
                        Some(crate::dali::diagnostics::Value::Number("1.234".into()))
                    );
                    assert_eq!(report.readings[32].reading, Some(R::Value { raw: 85 }));
                }
            }
        }
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        peer.abort();
        assert!(peer.await.unwrap_err().is_cancelled());
    }
}
async fn native(emulator: &Emulator, scratch: bool) -> serde_json::Value {
    let mut value = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    if !scratch {
        for bus in value["buses"].as_array_mut().unwrap() {
            for gear in bus["outputs"].as_array_mut().unwrap() {
                gear.as_object_mut().unwrap().remove("dtr");
            }
        }
    }
    value
}
async fn command(
    broker: &FakeBroker,
    index: usize,
    target: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let mut call = json!({"command":"read_diagnostics","target":target});
    call.as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    let topic = format!("test/diagnostics/{index}");
    assert!(broker.publish(
        "DALI/Command/Controller",
        call.to_string(),
        &SendOptions::default().response_topic(&topic).user_property(
            "traceparent",
            "00-11111111111111111111111111111111-2222222222222222-01"
        )
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b
                .received_on(&topic)
                .is_empty())
            .await
    );
    let messages = broker.received_on(&topic);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].user_property("target-generation"), None);
    assert_eq!(
        messages[0].user_property("traceparent"),
        Some("00-11111111111111111111111111111111-2222222222222222-01")
    );
    serde_json::from_slice(&messages[0].payload).unwrap()
}
#[tokio::test]
async fn diagnostics_managed_mqtt_exact_banks_policy_static_read_and_preserved_state() {
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
        ("one_terminal", RepeatPolicy::Unverified),
    ] {
        let fixture = Fixture::new();
        fixture.add_button();
        for (address, name) in [
            (1, "UnknownVersion"),
            (2, "Absent"),
            (3, "Truncated"),
            (4, "NoLatch"),
        ] {
            fixture.add_output(address, name);
        }
        let saved = fixture.current_bytes();
        let mut scenario: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/emulator/diagnostics-i4.json"
        ))
        .unwrap();
        scenario["clock"] = json!("frozen");
        scenario["repeat"] = json!(profile);
        let original = scenario["buses"][0]["outputs"][0].clone();
        for (address, id) in [
            (1, "unknown"),
            (2, "absent"),
            (3, "truncated"),
            (4, "no-latch"),
        ] {
            let mut output = original.clone();
            output["id"] = json!(id);
            output["short_address"] = json!(address);
            output["groups"] = json!(0);
            match address {
                1 => {
                    for bank in output["memory"]["banks"].as_array_mut().unwrap() {
                        bank["bytes"][3] = json!(2);
                    }
                }
                2 => {
                    output.as_object_mut().unwrap().remove("memory");
                }
                3 => {
                    for bank in output["memory"]["banks"].as_array_mut().unwrap() {
                        bank["bytes"][0] = json!(5);
                    }
                }
                4 => {
                    for bank in output["memory"]["banks"].as_array_mut().unwrap() {
                        bank.as_object_mut().unwrap().remove("latch");
                    }
                }
                _ => unreachable!(),
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
        let running = start(service.with_hat_tcp(address, policy), &handle, &broker).await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        assert!(
            broker
                .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
                .await
        );
        handle
            .read_output(TARGET, context(1), deadline())
            .await
            .unwrap();
        handle
            .read_group_output("Bus1.Groups.All", context(1), deadline())
            .await
            .unwrap();
        let before = handle.snapshot().unwrap();
        let physical = native(&emulator, false).await;
        // Static207 must work even when repeated native commands are unverified.
        for bank in [205, 206, 207] {
            let scratch = native(&emulator, true).await;
            let reply = command(&broker, bank as usize, TARGET, json!({"bank":bank})).await;
            if policy == RepeatPolicy::Unverified && bank != 207 {
                assert!(
                    reply["error"]
                        .as_str()
                        .is_some_and(|e| e.contains("known HAT profile")),
                    "unverified diagnostic snapshot did not refuse before native commands: {reply}"
                );
                assert_eq!(
                    native(&emulator, true).await,
                    scratch,
                    "unverified diagnostics sent a native prefix"
                );
                continue;
            }
            assert!(
                reply.get("error").is_none(),
                "managed diagnostic read-class command failed: {reply}"
            );
            let report = &reply["value"];
            assert_eq!(report["operation"], "read_diagnostics");
            assert_eq!(report["bank"], bank);
            assert_eq!(
                report["count"],
                match bank {
                    205 => 29,
                    206 => 33,
                    _ => 8,
                },
                "diagnostic standard bank was truncated"
            );
            assert_eq!(report["quality"], "complete");
            assert_eq!(report["configuration_changed"], false);
            assert!(report.get("energy").is_none());
            assert!(report.get("luminaire").is_none());
            let data = &report["diagnostics"];
            assert_eq!(data["version"], 1);
            assert_eq!(data["format_status"], "supported");
            assert_eq!(
                data["fields"].as_object().unwrap().len(),
                match bank {
                    205 => 19,
                    206 => 17,
                    _ => 3,
                }
            );
            if bank == 207 {
                assert_eq!(
                    data["snapshot"],
                    serde_json::Value::Null,
                    "static maintenance invented a latch snapshot"
                );
                assert_eq!(
                    data["fields"]["rated_life"],
                    json!({"status":"value","raw":50,"value":"50000","unit":"h"})
                );
                assert_eq!(data["fields"]["reference_temperature"]["value"], "-60");
                assert_eq!(data["fields"]["rated_starts"]["value"], "6553300");
            } else {
                for (key, raw) in [("start", 170), ("end", 170), ("release", 255)] {
                    assert_eq!(
                        data["snapshot"][key],
                        json!({"kind":"value","raw":raw}),
                        "diagnostic snapshot proof missing:{key}"
                    );
                }
                if bank == 205 {
                    assert_eq!(data["fields"]["operating_time"]["value"], "305419896");
                    assert_eq!(data["fields"]["supply_voltage"]["value"], "230");
                    assert_eq!(data["fields"]["supply_voltage"]["unit"], "Vrms");
                    assert_eq!(data["fields"]["temperature"]["value"], "-25");
                    assert_eq!(data["fields"]["overall_failure"]["value"], true);
                } else {
                    assert_eq!(data["fields"]["voltage"]["value"], "48.1");
                    assert_eq!(
                        data["fields"]["current"],
                        json!({"status":"value","raw":1234,"value":"1.234","unit":"A"})
                    );
                    assert_eq!(data["fields"]["temperature"]["value"], "25");
                    assert_eq!(data["fields"]["overall_failure"]["value"], false);
                }
                let raw = handle
                    .read_memory(
                        TARGET,
                        bank as u8,
                        crate::dali::memory::Window::new(2, 1).unwrap(),
                        context(1),
                        deadline(),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    raw.readings[0].reading,
                    Some(crate::service::scenes::Reading::Value { raw: 255 }),
                    "successful diagnostic read retained latch"
                );
                assert!(serde_json::to_value(raw)
                    .unwrap()
                    .get("diagnostics")
                    .is_none());
            }
        }
        // Every bank's prefix is bounded; static207 also preserves that contract.
        for bank in if policy == RepeatPolicy::Unverified {
            vec![207]
        } else {
            vec![205, 206, 207]
        } {
            for (index, name, count, quality, status) in [
                (1, "UnknownVersion", 4, "partial", "unrecognized"),
                (2, "Absent", 1, "unavailable", "unavailable"),
                (3, "Truncated", 6, "partial", "supported"),
            ] {
                let reply = command(
                    &broker,
                    bank as usize * 10 + index,
                    &format!("Bus1.Outputs.{name}"),
                    json!({"bank":bank}),
                )
                .await;
                assert!(
                    reply.get("error").is_none(),
                    "bounded diagnostic read failed:{reply}"
                );
                let report = &reply["value"];
                assert_eq!(
                    report["count"], count,
                    "diagnostic header/version bound was ignored"
                );
                assert_eq!(report["quality"], quality);
                assert_eq!(report["diagnostics"]["format_status"], status);
                if index < 3 {
                    assert_eq!(report["diagnostics"]["fields"], json!({}));
                    assert_eq!(report["diagnostics"]["snapshot"], serde_json::Value::Null);
                }
                if index == 1 {
                    assert_eq!(
                        native(&emulator, true).await["buses"][0]["outputs"][1]["dtr"][0],
                        4,
                        "unknown diagnostic version authorized extension/latch"
                    );
                }
            }
        }
        if policy != RepeatPolicy::Unverified {
            for bank in [205, 206] {
                let reply = command(
                    &broker,
                    bank as usize + 400,
                    "Bus1.Outputs.NoLatch",
                    json!({"bank":bank}),
                )
                .await;
                assert!(
                    reply["error"].as_str().is_some_and(|e| e.contains("latch")),
                    "missing diagnostic latch accepted measurements:{reply}"
                );
            }
        }
        for (index, target, params) in [
            (0, "Bus1.Inputs.Panel", json!({"bank":207})),
            (1, "Bus1.Groups.All", json!({"bank":207})),
            (2, TARGET, json!({})),
            (3, TARGET, json!({"bank":204})),
            (4, TARGET, json!({"bank":208})),
            (5, TARGET, json!({"bank":207,"expected_revision":1})),
            (6, TARGET, json!({"bank":207.0})),
        ] {
            let reply = command(&broker, 5000 + index, target, params).await;
            assert!(
                reply.get("error").is_some(),
                "invalid diagnostic target/params accepted:{reply}"
            );
        }
        let after = handle.snapshot().unwrap();
        assert_eq!(after.outputs, before.outputs);
        assert_eq!(after.group_outputs, before.group_outputs);
        assert_eq!(after.inputs, before.inputs);
        assert_eq!(
            native(&emulator, false).await,
            physical,
            "diagnostic read changed native installation state"
        );
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
