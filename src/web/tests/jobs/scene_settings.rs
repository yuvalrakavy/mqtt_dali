use super::*;

#[tokio::test]
async fn scene_settings_http_jobs_enforce_strict_payloads_original_tokens_and_snapshot_privacy() {
    use crate::{atx::transactions::RepeatPolicy, emulator::pty};
    let emulator = pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":91}]}]}))
        .unwrap(),
    )
    .await
    .unwrap();
    let fixture =
        Fixture::configured(|s| s.with_uart(&emulator.path, RepeatPolicy::OneTerminal)).await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            None,
            "create_scene",
            json!({"bus":0,"slot":0,"name":"Evening","expected_revision":1}),
        ),
    )
    .await;
    let target = "PrivateBus.Outputs.PrivateLight";
    let scene = "PrivateBus.Scenes.Evening";
    let desired = json!({"level":{"kind":"value","value":42},"color":{"kind":"mask"}});
    for (name, scope, token, params) in [
        (
            "set_scene_row",
            scene,
            2,
            json!({"fixture":target,"setting":desired,"extra":1,"expected_revision":2}),
        ),
        (
            "set_scene_row",
            scene,
            2,
            json!({"fixture":target,"setting":{"level":{"kind":"mask"},"color":{"kind":"temperature"}},"expected_revision":2}),
        ),
        (
            "set_scene_row",
            scene,
            2,
            json!({"fixture":target,"setting":desired}),
        ),
        (
            "set_scene_row",
            scene,
            1,
            json!({"fixture":target,"setting":desired,"expected_revision":2}),
        ),
        (
            "set_scene_row",
            target,
            1,
            json!({"fixture":target,"setting":desired,"expected_revision":2}),
        ),
        (
            "set_scene_row",
            scene,
            2,
            json!({"fixture":scene,"setting":desired,"expected_revision":2}),
        ),
        (
            "set_scene_row",
            scene,
            2,
            json!({"fixture":target,"setting":desired,"expected_revision":1}),
        ),
        (
            "capture_scene_row",
            target,
            1,
            json!({"slot":0,"extra":true}),
        ),
        ("capture_scene_row", scene, 2, json!({"slot":0})),
        (
            "adopt_scene_row",
            scene,
            2,
            json!({"fixture":target,"reviewed":{},"expected_revision":2}),
        ),
    ] {
        assert_ne!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(token, Some(scope), name, params)
            )
            .await
            .status,
            202,
            "HTTP scene setting accepted invalid parameters or original authority"
        );
    }
    let saved = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some(scene),
            "set_scene_row",
            json!({"fixture":target,"setting":desired,"expected_revision":2}),
        ),
    )
    .await;
    assert_eq!(saved["outcome"], "verified");
    assert_eq!(saved["revision"], 3);
    assert_eq!(saved["target_generation"], format!("{EPOCH}.2"));
    assert_eq!(saved["fixture_generation"], format!("{EPOCH}.1"));
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await
        .json();
    assert!(
        snapshot["inventory"]["Inventory"]["buses"][0]["scenes"][0]
            .get("contents")
            .is_none(),
        "HTTP snapshot leaked private scene settings"
    );
    let disk: Value =
        serde_json::from_slice(&fs::read(fixture.path.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(
        disk["buses"][0]["scenes"][0]["contents"][0]["setting"],
        desired
    );
    let observed = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(target), "read_scene", json!({"slot":0})),
    )
    .await;
    assert_eq!(observed["saved"], desired);
    assert_eq!(observed["comparison"], "matching");
    let capture = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(target), "capture_scene_row", json!({"slot":0})),
    )
    .await;
    assert_eq!(capture["candidate"]["level"]["value"], 91);
    let reviewed = observed["stored"].clone();
    let repaired = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some(scene),
            "repair_scene_row",
            json!({"fixture":target,"reviewed":reviewed,"expected_revision":3}),
        ),
    )
    .await;
    assert_eq!(repaired["outcome"], "verified");
    let adopted = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some(scene),
            "adopt_scene_row",
            json!({"fixture":target,"reviewed":reviewed,"expected_revision":3}),
        ),
    )
    .await;
    assert_eq!(adopted["outcome"], "adopted");
    assert_eq!(adopted["revision"], 3);
    assert_eq!(
        fixture
            .request("GET", "/api/snapshot", &[], "")
            .await
            .status,
        401
    );
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}
