use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};
mod literal;

const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";
async fn call(
    broker: &FakeBroker,
    body: serde_json::Value,
    revision: u64,
    topic: &str,
) -> serde_json::Value {
    assert!(broker.publish(
        "DALI/Command/Controller",
        body.to_string(),
        &SendOptions::default()
            .response_topic(topic)
            .correlation_data(topic.as_bytes().to_vec())
            .user_property("target-generation", context(revision).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b.received_on(topic).is_empty())
            .await,
        "input assignment did not complete"
    );
    let replies = broker.received_on(topic);
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0].user_property("target-generation"),
        context(revision).generation()
    );
    assert_eq!(replies[0].user_property("traceparent"), Some(TRACE));
    assert_eq!(
        replies[0].correlation_data().unwrap().as_ref(),
        topic.as_bytes()
    );
    serde_json::from_slice(&replies[0].payload).unwrap()
}

async fn new_module() -> pty::PtyEmulator {
    pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "inputs":[{"id":"new-input","short_address":null,"instances":[1,2]}]}]}))
        .unwrap(),
    )
    .await
    .unwrap()
}
async fn still_unaddressed(emulator: &crate::emulator::session::Emulator) -> bool {
    let snapshot = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    snapshot["buses"][0]["inputs"][0]["short_address"].is_null()
}

