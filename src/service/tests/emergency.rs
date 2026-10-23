//! DT1 commands and retained type facts against the independent TCP HAT.
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::session::{Control, Emulator},
};
const TARGET: &str = "Bus1.Outputs.Light1";
async fn call(
    broker: &FakeBroker,
    index: usize,
    command: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let mut body = json!({"command":command,"target":TARGET});
    body.as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    let reply = format!("test/emergency/{index}");
    assert!(broker.publish(
        "DALI/Command/Controller",
        body.to_string(),
        &SendOptions::default()
            .response_topic(&reply)
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property(
                "traceparent",
                "00-11111111111111111111111111111111-2222222222222222-01"
            )
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b
                .received_on(&reply)
                .is_empty())
            .await
    );
    let received = broker.received_on(&reply);
    assert_eq!(received.len(), 1);
    assert_eq!(
        received[0].user_property("traceparent"),
        Some("00-11111111111111111111111111111111-2222222222222222-01")
    );
    serde_json::from_slice(&received[0].payload).unwrap()
}
#[tokio::test]
async fn emergency_commands_read_configure_and_test_without_waiting_for_battery_duration() {
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
    ] {
        let fixture = Fixture::new();
        let saved = fixture.current_bytes();
        let scenario=serde_json::from_value(json!({"version":1,"clock":"frozen","repeat":profile,"buses":[{"number":0,"outputs":[{"id":"dt1","short_address":0,"device_type":1,"level":91,"groups":3,"emergency":{"failures":4,"function_test_seconds":10,"duration_test_seconds":240,"duration_test_passes":false}}]}]})).unwrap();
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
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
        let read = call(&broker, 0, "read_emergency", json!({})).await;
        assert!(read.get("error").is_none(), "DT1 read missing:{read}");
        let r = &read["value"];
        assert_eq!(r["support"], true);
        assert_eq!(r["outcome"], "observed");
        assert_eq!(r["configuration_changed"], false);
        assert_eq!(r["observed"]["fields"]["battery_charge"]["value"], 254);
        assert_eq!(r["observed"]["failures"]["battery_failure"], true);
        let patch = json!({"emergency_level":123,"prolong_seconds":90,"test_execution_timeout_minutes":1440,"function_test_interval_days":7,"function_delay_minutes":480,"duration_test_interval_weeks":4,"duration_delay_minutes":960});
        let changed = call(
            &broker,
            1,
            "configure_emergency",
            json!({"expected_revision":1,"settings":patch}),
        )
        .await;
        assert!(
            changed.get("error").is_none(),
            "DT1 settings refused:{changed}"
        );
        let r = &changed["value"];
        assert_eq!(r["outcome"], "applied");
        for (field, value) in [
            ("emergency_level", 123),
            ("prolong_seconds", 90),
            ("test_execution_timeout_minutes", 1440),
            ("function_test_interval_days", 7),
            ("duration_test_interval_weeks", 4),
            ("next_function_test_minutes", 480),
            ("next_duration_test_minutes", 960),
        ] {
            assert_eq!(r["observed"]["fields"][field]["value"], value, "{field}");
        }
        let requested = call(
            &broker,
            2,
            "start_emergency_test",
            json!({"expected_revision":1,"test":"function"}),
        )
        .await;
        assert_eq!(
            requested["value"]["outcome"], "request_sent",
            "native test request did not return promptly:{requested}"
        );
        assert_eq!(
            requested["value"]["observed"]["mode"]["function_test"],
            true
        );
        emulator
            .control(
                Control::AdvanceTime { millis: 10000 },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        let finished = call(&broker, 3, "read_emergency", json!({})).await;
        assert_eq!(
            finished["value"]["observed"]["status"]["function_done_valid"],
            true
        );
        let requested = call(
            &broker,
            4,
            "start_emergency_test",
            json!({"expected_revision":1,"test":"duration"}),
        )
        .await;
        assert_eq!(requested["value"]["outcome"], "request_sent");
        let stopped = call(
            &broker,
            5,
            "stop_emergency_test",
            json!({"expected_revision":1}),
        )
        .await;
        assert_eq!(stopped["value"]["outcome"], "request_sent");
        assert_eq!(stopped["value"]["observed"]["mode"]["duration_test"], false);
        for (index, command, params) in [
            (10, "read_emergency", json!({"expected_revision":1})),
            (
                11,
                "start_emergency_test",
                json!({"expected_revision":1,"test":"battery"}),
            ),
            (
                12,
                "configure_emergency",
                json!({"expected_revision":1,"settings":{"emergency_level":0}}),
            ),
            (
                13,
                "configure_emergency",
                json!({"expected_revision":1,"settings":{"function_test_interval_days":7}}),
            ),
            (
                14,
                "configure_emergency",
                json!({"expected_revision":1,"settings":{"duration_test_interval_weeks":98}}),
            ),
            (
                15,
                "configure_emergency",
                json!({"expected_revision":1,"settings":{"prolong_seconds":null}}),
            ),
        ] {
            assert!(
                call(&broker, index, command, params)
                    .await
                    .get("error")
                    .is_some(),
                "bad DT1 request accepted"
            );
        }
        let native = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(native["buses"][0]["outputs"][0]["level"], 91);
        assert_eq!(native["buses"][0]["outputs"][0]["groups"], 3);
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
#[tokio::test]
async fn normal_output_read_retains_relay_and_emergency_support_without_selecting_color() {
    for (kind, switching, emergency) in [(0, false, false), (7, true, false), (1, false, true)] {
        let fixture = Fixture::new();
        let saved = fixture.current_bytes();
        let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"fixture","short_address":0,"device_type":kind,"level":0}]}]})).unwrap();
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
        let observed = handle
            .read_output(TARGET, context(1), deadline())
            .await
            .unwrap();
        assert!(
            observed.colour.is_none(),
            "type-only read selected full color sampling"
        );
        let topic = format!("DALI/State/Controller/{TARGET}");
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on(&topic)
                    .last()
                    .is_some_and(|m| serde_json::from_slice::<serde_json::Value>(
                        &m.payload
                    )
                    .is_ok_and(|v| v["SwitchingSupported"] == switching
                        && v["EmergencySupported"] == emergency)))
                .await,
            "normal Read did not populate native type capabilities"
        );
        assert_eq!(fixture.current_bytes(), saved);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn emergency_unverified_writes_refuse_before_any_native_prefix_and_reads_remain_usable() {
    let fixture = Fixture::new();
    let saved = fixture.current_bytes();
    let scenario=serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[{"id":"dt1","short_address":0,"device_type":1,"emergency":{}}]}]})).unwrap();
    let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
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
    let read = handle
        .read_emergency(TARGET, context(1), deadline())
        .await
        .unwrap();
    assert_eq!(read.support, Some(true));
    let before = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()["buses"]
        .clone();
    let settings = serde_json::from_value(json!({"prolong_seconds":90})).unwrap();
    for result in [
        handle
            .configure_emergency(TARGET, 1, settings, context(1), deadline())
            .await,
        handle
            .start_emergency_test(
                TARGET,
                1,
                crate::dali::emergency::Test::Function,
                context(1),
                deadline(),
            )
            .await,
        handle
            .stop_emergency_test(TARGET, 1, context(1), deadline())
            .await,
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("known HAT profile"),
            "unverified DT1 mutation reached native traffic"
        );
    }
    let after = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()["buses"]
        .clone();
    assert_eq!(
        after, before,
        "unverified refusal emitted even a native prefix/DTR write"
    );
    assert_eq!(fixture.current_bytes(), saved);
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}
