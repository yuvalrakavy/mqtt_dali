use super::*;
use crate::dali::gear::Intent;
use crate::service::scene_settings::Action;
mod literal;

#[tokio::test]
async fn dt7_scene_capture_mqtt_preserves_virtual_level_and_binary_actual_state() {
    for two in [false, true] {
        let fixture = Fixture::new();
        save_scene(&fixture);
        let original = fs::read(&fixture.paths.current).unwrap();
        let emulator = pty::spawn(
            serde_json::from_value(json!({"version":1,
                "repeat":if two {"two_terminals"}else{"one_terminal"},
                "buses":[{"number":0,"outputs":[{"id":"relay","short_address":0,
                "device_type":7,"level":254,"switching":{"adjustable_thresholds":true}}]}]
            }))
            .unwrap(),
        )
        .await
        .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &emulator.path,
                if two {
                    RepeatPolicy::TwoTerminals
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let configured = handle
            .configure_output(
                OUTPUT,
                1,
                serde_json::from_value(json!({"minimum_level":1,
                "switching":{"upward_on":30,"downward_off":20}}))
                .unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(configured.outcome, "configured");
        assert!(
            broker
                .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
                .await
        );
        let mut captured_on = None;
        for (virtual_level, contact) in [(100, 254), (10, 0), (0, 0)] {
            let actual = handle
                .apply_output(
                    OUTPUT,
                    Intent::new(None, Some(f64::from(virtual_level) / 254.0)).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap();
            assert_eq!(actual.brightness(), Some(f64::from(contact) / 254.0));
            let cached = handle.snapshot().unwrap().outputs.clone();
            let capture = call(
                &broker,
                json!({"command":"capture_scene_row",
                "target":OUTPUT,"slot":0}),
                1,
                &format!("test/dt7-scene/capture-{virtual_level}"),
            )
            .await;
            assert_eq!(
                capture["value"]["candidate"],
                base_setting(virtual_level),
                "DT7 capture copied binary contact instead of virtual level"
            );
            assert_eq!(capture["value"]["configuration_changed"], false);
            assert_eq!(capture["value"]["revision"], 1);
            if virtual_level == 100 {
                captured_on =
                    Some(serde_json::from_value(capture["value"]["candidate"].clone()).unwrap());
            }
            assert_eq!(
                handle.snapshot().unwrap().outputs,
                cached,
                "DT7 capture replaced public actual State with virtual level"
            );
            let physical = emulator
                .emulator
                .control(Control::Snapshot, Duration::from_secs(1))
                .await
                .unwrap();
            let gear = &physical["buses"][0]["outputs"][0];
            assert_eq!(gear["level"], contact, "DT7 capture operated relay contact");
            assert_eq!(gear["switching_state"]["virtual_level"], virtual_level);
            assert_eq!(
                gear["scenes"][0], 255,
                "DT7 capture programmed scene before review"
            );
            assert_eq!(
                fs::read(&fixture.paths.current).unwrap(),
                original,
                "DT7 capture saved inventory before review"
            );
        }
        let programmed = handle
            .change_scene_row(
                SCENE,
                Action::Set {
                    fixture: OUTPUT.into(),
                    setting: captured_on.unwrap(),
                    expected_revision: 1,
                },
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(programmed.outcome, "verified");
        assert_eq!(
            programmed.stored.unwrap().level,
            crate::service::scenes::Reading::Value { raw: 100 }
        );
        handle
            .recall_scene(SCENE, None, 2, context(1), deadline())
            .await
            .unwrap();
        let recalled = handle
            .read_output(OUTPUT, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(recalled.brightness(), Some(1.0));
        let physical = emulator
            .emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            physical["buses"][0]["outputs"][0]["switching_state"]["virtual_level"], 100,
            "reviewed capture recalled binary contact instead of captured virtual level"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}
