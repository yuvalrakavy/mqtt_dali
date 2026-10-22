use super::*;

#[tokio::test]
async fn scene_saved_view_http_returns_dense_result_with_summary_feeds_and_strict_authority() {
    let names: Vec<String> = (0..64)
        .map(|i| format!("Light{i}{}", "x".repeat(620)))
        .collect();
    let desired = json!({"level":{"kind":"value","value":42},"color":{"kind":"mask"}});
    let outputs: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(i, n)| json!({"short_address":i,"name":n}))
        .collect();
    let inventory = json!({"schema_version":2,"revision":1,"inventory_epoch":EPOCH,"name":"Controller","buses":[{
        "bus":0,"name":"PrivateBus","enabled":false,"outputs":outputs,"scenes":[{"slot":4,"name":"Evening","contents":[{
            "output_origin":{"target":format!("PrivateBus.Outputs.{}", names[0]),"revision":1},"setting":desired
        }]}]
    }]});
    let fixture = Fixture::with_inventory(inventory, |s| s).await;
    let (cookie, csrf) = fixture.login().await;
    let scene = "PrivateBus.Scenes.Evening";
    let admission = submit(
        &fixture,
        &cookie,
        &csrf,
        &command(1, Some(scene), "read_scene_settings", json!({})),
    )
    .await;
    assert_eq!(
        admission.status, 202,
        "offline saved-view HTTP reader unavailable: {}",
        admission.body
    );
    let path = admission.header("location").unwrap();
    let result = finished(&fixture, &cookie, path).await;
    assert_eq!(
        result["status"], "succeeded",
        "dense saved-view result exceeded HTTP budget: {result}"
    );
    assert!(
        result["result"].to_string().len() > 32768,
        "dense fixture did not exercise result budget"
    );
    let view = &result["result"];
    assert_eq!(
        view["fixtures"].as_array().unwrap().len(),
        64,
        "saved view truncated fixtures"
    );
    assert_eq!(view["fixtures"][0]["saved"], desired);
    assert_eq!(view["fixtures"][63]["saved"], Value::Null);
    let list = fixture
        .request("GET", "/api/jobs", &[("Cookie", &cookie)], "")
        .await
        .json();
    assert!(
        list["jobs"][0]["result"].is_null(),
        "job list included full scene contents"
    );
    let (_, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    let snapshot = chunk(&mut stream).await.unwrap();
    assert!(
        !snapshot.contains("contents"),
        "SSE snapshot included saved contents"
    );
    let jobs = chunk(&mut stream).await.unwrap();
    assert!(
        !jobs.contains("fixtures"),
        "SSE job summary included full scene result"
    );
    for (token, target, params) in [
        (2, scene, json!({})),
        (1, scene, json!({"slot":4})),
        (1, "PrivateBus.Outputs.missing", json!({})),
        (1, "PrivateBus.Groups.Evening", json!({})),
    ] {
        assert_ne!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(token, Some(target), "read_scene_settings", params)
            )
            .await
            .status,
            202,
            "saved view ignored original authority or strict scope"
        );
    }
    assert_eq!(fixture.request("GET", path, &[], "").await.status, 401);
    drop(stream);
    fixture.close().await;
}
