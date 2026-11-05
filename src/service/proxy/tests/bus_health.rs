use super::*;
use crate::emulator::{scenario::Scenario, session::Emulator};

const TOPIC: &str = "DALI/State/Controller/__bus_health";

fn row(bus: u8, health: &str, code: Option<u8>) -> Value {
    json!({"BusNumber":bus,"Health":health,"StatusCode":code})
}

async fn wait_health(broker: &FakeBroker, revision: u64, buses: Vec<Value>) {
    let expected = json!({"Buses":buses});
    let ctx = context(revision);
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |broker| {
                broker.received_on(TOPIC).last().is_some_and(|message| {
                    message.retain
                        && message.user_property("target-generation") == ctx.generation()
                        && serde_json::from_slice::<Value>(&message.payload)
                            .is_ok_and(|value| value == expected)
                })
            })
            .await,
        "missing retained bus health at revision {revision}: {expected}"
    );
}

#[tokio::test]
async fn bus_health_mqtt_tracks_empty_bus_power_and_disable_independently() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses.push(
        serde_json::from_value(json!({"bus":1,"name":"Bus2","enabled":true,"outputs":[]})).unwrap(),
    );
    fixture.write_inventory(&inventory);
    let scenario: Scenario = serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[
        {"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100}]},
        {"number":1,"outputs":[]}
    ]}))
    .unwrap();
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
    wait_health(
        &broker,
        1,
        vec![row(0, "ok", Some(2)), row(1, "ok", Some(2))],
    )
    .await;
    for (wire, status, raw) in [
        (b"1D14\n", "short_to_positive", 1),
        (b"1D74\n", "unknown", 7),
    ] {
        emulator
            .control(
                Control::Raw {
                    bytes: wire.to_vec(),
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        wait_health(
            &broker,
            1,
            vec![row(0, "ok", Some(2)), row(1, status, Some(raw))],
        )
        .await;
    }
    for (powered, status, raw) in [(false, "no_bus_power", 0), (true, "ok", 2)] {
        emulator
            .control(
                Control::BusPower { bus: 1, powered },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        wait_health(
            &broker,
            1,
            vec![row(0, "ok", Some(2)), row(1, status, Some(raw))],
        )
        .await;
    }
    handle
        .edit(
            1,
            MetadataEdit::ConfigureBus {
                bus: 1,
                name: "Bus2".into(),
                enabled: false,
            },
            context(1),
            crate::service::tests::deadline(),
        )
        .await
        .unwrap();
    wait_health(
        &broker,
        2,
        vec![row(0, "ok", Some(2)), row(1, "disabled", None)],
    )
    .await;
    emulator.close().await.unwrap();
    wait_health(
        &broker,
        2,
        vec![row(0, "unavailable", None), row(1, "disabled", None)],
    )
    .await;
    assert!(running.stop().await);
}
