use super::*;
use crate::service::tests::deadline;
mod literal;

const TARGET: &str = "Bus1.Groups.All";
const STATE: &str = "DALI/State/Controller/Bus1.Groups.All";
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

async fn boot(
    fixture: &Fixture,
    scenario: Value,
) -> (
    pty::PtyEmulator,
    FakeBroker,
    mqtt_bridge_kit::Running,
    ServiceHandle,
) {
    let emulator = pty::spawn(serde_json::from_value(scenario).unwrap())
        .await
        .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for topic in [
        "DALI/Command/Controller",
        "DALI/Desired/Controller/+",
        "DALI/Desired/Controller/+/+",
    ] {
        assert!(
            broker
                .wait_for_subscription(topic, Duration::from_secs(2))
                .await
        );
    }
    (emulator, broker, running, handle)
}

async fn values(broker: &FakeBroker, predicate: impl Fn(&Value) -> bool) -> Value {
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(STATE)
                .last()
                .is_some_and(
                    |m| serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| predicate(&v))
                ))
            .await,
        "group scalar actual evidence did not reach MQTT"
    );
    let messages = broker.received_on(STATE);
    serde_json::from_slice(&messages.last().unwrap().payload).unwrap()
}

async fn refresh(broker: &FakeBroker, response: &str, generation: Option<&str>) -> Value {
    let mut options = SendOptions::default().response_topic(response);
    if let Some(generation) = generation {
        options = options.user_property("target-generation", generation);
    }
    assert!(broker.send_with(
        "DALI/Command/Controller",
        json!({"target":TARGET,"command":"refresh"}).to_string(),
        &options
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on(response)
                .is_empty())
            .await
    );
    serde_json::from_slice(&broker.received_on(response)[0].payload).unwrap()
}

