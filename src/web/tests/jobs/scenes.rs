use super::*;

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
