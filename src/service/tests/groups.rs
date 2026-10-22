use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

#[path = "groups_cases.rs"]
mod cases;

const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

async fn call(
    broker: &FakeBroker,
    body: serde_json::Value,
    token: u64,
    topic: &str,
) -> serde_json::Value {
    assert!(broker.publish(
        "DALI/Command/Controller",
        body.to_string(),
        &SendOptions::default()
            .response_topic(topic)
            .correlation_data(topic.as_bytes().to_vec())
            .user_property("target-generation", context(token).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(topic).is_empty())
            .await,
        "group command did not complete"
    );
    let replies = broker.received_on(topic);
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0].user_property("target-generation"),
        context(token).generation()
    );
    assert_eq!(replies[0].user_property("traceparent"), Some(TRACE));
    assert_eq!(
        replies[0].correlation_data().unwrap().as_ref(),
        topic.as_bytes()
    );
    serde_json::from_slice(&replies[0].payload).unwrap()
}

#[tokio::test]
async fn group_assignment_saves_and_verifies_one_bit_without_clearing_unmanaged_groups() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Other");
    let mut inventory = fixture.disk();
    inventory.buses[0].groups[0].members.clear();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let scenario = serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"fixture","short_address":0,"level":100,"groups":32770},
        {"id":"other","short_address":1,"level":80,"groups":4}]}]}))
    .unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
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
    let reply = call(
        &broker,
        json!({"command":"assign_group","target":"Bus1.Outputs.Light1","group":0,
        "member":true,"expected_revision":1}),
        1,
        "test/group-assign",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["outcome"] == "verified",
        "single-bit group assignment was not verified: {reply}"
    );
    assert_eq!(
        reply["value"]["revision"], 2,
        "group assignment did not durably save desired intent"
    );
    assert_eq!(reply["value"]["configuration_changed"], true);
    assert_eq!(
        reply["value"]["probe"]["mask"], 32771,
        "assignment cleared unselected hardware group bits"
    );
    assert_eq!(reply["value"]["expected_mask"], 1);
    assert_eq!(reply["value"]["drift_mask"], 32770);
    assert_eq!(fixture.disk().buses[0].groups[0].members, vec![0]);
    assert_eq!(
        fixture.disk().buses[0].groups.len(),
        1,
        "ordinary assignment adopted unnamed hardware groups"
    );
    for target in ["Bus1.Outputs.Light1", "Bus1.Outputs.Other"] {
        assert_eq!(
            before
                .projections()
                .into_iter()
                .find(|p| p.target == target),
            handle
                .snapshot()
                .unwrap()
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
        );
    }
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["groups"], 32771,
        "assignment cleared unselected hardware group bits"
    );
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 100);
    assert_eq!(physical["buses"][0]["outputs"][1]["groups"], 4);
    assert_eq!(physical["buses"][0]["outputs"][1]["level"], 80);
    let repeat = call(
        &broker,
        json!({"command":"assign_group","target":"Bus1.Outputs.Light1","group":0,
        "member":true,"expected_revision":2}),
        1,
        "test/group-repeat",
    )
    .await;
    assert_eq!(
        repeat["value"]["revision"], 2,
        "no-op group assignment rotated inventory"
    );
    assert_eq!(repeat["value"]["configuration_changed"], false);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_read_headerless_mqtt_needs_no_completion_capability_and_ff_is_a_full_mask() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let scenario = serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"fixture","short_address":0,"level":100,"groups":65535}]}]}))
    .unwrap();
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
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":"read_groups","target":"Bus1.Outputs.Light1"}).to_string(),
        &SendOptions::default()
            .response_topic("test/group-read")
            .correlation_data("headerless")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/group-read")
                .is_empty())
            .await
    );
    let replies = broker.received_on("test/group-read");
    assert_eq!(replies.len(), 1);
    let reply: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
    assert!(
        reply.get("error").is_none(),
        "headerless group read required a mutation completion capability: {reply}"
    );
    assert_eq!(
        reply["value"]["probe"]["mask"], 65535,
        "FF membership bytes were treated as MASK"
    );
    assert_eq!(reply["value"]["probe"]["low"], 255);
    assert_eq!(reply["value"]["probe"]["high"], 255);
    assert_eq!(reply["value"]["probe"]["quality"], "complete");
    assert_eq!(reply["value"]["probe"]["stale"], false);
    assert_eq!(reply["value"]["expected_mask"], 1);
    assert_eq!(reply["value"]["drift_mask"], 65534);
    assert_eq!(
        reply["value"]["target_generation"],
        context(1).generation().unwrap()
    );
    assert!(replies[0].user_property("target-generation").is_none());
    assert!(replies[0].user_property("traceparent").is_none());
    assert_eq!(
        replies[0].correlation_data().unwrap().as_ref(),
        b"headerless"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
