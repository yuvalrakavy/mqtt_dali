use super::*;
use crate::atx::transactions::BusReply;
use crate::dali::gear::Query;
const TARGET: &str = "Bus1.Outputs.Light1";
const FLAGS: [&str; 8] = [
    "GearFailure",
    "LampFailure",
    "LampOn",
    "LimitError",
    "FadeRunning",
    "ResetState",
    "MissingShortAddress",
    "PowerFailure",
];
fn snapshot(status: Option<BusReply>, level: u8) -> Snapshot {
    let mut s = crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    s.hardware = Hardware::Ready {
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
    s.outputs.insert(
        TARGET.into(),
        output::Observation {
            context: context(1),
            actual_level: Some(Query::ActualLevel.decode(BusReply::Value(level))),
            status: status.map(|r| Query::Status.decode(r)),
            colour: None,
            device_types: vec![],
            quality: if level == 255 {
                output::Quality::Partial
            } else {
                output::Quality::Current
            },
            reason: None,
            stamp: output::Stamp {
                session: 1,
                actor: 1,
                bus: 1,
                observed_bus: 1,
            },
            bus: 0,
        },
    );
    s
}
fn projected(s: &Snapshot) -> PropertyMap {
    state::projections(s)
        .into_iter()
        .find(|p| p.state.target == TARGET)
        .unwrap()
        .state
        .values
}
fn unknown(v: &PropertyMap, quality: &str) {
    assert_eq!(
        v.get("FixtureStatusQuality"),
        Some(&json!(quality)),
        "missing or incorrect fixture status quality"
    );
    for key in FLAGS.into_iter().chain(["FixtureStatusRaw"]) {
        assert_eq!(
            v.get(key),
            Some(&Value::Null),
            "unknown fixture status retained or invented {key}"
        );
    }
}
#[test]
fn fixture_status_projection_decodes_faults_without_marking_a_responding_fixture_unavailable() {
    let v = projected(&snapshot(Some(BusReply::Value(0x83)), 100));
    assert_eq!(
        v.get("LampFailure"),
        Some(&json!(true)),
        "fixture MQTT State omits reported lamp failure"
    );
    assert_eq!(v["GearFailure"], true);
    assert_eq!(v["PowerFailure"], true);
    assert_eq!(v["FixtureStatusRaw"], 131);
    assert_eq!(v["FixtureStatusQuality"], "current");
    for key in [
        "LampOn",
        "LimitError",
        "FadeRunning",
        "ResetState",
        "MissingShortAddress",
    ] {
        assert_eq!(v[key], false);
    }
    assert_eq!(v["Available"], true);
    assert_eq!(v["Brightness"], json!(100.0 / 254.0));
    let healthy = projected(&snapshot(Some(BusReply::Value(4)), 100));
    assert_eq!(
        healthy["LampFailure"], false,
        "cleared lamp failure did not replace true"
    );
    assert_eq!(healthy["LampOn"], true);
}
#[test]
fn fixture_status_remains_current_when_level_is_mask_and_ff_status_is_not_mask() {
    let v = projected(&snapshot(Some(BusReply::Value(2)), 255));
    assert_eq!(
        v.get("FixtureStatusQuality"),
        Some(&json!("current")),
        "level MASK suppressed independently current status"
    );
    assert_eq!(v["Available"], false);
    assert_eq!(v["LampFailure"], true);
    assert!(!v.contains_key("PowerState"));
    let v = projected(&snapshot(Some(BusReply::Value(255)), 100));
    assert_eq!(
        v["FixtureStatusRaw"], 255,
        "status FF was mistaken for MASK"
    );
    for key in FLAGS {
        assert_eq!(v[key], true, "status FF lost {key}");
    }
}
#[test]
fn fixture_status_missing_stale_and_retired_observations_explicitly_clear_all_fields() {
    let mut s = snapshot(None, 100);
    unknown(&projected(&s), "unavailable");
    s.outputs.get_mut(TARGET).unwrap().status = Some(Query::Status.decode(BusReply::NoResponse));
    unknown(&projected(&s), "unavailable");
    for reply in [
        BusReply::ReceiveCollision(None),
        BusReply::TransmitCollision(None),
    ] {
        s.outputs.get_mut(TARGET).unwrap().status = Some(Query::Status.decode(reply));
        unknown(&projected(&s), "unavailable");
    }
    s = snapshot(Some(BusReply::Value(2)), 100);
    s.outputs.get_mut(TARGET).unwrap().quality = output::Quality::Stale;
    unknown(&projected(&s), "stale");
    s = snapshot(Some(BusReply::Value(2)), 100);
    s.hardware = Hardware::Closed;
    unknown(&projected(&s), "stale");
    s = snapshot(Some(BusReply::Value(2)), 100);
    s.outputs.get_mut(TARGET).unwrap().context = context(2);
    unknown(&projected(&s), "unobserved");
    s.outputs.clear();
    unknown(&projected(&s), "unobserved");
}
#[test]
fn fixture_status_properties_are_read_only_desired_values() {
    for property in FLAGS.into_iter().chain([
        "FixtureStatusRaw",
        "FixtureStatusQuality",
        "SwitchingSupported",
        "EmergencySupported",
    ]) {
        let values = PropertyMap::from_iter([(property.into(), json!(false))]);
        assert!(
            matches!(state::intent(&values, None), Err(OpError::Unsupported(_))),
            "status property became writable: {property}"
        );
    }
}

#[test]
fn output_type_capabilities_are_independent_and_clear_when_evidence_is_retired() {
    for (kind, switching, emergency) in [(7, true, false), (1, false, true), (0, false, false)] {
        let mut current = snapshot(Some(BusReply::Value(4)), 255);
        current.outputs.get_mut(TARGET).unwrap().device_types = vec![Some(kind)];
        let values = projected(&current);
        assert_eq!(values["SwitchingSupported"], switching);
        assert_eq!(values["EmergencySupported"], emergency);
        assert_eq!(
            values["Available"], false,
            "type evidence fabricated usable brightness"
        );
        for retired in ["stale", "disconnected", "generation", "missing"] {
            let mut old = current.clone();
            match retired {
                "stale" => old.outputs.get_mut(TARGET).unwrap().quality = output::Quality::Stale,
                "disconnected" => old.hardware = Hardware::Closed,
                "generation" => old.outputs.get_mut(TARGET).unwrap().context = context(2),
                _ => old.outputs.clear(),
            }
            let values = projected(&old);
            for key in ["SwitchingSupported", "EmergencySupported"] {
                assert_eq!(
                    values[key],
                    Value::Null,
                    "retired native type remained actionable:{retired}/{key}"
                );
            }
        }
    }
}
#[tokio::test]
async fn fixture_status_native_tcp_lamp_failure_reaches_mqtt_and_clears_without_changing_levels() {
    use crate::emulator::{scenario::Scenario, session::Emulator};
    let fixture = Fixture::new();
    let scenario: Scenario = serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[
        {"id":"lamp","short_address":0,"level":100}, {"id":"other","short_address":1,"level":50}]}]})).unwrap();
    let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
        .await
        .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_hat_tcp(address, RepeatPolicy::OneTerminal),
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
    let before = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    for (index, failed) in [false, true, false].into_iter().enumerate() {
        emulator
            .control(
                Control::SetLampFailure {
                    id: "lamp".into(),
                    failed,
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        let result = refresh(&broker, &format!("test/status/{index}")).await;
        assert!(result.get("error").is_none(), "refresh failed: {result}");
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on(&format!("DALI/State/Controller/{TARGET}"))
                    .last()
                    .is_some_and(|m| serde_json::from_slice::<Value>(&m.payload)
                        .is_ok_and(|v| v["LampFailure"] == failed
                            && v["FixtureStatusQuality"] == "current"
                            && v["FixtureStatusRaw"] == if failed { 6 } else { 4 }
                            && v["Available"] == true
                            && v["Brightness"] == json!(100.0 / 254.0))))
                .await,
            "native lamp failure/clear did not reach retained MQTT state"
        );
        let actual = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(actual["buses"][0]["outputs"][0]["level"], 100);
        assert_eq!(
            actual["buses"][0]["outputs"][1],
            before["buses"][0]["outputs"][1]
        );
    }
    let s = handle.snapshot().unwrap();
    let projection = state::projections(&s)
        .into_iter()
        .find(|p| p.state.target == TARGET)
        .unwrap();
    assert_eq!(
        projection.context.generation(),
        s.outputs[TARGET].context.generation()
    );
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}
