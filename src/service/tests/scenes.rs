use super::groups::call;
use super::*;

#[test]
fn scene_metadata_namespace_never_resolves_as_a_physical_output_or_input() {
    let model = model()
        .edit(
            1,
            &[Edit::AddScene {
                bus: 0,
                scene: crate::config::Scene {
                    slot: 0,
                    name: "Native".into(),
                    description: None,
                },
            }],
        )
        .unwrap();
    let snapshot = snapshot(model, Phase::Ready);
    assert!(
        matches!(output::resolve(&snapshot,"Bus1.Scenes.Native",&context(2),true),
        Err(ServiceError::Rejected(reason)) if reason=="individual output required")
    );
    assert!(
        matches!(crate::service::input::readback::resolve(&snapshot,"Bus1.Scenes.Native",&context(2)),
        Err(ServiceError::Rejected(reason)) if reason=="individual input instance required")
    );
}

#[tokio::test]
async fn scene_metadata_mqtt_rejects_unknown_malformed_duplicate_and_stale_parameters() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let created = call(
        &broker,
        json!({"command":"create_scene","bus":0,"slot":0,"name":"Named",
        "expected_revision":1}),
        1,
        "test/scene-strict-setup",
    )
    .await;
    assert_eq!(created["value"]["revision"], 2);
    let valid =
        json!({"command":"create_scene","bus":0,"slot":1,"name":"Second","expected_revision":2});
    let mut invalid = Vec::new();
    for (field, data) in [
        ("extra", json!(true)),
        ("bus", json!(4)),
        ("slot", json!(16)),
        ("slot", json!(0)),
        ("slot", json!(false)),
        ("slot", json!(-1)),
        ("slot", json!(1.5)),
        ("name", json!("Named")),
        ("name", json!(" ")),
        ("name", json!("a/b")),
        ("description", json!(true)),
        ("target", json!("Bus1.Outputs.Light1")),
        ("expected_revision", json!(1)),
    ] {
        let mut body = valid.clone();
        body[field] = data;
        invalid.push(body);
    }
    for field in ["bus", "slot", "name", "expected_revision"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        invalid.push(body);
    }
    for (index, body) in invalid.into_iter().enumerate() {
        let result = call(&broker, body, 2, &format!("test/scene-invalid/{index}")).await;
        assert!(
            result.get("error").is_some(),
            "invalid scene command parameters were accepted: {result}"
        );
        assert_eq!(fixture.disk().revision, 2);
    }
    let stale = call(
        &broker,
        valid,
        1,
        "test/scene-invalid/controller-generation",
    )
    .await;
    assert!(stale.get("error").is_some());
    for (index, body) in [
        json!({"command":"describe","target":"Bus1.Scenes.Named","expected_revision":2}),
        json!({"command":"describe","target":"Bus1.Scenes.Named","expected_revision":2,"description":1}),
        json!({"command":"remove_from_list","target":"Bus1.Scenes.Named","expected_revision":2,"clear_hardware":true}),
    ].into_iter().enumerate() {
        let result = call(
            &broker,
            body,
            2,
            &format!("test/scene-invalid/targeted-{index}"),
        )
        .await;
        assert!(result.get("error").is_some());
        assert_eq!(fixture.disk().revision, 2);
    }
    assert!(running.stop().await);
}

#[tokio::test]
async fn scene_metadata_mqtt_saves_with_original_tokens_and_retires_recreated_slots() {
    let fixture = Fixture::new();
    fixture.add_button();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let before = handle.snapshot().unwrap().model.projections();
    let created = call(
        &broker,
        json!({"command":"create_scene","bus":0,"slot":15,
        "name":"All","description":"Native","expected_revision":1}),
        1,
        "test/scene-create",
    )
    .await;
    assert_eq!(
        created["value"]["revision"], 2,
        "valid scene creation did not complete: {created}"
    );
    let output = handle.snapshot().unwrap();
    assert_eq!(
        output.hardware,
        Hardware::NotStarted,
        "scene metadata required UART"
    );
    assert!(output
        .model
        .projections()
        .iter()
        .any(|p| p.target == "Bus1.Scenes.All"));
    call(
        &broker,
        json!({"command":"describe","description":"Controller","expected_revision":2}),
        2,
        "test/scene-unrelated",
    )
    .await;
    let described = call(
        &broker,
        json!({"command":"describe","target":"Bus1.Scenes.All",
        "description":"Evening scene","expected_revision":3}),
        2,
        "test/scene-describe",
    )
    .await;
    assert_eq!(
        described["value"]["revision"], 4,
        "original scene generation was replaced by controller generation: {described}"
    );
    let renamed = call(
        &broker,
        json!({"command":"rename","target":"Bus1.Scenes.All",
        "name":"Evening","expected_revision":4}),
        2,
        "test/scene-rename",
    )
    .await;
    assert_eq!(
        renamed["value"]["revision"], 5,
        "scene rename failed: {renamed}"
    );
    let stale = call(
        &broker,
        json!({"command":"rename","target":"Bus1.Scenes.Evening",
        "name":"Wrong","expected_revision":5}),
        2,
        "test/scene-stale",
    )
    .await;
    assert!(
        stale.get("error").is_some(),
        "retired scene token mutated a renamed scene"
    );
    let cleared = call(
        &broker,
        json!({"command":"describe","target":"Bus1.Scenes.Evening",
        "description":null,"expected_revision":5}),
        5,
        "test/scene-clear-description",
    )
    .await;
    assert_eq!(cleared["value"]["revision"], 6);
    for projection in &before {
        assert!(
            handle
                .snapshot()
                .unwrap()
                .model
                .projections()
                .contains(projection),
            "scene metadata changed unrelated fixture/input/group bindings"
        );
    }
    let renamed_bus = call(
        &broker,
        json!({"command":"rename_bus","bus":0,"name":"Main",
        "expected_revision":6}),
        6,
        "test/scene-rename-bus",
    )
    .await;
    assert_eq!(renamed_bus["value"]["revision"], 7);
    let removed = call(
        &broker,
        json!({"command":"remove_from_list","target":"Main.Scenes.Evening",
        "expected_revision":7}),
        7,
        "test/scene-remove",
    )
    .await;
    assert_eq!(
        removed["value"]["revision"], 8,
        "scene removal lost original completion: {removed}"
    );
    let recreated = call(
        &broker,
        json!({"command":"create_scene","bus":0,"slot":15,"name":"Evening",
        "expected_revision":8}),
        8,
        "test/scene-recreate",
    )
    .await;
    assert_eq!(recreated["value"]["revision"], 9);
    let stale = call(
        &broker,
        json!({"command":"describe","target":"Main.Scenes.Evening",
        "description":"old lifetime","expected_revision":9}),
        7,
        "test/scene-old-lifetime",
    )
    .await;
    assert!(
        stale.get("error").is_some(),
        "recreated slot accepted its retired lifetime"
    );
    assert_eq!(fixture.disk().revision, 9);
    assert_eq!(
        fs::read(&fixture.paths.legacy).unwrap(),
        b"legacy must not be touched"
    );
    assert!(running.stop().await);
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        handle
            .snapshot()
            .unwrap()
            .model
            .projections()
            .iter()
            .any(|p| p.target == "Main.Scenes.Evening" && p.binding_revision == 9),
        "scene metadata did not survive reopen"
    );
    assert!(running.stop().await);
}