#[tokio::test]
async fn input_assignment_save_failure_and_uncertainty_never_program_even_after_recovery() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let emulator = new_module().await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected input reservation failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = handle.assign_next_input(0, 1, context(1), deadline()).await;
        if at == Step::BeforeRename(Stage::Reservation) {
            assert!(matches!(result, Err(ServiceError::Rejected(_))));
            assert_eq!(fixture.disk().revision, 1);
        } else {
            assert!(matches!(result, Err(ServiceError::Uncertain(_))));
            assert!(matches!(
                handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
            assert_eq!(
                handle
                    .recover(context(1), deadline())
                    .await
                    .unwrap()
                    .model
                    .inventory()
                    .revision,
                2
            );
        }
        assert!(
            still_unaddressed(&emulator.emulator).await,
            "unconfirmed input reservation programmed hardware"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn input_assignment_held_save_bus_cycle_and_cancellation_preserve_reservation_without_program(
) {
    for cancel in [false, true] {
        let fixture = Fixture::new();
        let hold = Hold::new(Step::AfterRename(Stage::Mapping));
        let emulator = new_module().await;
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        service.hooks = hold.hooks.clone();
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let client = handle.clone();
        let pending = tokio::spawn(async move {
            client
                .assign_next_input(0, 1, context(1), Instant::now() + Duration::from_secs(5))
                .await
        });
        hold.entered().await;
        assert!(
            still_unaddressed(&emulator.emulator).await,
            "input PROGRAM preceded durable reservation"
        );
        assert!(
            matches!(
                handle
                    .read_output("Bus1.Outputs.Light1", context(1), deadline())
                    .await,
                Err(ServiceError::Busy)
            ),
            "input reservation released its permit during save"
        );
        if !cancel {
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
        }
        if cancel {
            pending.abort();
            let _ = pending.await;
            wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
            hold.release.store(true, Ordering::SeqCst);
            let until = deadline();
            let recovered = loop {
                assert!(Instant::now() < until, "input save recovery never settled");
                match handle.recover(context(1), until).await {
                    Ok(snapshot) => break snapshot,
                    Err(ServiceError::Uncertain(reason)) if reason.contains("worker is busy") => {
                        tokio::time::sleep(Duration::from_millis(5)).await
                    }
                    other => panic!("unexpected input recovery result: {other:?}"),
                }
            };
            assert_eq!(recovered.model.inventory().revision, 2);
            tokio::time::timeout(Duration::from_secs(1), async {
                while handle.permit.available_permits() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        } else {
            hold.release.store(true, Ordering::SeqCst);
            assert_eq!(
                pending.await.unwrap().unwrap().outcome,
                crate::service::commissioning::AssignmentOutcome::ReservedButUnconfirmed,
                "input save refreshed a stale bus candidate"
            );
        }
        assert_eq!(fixture.disk().revision, 2);
        assert_eq!(fixture.disk().buses[0].inputs.len(), 1);
        assert!(
            still_unaddressed(&emulator.emulator).await,
            "abandoned input candidate was programmed after save"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn input_assignment_rejects_bad_authority_payload_and_full_input_capacity() {
    let fixture = Fixture::new();
    let emulator = new_module().await;
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    for (bus, expected, cause) in [
        (1, 1, context(1)),
        (4, 1, context(1)),
        (0, 2, context(1)),
        (0, 1, context(2)),
        (0, 1, RequestContext::default()),
    ] {
        assert!(
            handle
                .assign_next_input(bus, expected, cause, deadline())
                .await
                .is_err(),
            "input assignment admitted invalid authority"
        );
    }
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (index,body) in [json!({"command":"assign_next_input","bus":0}),json!({"command":"assign_next_input","bus":0,"expected_revision":1,"space":"inputs"}),json!({"command":"assign_next_input","bus":0,"expected_revision":1,"target":"Bus1.Outputs.Light1"})].into_iter().enumerate() {
        assert!(call(&broker,body,1,&format!("test/input-invalid-{index}")).await.get("error").is_some(),"input assignment accepted an invalid payload");
    }
    assert_eq!(fixture.disk().revision, 1);
    assert!(still_unaddressed(&emulator.emulator).await);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();

    let mut inventory = fixture.disk();
    inventory.buses[0].inputs = (0..64)
        .map(|a| {
            serde_json::from_value(
                json!({"short_address":a,"name":format!("Missing{a}"),"instances":[]}),
            )
            .unwrap()
        })
        .collect();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    let emulator = new_module().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let report = handle
        .assign_next_input(0, 1, context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        report.outcome,
        crate::service::commissioning::AssignmentOutcome::NoFreeAddress,
        "saved missing inputs lost their address reservations"
    );
    assert_eq!(fixture.disk().revision, 1);
    assert!(still_unaddressed(&emulator.emulator).await);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn input_assignment_mqtt_reserves_fresh_module_then_discovery_adds_only_real_instances() {
    let fixture = Fixture::new();
    fixture.add_button();
    let scenario=serde_json::from_value(json!({"version":1,"seed":42,"buses":[{"number":0,
        "outputs":[{"id":"light","short_address":0,"level":100,"groups":5,"scenes":[50,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255]}],
        "inputs":[{"id":"panel","short_address":0,"instances":[1],"values":[{"instance":0,"resolution":1,"bytes":[255]}]},
        {"id":"untracked","short_address":1,"instances":[2]},
        {"id":"new-input","short_address":null,"instances":[1,2,3,4]}]}]})).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
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
    let before = handle.snapshot().unwrap().model.projections();
    let reply = call(
        &broker,
        json!({"command":"assign_next_input","bus":0,"expected_revision":1}),
        1,
        "test/input-assign",
    )
    .await;
    assert!(
        reply.get("error").is_none() && reply["value"]["outcome"] == "assigned",
        "new unaddressed input was not assigned: {reply}"
    );
    assert_eq!(reply["value"]["space"], "inputs");
    assert_eq!(
        reply["value"]["address"], 2,
        "input assignment reused a saved or physically occupied address"
    );
    assert_eq!(reply["value"]["target"], "Bus1.Inputs.Input1");
    assert_eq!(reply["value"]["revision"], 2);
    let disk = fixture.disk();
    let module = disk.buses[0]
        .inputs
        .iter()
        .find(|i| i.short_address == 2)
        .unwrap();
    assert_eq!(module.name, "Input1");
    assert!(
        module.instances.is_empty(),
        "input assignment invented undiscovered instance types"
    );
    let assigned = handle.snapshot().unwrap().model.projections();
    for old in &before {
        assert!(
            assigned.contains(old),
            "input addressing changed an existing binding"
        );
    }
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let row = &physical["buses"][0];
    assert_eq!(row["outputs"][0]["short_address"], 0);
    assert_eq!(row["outputs"][0]["level"], 100);
    assert_eq!(row["outputs"][0]["groups"], 5);
    assert_eq!(row["outputs"][0]["scenes"][0], 50);
    assert_eq!(row["inputs"][0]["short_address"], 0);
    assert_eq!(row["inputs"][0]["values"][0]["bytes"], json!([255]));
    assert_eq!(row["inputs"][1]["short_address"], 1);
    assert_eq!(
        row["inputs"][2]["short_address"], 2,
        "raw input short-address programming targeted the wrong module"
    );
    let discovered=call(&broker,json!({"command":"discover_address","bus":0,"space":"inputs","address":2,"expected_revision":2}),2,"test/input-assigned-discover").await;
    assert_eq!(
        discovered["value"]["revision"], 3,
        "assigned input instance discovery failed: {discovered}"
    );
    assert_eq!(discovered["value"]["added_items"], 4);
    let after = handle.snapshot().unwrap().model.projections();
    let parent = assigned
        .iter()
        .find(|p| p.target == "Bus1.Inputs.Input1")
        .unwrap();
    assert!(
        after.contains(parent),
        "instance discovery replaced the assigned input module"
    );
    for instance in &fixture.disk().buses[0]
        .inputs
        .iter()
        .find(|i| i.short_address == 2)
        .unwrap()
        .instances
    {
        assert_eq!(instance.instance_type, instance.number + 1);
        assert_eq!(instance.name, format!("Input{}", instance.number + 1));
    }
    let repeat = call(
        &broker,
        json!({"command":"assign_next_input","bus":0,"expected_revision":3}),
        3,
        "test/input-assign-empty",
    )
    .await;
    assert_eq!(
        repeat["value"]["outcome"], "no_unaddressed_input",
        "repeat assignment invented an input: {repeat}"
    );
    assert_eq!(fixture.disk().revision, 3);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
