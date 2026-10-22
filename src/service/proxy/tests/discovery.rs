use super::*;
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

async fn discover(
    broker: &FakeBroker,
    address: u8,
    space: &str,
    revision: u64,
    topic: &str,
) -> Value {
    assert!(broker.send_with("DALI/Command/Controller",
        json!({"command":"discover_address","bus":0,"space":space,"address":address,"expected_revision":revision}).to_string(),
        &SendOptions::default().response_topic(topic)
            .user_property("target-generation",context(revision).generation().unwrap())
            .user_property("traceparent",TRACE)));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(topic).is_empty())
            .await,
        "discovery command did not complete"
    );
    serde_json::from_slice(&broker.received_on(topic).last().unwrap().payload).unwrap()
}

#[tokio::test]
async fn discovery_mqtt_adopts_a_new_address_and_replies_under_its_original_generation() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let scenario =
        serde_json::from_str(include_str!("../../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
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
    let reply = discover(&broker, 1, "outputs", 1, "test/discovery-new").await;
    assert!(
        reply.get("error").is_none(),
        "new addressed light was not discovered/adopted: {reply}"
    );
    let snapshot = handle.snapshot().unwrap();
    assert_eq!(snapshot.model.inventory().revision, 2);
    assert_eq!(
        snapshot.model.inventory().buses[0].outputs[1].name,
        "Light2"
    );
    assert_eq!(
        snapshot.model.inventory().buses[0].outputs[1].short_address,
        1
    );
    assert_eq!(broker.received_on("test/discovery-new").len(), 1);
    let record = broker.received_on("test/discovery-new").pop().unwrap();
    assert_eq!(
        record.user_property("target-generation"),
        context(1).generation(),
        "discovery reply lost its original controller generation"
    );
    assert_eq!(record.user_property("traceparent"), Some(TRACE));
    let repeated = discover(&broker, 1, "outputs", 2, "test/discovery-repeat").await;
    assert!(repeated.get("error").is_none());
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().revision,
        2,
        "repeated discovery changed an existing lifetime"
    );
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().buses[0]
            .outputs
            .len(),
        2
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn discovery_mqtt_strict_parameters_reject_before_hardware_or_save() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let valid = json!({"command":"discover_address","bus":0,"space":"outputs","address":1,"expected_revision":1});
    let mut invalid = Vec::new();
    for field in ["bus", "space", "address", "expected_revision"] {
        let mut value = valid.clone();
        value.as_object_mut().unwrap().remove(field);
        invalid.push(value);
    }
    for (field, value) in [
        ("space", json!({"outputs":null})),
        ("space", json!("Outputs")),
        ("address", json!(64)),
        ("bus", json!(4)),
        ("bus", json!(1)),
        ("expected_revision", json!(2)),
        ("address", json!("1")),
        ("surprise", json!(true)),
        ("target", json!("Bus1.Outputs.Light1")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        invalid.push(body);
    }
    for (index, body) in invalid.into_iter().enumerate() {
        let topic = format!("test/discovery-invalid/{index}");
        assert!(broker.send_with(
            "DALI/Command/Controller",
            body.to_string(),
            &SendOptions::default()
                .response_topic(&topic)
                .user_property("target-generation", context(1).generation().unwrap())
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await
        );
        let reply: Value = serde_json::from_slice(&broker.received_on(&topic)[0].payload).unwrap();
        assert!(
            reply.get("error").is_some(),
            "invalid discovery parameters were accepted: {body}"
        );
        assert_ne!(
            reply["error"], "UART or bus is not ready",
            "invalid discovery reached physical admission: {body}"
        );
        assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    }
    assert!(running.stop().await);
}
