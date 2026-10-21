use super::*;

#[tokio::test]
async fn mqtt_input_renames_persist_nested_names_and_refuse_retired_descendants() {
    let fixture = Fixture::new();
    let mut inventory = fixture.disk();
    inventory.buses[0].inputs = serde_json::from_value(json!([
        {"short_address":0,"name":"Panel","instances":[
            {"number":0,"name":"Key","instance_type":1}]}]))
    .unwrap();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(3))
            .await
    );
    for (target, name, expected, token) in [
        ("Bus1.Inputs.Panel.Instances.Key", "Door", 1, 1),
        ("Bus1.Inputs.Panel", "Wall", 2, 1),
    ] {
        let reply = format!("test/input-rename/{expected}");
        let payload = json!({"command":"rename","target":target,"name":name,
            "expected_revision":expected})
        .to_string();
        assert!(broker.publish(
            "DALI/Command/Controller",
            payload,
            &SendOptions::default()
                .user_property("target-generation", context(token).generation().unwrap())
                .response_topic(&reply)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(3), |b| !b
                    .received_on(&reply)
                    .is_empty())
                .await
        );
        let replies = broker.received_on(&reply);
        assert_eq!(replies.len(), 1);
        let value: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(
            value["value"]["revision"],
            expected + 1,
            "input rename failed: {value}"
        );
        assert!(value.get("error").is_none());
        assert_eq!(
            replies[0].user_property("target-generation"),
            context(token).generation()
        );
    }
    wait_for(&handle, |s| {
        s.phase == Phase::Ready && s.model.inventory().revision == 3
    })
    .await;
    let saved = fixture.disk();
    assert_eq!(saved.buses[0].inputs[0].name, "Wall");
    assert_eq!(saved.buses[0].inputs[0].instances[0].name, "Door");
    let inv = "DALI/State/Controller/__inventory";
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b.received_on(inv).iter().any(
                |m| serde_json::from_slice::<serde_json::Value>(&m.payload).is_ok_and(|v| v
                    ["Inventory"]["revision"]
                    == 3
                    && v["Inventory"]["buses"][0]["inputs"][0]["instances"][0]["target"]
                        == "Bus1.Inputs.Wall.Instances.Door")
            ))
            .await,
        "nested input inventory was not published after durable rename"
    );
    let stale = handle
        .edit(
            3,
            MetadataEdit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Stale".into(),
            },
            context(2),
            deadline(),
        )
        .await;
    assert!(
        matches!(stale, Err(ServiceError::Rejected(_))),
        "retired descendant generation renamed a current input"
    );
    assert_eq!(fixture.disk().revision, 3);
    assert_eq!(
        fs::read(&fixture.paths.legacy).unwrap(),
        b"legacy must not be touched"
    );
    assert!(running.stop().await);
}
