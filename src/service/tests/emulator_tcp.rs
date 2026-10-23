//! Managed bridge acceptance against the standalone emulator's persistent TCP HAT.
use super::*;
use crate::atx::transactions::RepeatPolicy;
use crate::emulator::{
    scenario::Scenario,
    session::{Control, Emulator},
};
use crate::service::discovery::Space;

#[tokio::test]
async fn standalone_tcp_emulator_drives_discovery_rename_feedback_and_native_input_mqtt() {
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
    ] {
        let fixture = Fixture::new();
        fixture.add_button();
        let scenario:Scenario=serde_json::from_value(json!({"version":1,"repeat":profile,"clock":"frozen","buses":[{"number":0,
            "outputs":[{"id":"saved","short_address":0,"level":100},{"id":"new","short_address":3,"level":75}],
            "inputs":[{"id":"panel","short_address":0,"instances":[1],"feedback":[{"instance":0}]}]}]})).unwrap();
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
            .await
            .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service.with_hat_tcp(address, policy), &handle, &broker).await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        assert!(
            broker
                .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
                .await
        );
        let discovered = handle
            .discover_address(
                0,
                Space::Outputs,
                3,
                1,
                context(1),
                Instant::now() + Duration::from_secs(4),
            )
            .await
            .unwrap();
        assert_eq!(discovered.added_items, 1);
        assert_eq!(discovered.revision, 2);
        handle
            .edit(
                2,
                MetadataEdit::RenameOutput {
                    bus: 0,
                    address: 3,
                    name: "Desk".into(),
                },
                context(2),
                deadline(),
            )
            .await
            .unwrap();
        let snapshot = handle.snapshot().unwrap();
        let table = routing::generations(&snapshot.model);
        let target = "Bus1.Outputs.Desk";
        assert!(broker.send_with(
            &format!("DALI/Desired/Controller/{target}/Brightness"),
            "0.25",
            &SendOptions::default().user_property("target-generation", &table.targets[target])
        ));
        let topic = format!("DALI/State/Controller/{target}");
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on(&topic)
                    .last()
                    .is_some_and(|m| serde_json::from_slice::<serde_json::Value>(
                        &m.payload
                    )
                    .is_ok_and(|v| v["Brightness"] == json!(64.0 / 254.0))))
                .await,
            "standalone TCP emulator did not drive renamed fixture MQTT actual State"
        );
        let button = "Bus1.Inputs.Panel.Instances.Button";
        assert!(broker.send_with(
            &format!("DALI/Desired/Controller/{button}/FeedbackActive"),
            "true",
            &SendOptions::default().user_property("target-generation", &table.targets[button])
        ));
        let input_topic = format!("DALI/State/Controller/{button}");
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on(&input_topic)
                    .last()
                    .is_some_and(|m| serde_json::from_slice::<serde_json::Value>(
                        &m.payload
                    )
                    .is_ok_and(|v| v["FeedbackActive"] == true)))
                .await
        );
        wait_for(&handle, |s| s.input_listeners.contains(button)).await;
        let stimulus = serde_json::from_value(json!({"op":"stimulate","id":"panel","instance":0,
            "stimulus":{"kind":"button","pressed":true}}))
        .unwrap();
        emulator
            .control(stimulus, Duration::from_secs(1))
            .await
            .unwrap();
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on("DALI/Event/Controller")
                    .iter()
                    .any(|m| serde_json::from_slice::<serde_json::Value>(&m.payload)
                        .is_ok_and(
                            |v| v["target"] == button && v["action"] == "pressed"
                        )))
                .await,
            "semantic emulator input did not reach managed MQTT actions"
        );
        let native = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(native["buses"][0]["outputs"][1]["id"], "new");
        assert_eq!(native["buses"][0]["outputs"][1]["level"], 64);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