#[tokio::test]
async fn group_scalar_desired_uses_native_membership_and_publishes_actual_context() {
    let fixture = Fixture::new();
    let disk = serde_json::to_value(fixture.disk()).unwrap();
    let (emulator, broker, running, _handle) = boot(
        &fixture,
        json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"saved-member","short_address":0,"groups":1,"level":100},
        {"id":"untracked-member","short_address":3,"groups":1,"level":80},
        {"id":"other","short_address":4,"groups":2,"level":70}]}]}),
    )
    .await;
    assert!(broker.publish(
        &format!("DALI/Desired/Controller/{TARGET}/Brightness"),
        "0.25",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    let state = values(&broker, |v| {
        v["Available"] == true && v["Brightness"] == json!(64.0 / 254.0)
    })
    .await;
    assert_eq!(state["PowerState"], true);
    assert_eq!(state["Quality"], "current");
    assert_eq!(state["PowerMixed"], false);
    assert_eq!(state["BrightnessMixed"], false);
    assert_eq!(state["Coverage"], "saved_inventory");
    assert_eq!(state["KnownMembers"], 1);
    let messages = broker.received_on(STATE);
    assert_eq!(
        messages.last().unwrap().user_property("target-generation"),
        context(1).generation()
    );
    assert_eq!(
        messages.last().unwrap().user_property("traceparent"),
        Some(TRACE)
    );
    let actual = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(actual["buses"][0]["outputs"][0]["level"], 64);
    assert_eq!(
        actual["buses"][0]["outputs"][1]["level"], 64,
        "group scalar fanned out only to saved fixtures"
    );
    assert_eq!(actual["buses"][0]["outputs"][2]["level"], 70);
    assert_eq!(serde_json::to_value(fixture.disk()).unwrap(), disk);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_scalar_refresh_separates_mixed_power_and_brightness_and_clears_missing_values() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
    let (emulator, broker, running, handle) = boot(
        &fixture,
        json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"one","short_address":0,"groups":1,"level":100},
        {"id":"two","short_address":1,"groups":1,"level":150}]}]}),
    )
    .await;
    let reply = refresh(&broker, "test/group-mixed-levels", None).await;
    assert!(
        reply.get("error").is_none(),
        "group refresh was refused: {reply}"
    );
    let state = values(&broker, |v| {
        v["Available"] == true && v["BrightnessMixed"] == true
    })
    .await;
    assert_eq!(state["PowerState"], true);
    assert_eq!(state["PowerMixed"], false);
    assert!(
        state["Brightness"].is_null(),
        "mixed group invented an average brightness"
    );
    assert_eq!(state["KnownMembers"], 2);
    assert_eq!(
        state["MembershipMatches"], false,
        "actual membership was replaced by saved checkboxes"
    );
    handle
        .apply_output(
            "Bus1.Outputs.Light2",
            crate::dali::gear::Intent::new(Some(false), None).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(refresh(&broker, "test/group-mixed-power", None)
        .await
        .get("error")
        .is_none());
    let state = values(&broker, |v| {
        v["Available"] == true && v["PowerMixed"] == true
    })
    .await;
    assert!(
        state["PowerState"].is_null(),
        "mixed power retained old true in merged State"
    );
    assert!(state["Brightness"].is_null());
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "two".into(),
                short_address: None,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(refresh(&broker, "test/group-missing", None)
        .await
        .get("error")
        .is_none());
    let state = values(&broker, |v| {
        v["Available"] == false && v["Quality"] == "partial"
    })
    .await;
    for field in [
        "PowerState",
        "Brightness",
        "PowerMixed",
        "BrightnessMixed",
        "MembershipMatches",
    ] {
        assert!(
            state[field].is_null(),
            "partial group retained scalar evidence: {field}"
        );
    }
    assert_eq!(state["KnownMembers"], 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_scalar_invalid_maps_retired_tokens_and_disabled_bus_never_partially_write() {
    let fixture = Fixture::new();
    let (emulator, broker, running, handle) = boot(&fixture, json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"one","short_address":0,"groups":1,"level":100}]}]})).await;
    let current =
        SendOptions::default().user_property("target-generation", context(1).generation().unwrap());
    let mut cases = vec![];
    for payload in [
        json!({"Brightness":0.25,"Other":1}),
        json!({"PowerState":false,"Brightness":0.25}),
        json!({"PowerState":null}),
        json!({"Brightness":"0.25"}),
        json!({"Brightness":2}),
        json!({"PowerState":true,"Brightness":0}),
        json!({"PowerState":false,"ColorRed":1}),
        json!({"Brightness":0.25,"Available":true}),
    ] {
        cases.push((payload, current.clone()));
    }
    cases.push((json!({"Brightness":0.25}), SendOptions::default()));
    cases.push((
        json!({"Brightness":0.25}),
        SendOptions::default().user_property("target-generation", "retired.1"),
    ));
    for (index, (payload, options)) in cases.into_iter().enumerate() {
        let errors = broker.received_on("DALI/Error/Controller").len();
        assert!(broker.send_with(
            &format!("DALI/Desired/Controller/{TARGET}"),
            payload.to_string(),
            &options
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on("DALI/Error/Controller")
                    .len()
                    > errors)
                .await,
            "invalid group map was partially accepted: {payload}"
        );
        assert!(
            refresh(&broker, &format!("test/group-invalid/{index}"), None)
                .await
                .get("error")
                .is_none()
        );
        let actual = emulator
            .emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            actual["buses"][0]["outputs"][0]["level"], 100,
            "invalid group map changed native level"
        );
    }
    assert!(
        refresh(&broker, "test/group-retired-read", Some("retired.1"))
            .await
            .get("error")
            .is_some()
    );
    handle
        .edit(
            1,
            MetadataEdit::ConfigureBus {
                bus: 0,
                name: "Bus1".into(),
                enabled: false,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    let errors = broker.received_on("DALI/Error/Controller").len();
    assert!(broker.send_with(
        &format!("DALI/Desired/Controller/{TARGET}/Brightness"),
        "0.33",
        &current
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Error/Controller")
                .len()
                > errors)
            .await,
        "disabled bus accepted group scalar mutation"
    );
    assert!(refresh(&broker, "test/group-disabled-read", None)
        .await
        .get("error")
        .is_some());
    assert_eq!(
        emulator
            .emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap()["buses"][0]["outputs"][0]["level"],
        100
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_scalar_held_observation_cannot_accept_new_identical_report_after_bus_cycle() {
    use crate::service::tests::Pause;
    use std::sync::{atomic::Ordering, Arc};
    for cause in ["bus_cycle", "individual_write", "revision"] {
        let fixture = Fixture::new();
        let (emulator, _broker, running, handle) = boot(&fixture, json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"one","short_address":0,"groups":1,"level":100}]}]})).await;
        let pause = Arc::new(Pause::default());
        pause.armed.store(true, Ordering::Release);
        let mut proxy = DaliProxy {
            handle: handle.clone(),
            initial: routing::generations(&handle.snapshot().unwrap().model),
            reporter: None,
            read_cursor: 0,
            read_pause: Some(pause.clone()),
            snapshot_pause: None,
        };
        let read_context = RequestContext::default();
        let mut reading = Box::pin(proxy.read_scope(Scope::Target(TARGET.into()), &read_context));
        tokio::select! {
            result = &mut reading => panic!("group read did not suspend after actual reply: {result:?}"),
            result = tokio::time::timeout(Duration::from_secs(2), pause.entered.notified()) => result.unwrap(),
        }
        let original = handle.snapshot().unwrap().group_outputs[TARGET].clone();
        match cause {
            "bus_cycle" => {
                for powered in [false, true] {
                    emulator
                        .emulator
                        .control(
                            Control::BusPower { bus: 0, powered },
                            Duration::from_secs(1),
                        )
                        .await
                        .unwrap();
                }
                wait_for(&handle, |s| s.group_outputs[TARGET].stale && matches!(&s.hardware, Hardware::Ready { buses, .. } if buses.iter().any(|b|b.bus==0 && b.status==2))).await;
                let fresh = handle
                    .read_group_output(TARGET, context(1), deadline())
                    .await
                    .unwrap();
                assert_eq!(
                    &fresh,
                    original.report(),
                    "identical-report fixture changed public report"
                );
                assert!(!handle.snapshot().unwrap().group_outputs[TARGET].stale);
            }
            "individual_write" => {
                handle
                    .apply_output(
                        "Bus1.Outputs.Light1",
                        crate::dali::gear::Intent::new(None, Some(0.5)).unwrap(),
                        context(1),
                        deadline(),
                    )
                    .await
                    .unwrap();
            }
            _ => {
                handle
                    .edit(
                        1,
                        MetadataEdit::Description(Some("new root".into())),
                        context(1),
                        deadline(),
                    )
                    .await
                    .unwrap();
            }
        }
        pause.release.notify_one();
        assert!(
            matches!(reading.await, Err(OpError::Rejected(_))),
            "held group result accepted retired physical or mapping evidence: {cause}"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn group_scalar_all_polling_clips_group_budget_and_keeps_cursor() {
    let fixture = Fixture::new();
    let (mut service, handle) = fixture.service();
    let mut snapshot =
        crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    snapshot.hardware = Hardware::Ready {
        version: crate::atx::wire::Version {
            hardware: "01".into(),
            firmware: "32".into(),
            variant: "04".into(),
        },
        buses: vec![BusHealth {
            bus: 0,
            status: 2,
            channels: 4,
        }],
    };
    service.state.send_replace(Some(snapshot));
    let mut proxy = DaliProxy {
        handle,
        initial: routing::generations(&crate::service::tests::model()),
        reporter: None,
        read_cursor: 1,
        read_pause: None,
        snapshot_pause: None,
    };
    let ceiling = Instant::now() + Duration::from_secs(15);
    let actor = tokio::spawn(async move {
        let Message::GroupOutput(request) = service.receive.recv().await.unwrap() else {
            panic!("All cursor did not visit saved group");
        };
        let first = request.deadline;
        tokio::time::advance(Duration::from_secs(14)).await;
        group_output::finish(request, Err(ServiceError::Expired));
        let Message::Output(request) = service.receive.recv().await.unwrap() else {
            panic!("All cursor did not resume individual output");
        };
        let second = request.deadline;
        let output::Request { reply, _permit, .. } = request;
        drop(_permit);
        tokio::time::advance(Duration::from_secs(2)).await;
        let _ = reply.send(Err(ServiceError::Expired));
        (first, second)
    });
    assert!(matches!(
        proxy.read_all(&RequestContext::default()).await,
        Err(OpError::Rejected(_))
    ));
    let (first, second) = tokio::time::timeout(Duration::from_secs(1), actor)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first, ceiling,
        "group read renewed the original All ceiling"
    );
    assert_eq!(
        second, ceiling,
        "following output received a renewed All budget"
    );
    assert_eq!(
        proxy.read_cursor, 1,
        "group polling did not persist cursor progress before awaits"
    );
}
