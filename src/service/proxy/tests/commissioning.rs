use super::*;
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

async fn assign(broker: &FakeBroker, revision: u64, topic: &str) -> Value {
    assert!(broker.send_with(
        "DALI/Command/Controller",
        json!({"command":"assign_next_output","bus":0,"expected_revision":revision}).to_string(),
        &SendOptions::default()
            .response_topic(topic)
            .user_property("target-generation", context(revision).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b.received_on(topic).is_empty())
            .await,
        "address assignment did not return a bounded report"
    );
    serde_json::from_slice(&broker.received_on(topic).last().unwrap().payload).unwrap()
}

#[tokio::test]
async fn commissioning_mqtt_assigns_only_the_unaddressed_output_and_preserves_original_reply() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let emulator = pty::spawn(
        serde_json::from_str(include_str!("../../../../examples/emulator/basic-i4.json")).unwrap(),
    )
    .await
    .unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::OneTerminal),
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
    let before = handle.snapshot().unwrap().model;
    let reply = assign(&broker, 1, "test/commission-new").await;
    assert!(
        reply.get("error").is_none(),
        "new unaddressed output was not assigned: {reply}"
    );
    assert_eq!(reply["value"]["outcome"], "assigned");
    assert_eq!(
        reply["value"]["address"], 2,
        "assignment used a saved or physically occupied address"
    );
    assert_eq!(reply["value"]["target"], "Bus1.Outputs.Light2");
    let after = handle.snapshot().unwrap().model;
    assert_eq!(after.inventory().revision, 2);
    for target in ["Bus1.Outputs.Light1", "Bus1.Groups.All"] {
        assert_eq!(
            before
                .projections()
                .into_iter()
                .find(|p| p.target == target),
            after.projections().into_iter().find(|p| p.target == target),
            "address assignment replaced an existing binding"
        );
    }
    let state = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let outputs = state["buses"][0]["outputs"].as_array().unwrap();
    for (id, address) in [("ceiling-a", 0), ("ceiling-b", 1), ("new-fixture", 2)] {
        assert_eq!(
            outputs.iter().find(|o| o["id"] == id).unwrap()["short_address"],
            address,
            "assignment changed the wrong physical fixture"
        );
    }
    assert_eq!(state["buses"][0]["inputs"][0]["short_address"], 0);
    let record = broker.received_on("test/commission-new").pop().unwrap();
    assert_eq!(
        record.user_property("target-generation"),
        context(1).generation()
    );
    assert_eq!(record.user_property("traceparent"), Some(TRACE));
    let repeated = assign(&broker, 2, "test/commission-empty").await;
    assert_eq!(
        repeated["value"]["outcome"], "no_unaddressed_output",
        "repeat assignment fabricated another fixture: {repeated}"
    );
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 2);
    assert_eq!(broker.received_on("test/commission-new").len(), 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn commissioning_mqtt_strict_arguments_refuse_before_hardware_admission() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (index,body) in [
        json!({"command":"assign_next_output","bus":0}),
        json!({"command":"assign_next_output","expected_revision":1}),
        json!({"command":"assign_next_output","bus":"0","expected_revision":1}),
        json!({"command":"assign_next_output","bus":0,"expected_revision":1,"mode":"all"}),
        json!({"command":"assign_next_output","bus":0,"expected_revision":1,"address":2}),
        json!({"command":"assign_next_output","bus":0,"expected_revision":1,"target":"Bus1.Outputs.Light1"}),
    ].into_iter().enumerate() {
        let topic=format!("test/commission-invalid/{index}");
        assert!(broker.send_with("DALI/Command/Controller",body.to_string(),&SendOptions::default().response_topic(&topic).user_property("target-generation",context(1).generation().unwrap())));
        assert!(broker.wait_until(Duration::from_secs(2),|b|!b.received_on(&topic).is_empty()).await);
        let reply:Value=serde_json::from_slice(&broker.received_on(&topic)[0].payload).unwrap();
        assert!(reply.get("error").is_some(),"malformed assignment was accepted: {body}");
        assert!(!reply["error"].as_str().unwrap().contains("HAT profile"),"malformed assignment reached physical admission");
        assert_eq!(handle.snapshot().unwrap().model.inventory().revision,1);
    }
    assert!(running.stop().await);
}
