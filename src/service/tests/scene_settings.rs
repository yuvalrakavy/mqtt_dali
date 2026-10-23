use super::groups::call;
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

const OUTPUT: &str = "Bus1.Outputs.Light1";
const SCENE: &str = "Bus1.Scenes.Evening";
mod cases;
mod literal;
mod peer;
mod switching;
fn save_scene(fixture: &Fixture) {
    let mut value = serde_json::to_value(fixture.disk()).unwrap();
    value["buses"][0]["scenes"] = json!([{"slot":0,"name":"Evening"}]);
    fs::write(&fixture.paths.current, serde_json::to_vec(&value).unwrap()).unwrap();
}
fn base_setting(level: u8) -> serde_json::Value {
    json!({"level":{"kind":"value","value":level},"color":{"kind":"mask"}})
}

#[tokio::test]
async fn scene_settings_mqtt_saves_programs_inspects_captures_adopts_and_repairs_without_activation(
) {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let emulator = pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{
            "number":0,"outputs":[{"id":"fixture","short_address":0,"level":100}]
        }]}))
        .unwrap(),
    )
    .await
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
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
    let desired = base_setting(0);
    let result = call(
        &broker,
        json!({"command":"set_scene_row","target":SCENE,
        "fixture":OUTPUT,"setting":desired,"expected_revision":1}),
        1,
        "test/scene-settings/set",
    )
    .await;
    assert_eq!(
        result["value"]["outcome"], "verified",
        "row programming failed: {result}"
    );
    assert_eq!(result["value"]["revision"], 2);
    assert_eq!(result["value"]["configuration_changed"], true);
    assert_eq!(
        fixture.disk().buses[0].scenes[0].contents[0].setting,
        serde_json::from_value(desired.clone()).unwrap()
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["scenes"][0], 0);
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 100,
        "scene editing activated brightness"
    );
    let observed = call(
        &broker,
        json!({"command":"read_scene","target":OUTPUT,"slot":0}),
        1,
        "test/scene-settings/read",
    )
    .await;
    assert_eq!(observed["value"]["saved"], desired);
    assert_eq!(observed["value"]["comparison"], "matching");
    let capture = call(
        &broker,
        json!({"command":"capture_scene_row","target":OUTPUT,"slot":0}),
        1,
        "test/scene-settings/capture",
    )
    .await;
    assert_eq!(
        capture["value"]["candidate"],
        base_setting(100),
        "capture used stored instead of actual values"
    );
    assert_eq!(fixture.disk().revision, 2, "capture saved without review");
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":"capture_scene_row","target":OUTPUT,"slot":0}).to_string(),
        &SendOptions::default().response_topic("test/scene-settings/tokenless-capture")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/scene-settings/tokenless-capture")
                .is_empty())
            .await
    );
    let tokenless: serde_json::Value = serde_json::from_slice(
        &broker.received_on("test/scene-settings/tokenless-capture")[0].payload,
    )
    .unwrap();
    assert_eq!(
        tokenless["value"]["candidate"],
        base_setting(100),
        "capture required mutation authority/completion for a read"
    );
    let repeat = call(
        &broker,
        json!({"command":"set_scene_row","target":SCENE,
        "fixture":OUTPUT,"setting":desired,"expected_revision":2}),
        1,
        "test/scene-settings/noop",
    )
    .await;
    assert_eq!(repeat["value"]["outcome"], "verified");
    assert_eq!(repeat["value"]["revision"], 2);
    assert_eq!(repeat["value"]["configuration_changed"], false);
    let reviewed = observed["value"]["stored"].clone();
    let adopted = call(
        &broker,
        json!({"command":"adopt_scene_row","target":SCENE,
        "fixture":OUTPUT,"reviewed":reviewed,"expected_revision":2}),
        1,
        "test/scene-settings/adopt",
    )
    .await;
    assert_eq!(adopted["value"]["outcome"], "adopted");
    let repaired = call(
        &broker,
        json!({"command":"repair_scene_row","target":SCENE,
        "fixture":OUTPUT,"reviewed":reviewed,"expected_revision":2}),
        1,
        "test/scene-settings/repair",
    )
    .await;
    assert_eq!(repaired["value"]["outcome"], "verified");
    let public: serde_json::Value =
        serde_json::from_slice(&handle.snapshot().unwrap().model.state_bytes().unwrap()).unwrap();
    assert!(public["Inventory"]["buses"][0]["scenes"][0]
        .get("contents")
        .is_none());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
