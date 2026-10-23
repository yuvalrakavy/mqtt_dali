use super::*;
use serde_json::json;
#[tokio::test]
async fn fixture_status_control_capture_reboot_and_reset_preserve_saved_faults() {
    let (stream, mut wire) = tokio::io::duplex(4096);
    let emulator = Emulator::start(stream, scenario()).unwrap();
    assert_eq!(line(&mut wire).await, b"B000004\n");
    for value in [
        json!({"op":"set_lamp_failure","id":"physical-A","failed":null}),
        json!({"op":"set_lamp_failure","id":"physical-A","failed":true,"extra":1}),
        json!({"op":"set_lamp_failure","id":"physical-A"}),
    ] {
        assert!(serde_json::from_value::<Control>(value).is_err());
    }
    let set = |failed| {
        serde_json::from_value(json!({"op":"set_lamp_failure","id":"physical-A","failed":failed}))
            .unwrap()
    };
    control(&emulator, set(true)).await;
    assert!(emulator
        .control(
            Control::SetLampFailure {
                id: "missing".into(),
                failed: true
            },
            Duration::from_secs(1)
        )
        .await
        .is_err());
    control(&emulator, Control::Reboot).await;
    assert_eq!(line(&mut wire).await, b"B000004\n");
    wire.write_all(b"h0790\n").await.unwrap();
    assert_eq!(line(&mut wire).await, b"H0790\n");
    assert_eq!(line(&mut wire).await, b"J06\n");
    control(&emulator, Control::CaptureScenario).await;
    assert_eq!(
        control(&emulator, Control::Scenario).await["buses"][0]["outputs"][0]["lamp_failure"],
        true
    );
    control(&emulator, set(false)).await;
    assert!(
        control(&emulator, Control::Snapshot).await["buses"][0]["outputs"][0]
            .get("lamp_failure")
            .is_none()
    );
    control(&emulator, Control::ResetScenario).await;
    assert_eq!(
        control(&emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["lamp_failure"],
        true
    );
    emulator.close().await.unwrap();
}
