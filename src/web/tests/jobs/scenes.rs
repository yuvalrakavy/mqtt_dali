use super::*;

#[tokio::test]
async fn scene_operations_http_jobs_share_strict_reads_recalls_and_blink_exclusion() {
    use crate::{
        atx::transactions::RepeatPolicy,
        emulator::{pty, session::Control},
    };
    let mut slots = vec![255; 16];
    slots[0] = 42;
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":91,"scenes":slots}]}]})).unwrap()).await.unwrap();
    let fixture =
        Fixture::configured(|s| s.with_uart(&emulator.path, RepeatPolicy::Unverified)).await;
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
    let stored = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(target), "read_scene", json!({"slot":0})),
    )
    .await;
    assert_eq!(
        stored["stored"]["level"],
        json!({"kind":"value","raw":42}),
        "HTTP scene read bypassed the shared owner"
    );
    for (name, target, token, params) in [
        ("read_scene", target, 1, json!({})),
        ("read_scene", target, 1, json!({"slot":16})),
        ("read_scene", target, 1, json!({"slot":false})),
        ("read_scene", target, 1, json!({"slot":0,"extra":true})),
        ("read_scene", scene, 2, json!({"slot":0})),
        ("recall_scene", target, 1, json!({"expected_revision":2})),
        ("recall_scene", scene, 2, json!({})),
        ("recall_scene", scene, 2, json!({"expected_revision":1})),
        (
            "recall_scene",
            scene,
            2,
            json!({"group":16,"expected_revision":2}),
        ),
        (
            "recall_scene",
            scene,
            2,
            json!({"group":null,"expected_revision":2}),
        ),
        (
            "recall_scene",
            scene,
            2,
            json!({"group":false,"expected_revision":2}),
        ),
        (
            "recall_scene",
            scene,
            2,
            json!({"extra":true,"expected_revision":2}),
        ),
    ] {
        assert_ne!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(token, Some(target), name, params)
            )
            .await
            .status,
            202,
            "HTTP scene operation accepted invalid parameters or authority"
        );
    }
    assert_eq!(
        submit(
            &fixture,
            &cookie,
            &csrf,
            &command(
                1,
                Some(scene),
                "recall_scene",
                json!({"expected_revision":2})
            )
        )
        .await
        .status,
        409,
        "HTTP recall replaced stale scene authority"
    );
    run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(target), "blink", json!({})),
    )
    .await;
    let refused = submit(
        &fixture,
        &cookie,
        &csrf,
        &command(
            2,
            Some(scene),
            "recall_scene",
            json!({"expected_revision":2}),
        ),
    )
    .await;
    assert_eq!(refused.status, 202);
    let result = finished(&fixture, &cookie, refused.header("location").unwrap()).await;
    assert_eq!(
        result["status"], "failed",
        "scene recall overwrote active identification"
    );
    run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(target), "stop_blink", json!({})),
    )
    .await;
    let recalled = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some(scene),
            "recall_scene",
            json!({"expected_revision":2}),
        ),
    )
    .await;
    assert_eq!(recalled["outcome"], "sent");
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 42);
    assert_eq!(
        fixture
            .handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .revision,
        2
    );
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn scene_metadata_http_jobs_save_names_descriptions_and_keep_original_item_authority() {
    let fixture = Fixture::new().await;
    let (cookie, csrf) = fixture.login().await;
    let create = run(
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
    assert_eq!(create["revision"], 2);
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            None,
            "describe",
            json!({"description":"Controller","expected_revision":2}),
        ),
    )
    .await;
    let described = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some("PrivateBus.Scenes.Evening"),
            "describe",
            json!({"description":"Native","expected_revision":3}),
        ),
    )
    .await;
    assert_eq!(described["revision"], 4);
    let renamed = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some("PrivateBus.Scenes.Evening"),
            "rename",
            json!({"name":"Movie","expected_revision":4}),
        ),
    )
    .await;
    assert_eq!(renamed["revision"], 5);
    let old = command(
        2,
        Some("PrivateBus.Scenes.Movie"),
        "describe",
        json!({"description":"stale","expected_revision":5}),
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &old).await.status,
        409,
        "HTTP scene metadata replaced a retired item token"
    );
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await
        .json();
    let scene = &snapshot["inventory"]["Inventory"]["buses"][0]["scenes"][0];
    assert_eq!(scene["name"], "Movie");
    assert_eq!(scene["description"], "Native");
    let disk: Value =
        serde_json::from_slice(&fs::read(fixture.path.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(
        disk["buses"][0]["scenes"][0]["name"], "Movie",
        "HTTP scene job did not persist metadata"
    );
    fixture.close().await;
}
