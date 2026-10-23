//! Part252 capture through real managed MQTT and the independent TCP HAT.
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::session::{Control, Emulator},
};
const TARGET: &str = "Bus1.Outputs.Light1";
#[tokio::test]
async fn energy_literal_uart_proves_transition_holes_refusals_and_bounded_release() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    fn query(script: &mut std::collections::VecDeque<(String, String)>, offset: u8, reply: &str) {
        script.push_back((format!("hA3{offset:02X}\n"), "N\n".into()));
        script.push_back(("h01C5\n".into(), reply.into()));
    }
    fn latch(
        script: &mut std::collections::VecDeque<(String, String)>,
        value: u8,
        reply: &str,
        repeated: &str,
    ) {
        script.push_back(("t0181\n".into(), repeated.into()));
        script.push_back(("hA302\n".into(), "N\n".into()));
        script.push_back((format!("hC9{value:02X}\n"), "N\n".into()));
        query(script, 2, reply);
    }
    for (index, case) in [
        "old_latch_holes",
        "latch_collision",
        "release_collision",
        "unknown_version",
        "version_collision",
        "absent",
        "baseline_failure",
    ]
    .into_iter()
    .enumerate()
    {
        let scenario: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/emulator/energy-i4.json"))
                .unwrap();
        let bytes = scenario["buses"][0]["outputs"][0]["memory"]["banks"][0]["bytes"]
            .as_array()
            .unwrap();
        let policy = if index % 2 == 0 {
            RepeatPolicy::OneTerminal
        } else {
            RepeatPolicy::TwoTerminals
        };
        let repeated = if index % 2 == 0 { "N\n" } else { "N\nN\n" };
        let old = matches!(case, "old_latch_holes" | "baseline_failure");
        let no_latch = matches!(case, "unknown_version" | "version_collision" | "absent");
        let mut script =
            std::collections::VecDeque::from([("hC3CA\n".to_string(), "N\n".to_string())]);
        query(
            &mut script,
            0,
            if case == "absent" { "N\n" } else { "J0F\n" },
        );
        if case != "absent" {
            query(&mut script, 1, "J00\n");
            query(&mut script, 2, if old { "JAA\n" } else { "JFF\n" });
            query(
                &mut script,
                3,
                match case {
                    "unknown_version" => "J02\n",
                    "version_collision" => "X2\n",
                    _ => "J01\n",
                },
            );
        }
        if !no_latch {
            if old {
                latch(
                    &mut script,
                    255,
                    if case == "baseline_failure" {
                        "JAA\n"
                    } else {
                        "JFF\n"
                    },
                    repeated,
                );
            }
            if case != "baseline_failure" {
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
                if case != "latch_collision" {
                    for (offset, byte) in bytes.iter().enumerate().take(16).skip(4) {
                        let reply = if case == "old_latch_holes" && offset == 8 {
                            "N\n".into()
                        } else if case == "old_latch_holes" && offset == 13 {
                            "Z9\n".into()
                        } else {
                            format!("J{:02X}\n", byte.as_u64().unwrap())
                        };
                        query(&mut script, offset as u8, &reply);
                    }
                    query(&mut script, 2, "JAA\n");
                }
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
                            .expect("unexpected energy native command");
                        assert_eq!(
                            line, expected,
                            "energy native snapshot sequence changed: {case}"
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
            .read_energy_data(
                TARGET,
                crate::dali::energy::Bank::Active,
                context(1),
                deadline(),
            )
            .await;
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
        if matches!(
            case,
            "latch_collision" | "release_collision" | "baseline_failure"
        ) {
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(if case == "release_collision" {
                    "release"
                } else if case == "baseline_failure" {
                    "baseline"
                } else {
                    "latch"
                }),
                "failed snapshot proof accepted or changed cause: {message}"
            );
        } else {
            let report = result.unwrap();
            let energy = report.energy.unwrap();
            if case == "old_latch_holes" {
                assert_eq!(report.count, 16);
                assert_eq!(report.quality, "partial");
                assert_eq!(
                    report.readings[8].reading,
                    Some(crate::service::scenes::Reading::NoResponse {})
                );
                assert_eq!(
                    report.readings[13].reading,
                    Some(crate::service::scenes::Reading::TransmitCollision { code: Some(9) })
                );
                assert_eq!(
                    energy.data.fields["energy"].status,
                    crate::dali::energy::Status::Unavailable
                );
                assert_eq!(
                    energy.data.fields["power"].status,
                    crate::dali::energy::Status::Unavailable
                );
                assert_eq!(
                    energy.snapshot.unwrap().release,
                    crate::service::scenes::Reading::Value { raw: 255 }
                );
            } else {
                assert_eq!(report.count, if case == "absent" { 1 } else { 4 });
                assert!(energy.data.fields.is_empty());
                assert!(energy.snapshot.is_none());
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
    let mut call = json!({"command":"read_energy_data","target":target});
    call.as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    let topic = format!("test/energy/{index}");
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
async fn energy_managed_mqtt_exact_banks_snapshot_refusal_and_preserved_state() {
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
        let mut scenario: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/emulator/energy-i4.json"))
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
                1 => output["memory"]["banks"][0]["bytes"][3] = json!(2),
                2 => {
                    output.as_object_mut().unwrap().remove("memory");
                }
                3 => output["memory"]["banks"][0]["bytes"][0] = json!(10),
                4 => output["memory"]["banks"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("latch")
                    .map(|_| ())
                    .unwrap(),
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
        if policy == RepeatPolicy::Unverified {
            let scratch = native(&emulator, true).await;
            let reply = command(&broker, 0, TARGET, json!({"bank":202})).await;
            assert!(
                reply["error"]
                    .as_str()
                    .is_some_and(|e| e.contains("known HAT profile")),
                "unverified energy read did not refuse before native commands: {reply}"
            );
            assert_eq!(
                native(&emulator, true).await,
                scratch,
                "unverified energy read sent a native command"
            );
        } else {
            for bank in [202, 203, 204] {
                let reply = command(&broker, bank as usize, TARGET, json!({"bank":bank})).await;
                assert!(
                    reply.get("error").is_none(),
                    "managed energy read-class command failed: {reply}"
                );
                let report = &reply["value"];
                assert_eq!(report["operation"], "read_energy_data");
                assert_eq!(report["bank"], bank);
                assert_eq!(report["count"], 16);
                assert_eq!(report["quality"], "complete");
                assert_eq!(report["configuration_changed"], false);
                let energy = &report["energy"];
                assert_eq!(energy["version"], 1);
                assert_eq!(energy["format_status"], "supported");
                for (key, raw) in [("start", 170), ("end", 170), ("release", 255)] {
                    assert_eq!(
                        energy["snapshot"][key],
                        json!({"kind":"value","raw":raw}),
                        "energy snapshot proof missing: {key}"
                    );
                }
                let (value, power, unit, power_unit) = match bank {
                    202 => ("123.456", "42.5", "Wh", "W"),
                    203 => ("281474976710653000000", "4294.967293", "VAh", "VA"),
                    _ => ("", "0", "Wh", "W"),
                };
                if bank == 204 {
                    assert_eq!(
                        energy["fields"]["energy"]["status"],
                        "temporarily_unavailable"
                    );
                    assert_eq!(energy["fields"]["energy"]["value"], serde_json::Value::Null);
                } else {
                    assert_eq!(
                        energy["fields"]["energy"]["value"], value,
                        "energy decimal lost precision"
                    );
                }
                assert_eq!(energy["fields"]["power"]["value"], power);
                assert_eq!(energy["fields"]["energy"]["unit"], unit);
                assert_eq!(energy["fields"]["power"]["unit"], power_unit);
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
                    "successful energy read retained latch"
                );
                assert!(
                    serde_json::to_value(raw).unwrap().get("energy").is_none(),
                    "energy field changed raw report contract"
                );
            }
            for (index, target, count, quality, status) in [
                (1, "UnknownVersion", 4, "partial", "unrecognized"),
                (2, "Absent", 1, "unavailable", "unavailable"),
                (3, "Truncated", 11, "partial", "supported"),
            ] {
                let reply = command(
                    &broker,
                    300 + index,
                    &format!("Bus1.Outputs.{target}"),
                    json!({"bank":202}),
                )
                .await;
                assert!(
                    reply.get("error").is_none(),
                    "bounded energy read failed: {reply}"
                );
                let report = &reply["value"];
                assert_eq!(
                    report["count"], count,
                    "energy header/version bound was ignored"
                );
                assert_eq!(report["quality"], quality);
                assert_eq!(report["energy"]["format_status"], status);
                if index < 3 {
                    assert_eq!(report["energy"]["fields"], json!({}));
                    assert_eq!(report["energy"]["snapshot"], serde_json::Value::Null);
                } else {
                    assert_eq!(report["energy"]["fields"]["power"]["status"], "unavailable");
                }
                if index == 1 {
                    let raw = native(&emulator, true).await;
                    assert_eq!(
                        raw["buses"][0]["outputs"][1]["dtr"][0], 4,
                        "unknown energy version authorized latch/extension commands"
                    );
                }
            }
            let reply = command(&broker, 400, "Bus1.Outputs.NoLatch", json!({"bank":202})).await;
            assert!(
                reply["error"].as_str().is_some_and(|e| e.contains("latch")),
                "missing energy latch proof accepted measurements: {reply}"
            );
            for (index, target, params) in [
                (0, "Bus1.Inputs.Panel", json!({"bank":202})),
                (1, "Bus1.Groups.All", json!({"bank":202})),
                (2, TARGET, json!({})),
                (3, TARGET, json!({"bank":1})),
                (4, TARGET, json!({"bank":205})),
                (5, TARGET, json!({"bank":202,"expected_revision":1})),
                (6, TARGET, json!({"bank":202.0})),
            ] {
                let reply = command(&broker, 500 + index, target, params).await;
                assert!(
                    reply.get("error").is_some(),
                    "invalid energy target/parameters accepted: {reply}"
                );
            }
        }
        let after = handle.snapshot().unwrap();
        assert_eq!(after.outputs, before.outputs);
        assert_eq!(after.group_outputs, before.group_outputs);
        assert_eq!(
            native(&emulator, false).await,
            physical,
            "energy read changed native installation state"
        );
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
