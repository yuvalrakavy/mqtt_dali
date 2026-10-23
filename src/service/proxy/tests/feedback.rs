use super::*;
use crate::service::proxy::feedback as binding;
use crate::service::tests::deadline;

const MODULE: &str = "Bus1.Inputs.Panel";
const BUTTON: &str = "Bus1.Inputs.Panel.Instances.Button";
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

async fn boot() -> (
    Fixture,
    pty::PtyEmulator,
    FakeBroker,
    mqtt_bridge_kit::Running,
    ServiceHandle,
) {
    boot_paused(None).await
}
async fn boot_paused(
    pause: Option<std::sync::Arc<crate::service::tests::Pause>>,
) -> (
    Fixture,
    pty::PtyEmulator,
    FakeBroker,
    mqtt_bridge_kit::Running,
    ServiceHandle,
) {
    let fixture = Fixture::new();
    fixture.add_button();
    let scenario = serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":77}],
        "inputs":[{"id":"panel","short_address":0,"instances":[1],
            "values":[{"instance":0,"resolution":1,"bytes":[255]}],
            "feedback":[{"instance":null},{"instance":0}]}]}]}))
    .unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.proxy_snapshot_pause = pause;
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    (fixture, emulator, broker, running, handle)
}

async fn state(broker: &FakeBroker, target: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    let topic = format!("DALI/State/Controller/{target}");
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(&topic)
                .last()
                .is_some_and(
                    |m| serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| predicate(&v))
                ))
            .await,
        "feedback actual evidence did not reach MQTT State"
    );
    serde_json::from_slice(&broker.received_on(&topic).last().unwrap().payload).unwrap()
}

#[tokio::test]
async fn feedback_commands_publish_independent_module_and_instance_actual_state() {
    let (fixture, emulator, broker, running, handle) = boot().await;
    let original = fixture.disk();
    handle
        .read_input(BUTTON, context(1), deadline())
        .await
        .unwrap();
    let report = handle
        .set_input_feedback(
            BUTTON,
            1,
            true,
            context(1).with_traceparent(Some(TRACE)),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(report.outcome, "set");
    let active = state(&broker, BUTTON, |v| v["FeedbackActive"] == true).await;
    assert_eq!(active["FeedbackAvailable"], true);
    assert_eq!(active["FeedbackSupported"], true);
    assert_eq!(active["FeedbackCapabilityRaw"], 1);
    assert_eq!(active["FeedbackQuality"], "current");
    assert_eq!(
        active["Status"], true,
        "indicator operation changed the queried button measurement"
    );
    let module = handle
        .read_input_feedback(MODULE, context(1), deadline())
        .await
        .unwrap();
    assert_eq!(module.outcome, "current");
    state(&broker, MODULE, |v| v["FeedbackActive"] == false).await;
    assert_eq!(fixture.disk(), original);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

fn proxy(handle: &ServiceHandle) -> DaliProxy {
    DaliProxy {
        handle: handle.clone(),
        initial: routing::generations(&handle.snapshot().unwrap().model),
        reporter: None,
        read_cursor: 0,
        read_pause: None,
        snapshot_pause: None,
    }
}
async fn refresh_input(broker: &FakeBroker, target: &str, reply: &str) -> Value {
    assert!(broker.send_with(
        "DALI/Command/Controller",
        json!({"target":target,"command":"refresh"}).to_string(),
        &SendOptions::default()
            .response_topic(reply)
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b.received_on(reply).is_empty())
            .await
    );
    serde_json::from_slice(&broker.received_on(reply)[0].payload).unwrap()
}

#[tokio::test]
async fn feedback_desired_scalar_atomic_and_target_refresh_preserve_measurements() {
    let (fixture, emulator, broker, running, handle) = boot().await;
    assert!(refresh_input(&broker, BUTTON, "test/feedback-start")
        .await
        .get("error")
        .is_none());
    for (target, value, atomic) in [
        (BUTTON, true, false),
        (MODULE, true, true),
        (BUTTON, false, true),
    ] {
        let (topic, payload) = if atomic {
            (
                format!("DALI/Desired/Controller/{target}"),
                json!({"FeedbackActive":value}).to_string(),
            )
        } else {
            (
                format!("DALI/Desired/Controller/{target}/FeedbackActive"),
                value.to_string(),
            )
        };
        assert!(broker.send_with(
            &topic,
            payload,
            &SendOptions::default()
                .user_property("target-generation", context(1).generation().unwrap())
                .user_property("traceparent", TRACE)
        ));
        state(&broker, target, |s| s["FeedbackActive"] == value).await;
        let messages = broker.received_on(&format!("DALI/State/Controller/{target}"));
        assert_eq!(
            messages.last().unwrap().user_property("traceparent"),
            Some(TRACE)
        );
    }
    assert!(refresh_input(&broker, MODULE, "test/feedback-module")
        .await
        .get("error")
        .is_none());
    let merged = state(&broker, BUTTON, |s| s["FeedbackActive"] == false).await;
    assert_eq!(merged["Status"], true);
    assert_eq!(merged["InputValueRaw"], "01");
    let rows = projected_states(&handle.snapshot().unwrap());
    assert_eq!(rows.iter().filter(|s| s["target"] == BUTTON).count(), 1);
    assert_eq!(fixture.disk().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_validation_refuses_malformed_maps_and_retired_authority() {
    let (_fixture, emulator, _broker, running, handle) = boot().await;
    let p = proxy(&handle);
    for value in [
        json!({}),
        json!({"FeedbackActive":null}),
        json!({"FeedbackActive":"true"}),
        json!({"FeedbackActive":1}),
        json!({"Status":true}),
        json!({"FeedbackActive":true,"Status":true}),
    ] {
        assert!(
            p.validate_request(
                Some(BUTTON),
                &context(1),
                RequestKind::Apply(value.as_object().unwrap())
            )
            .is_err(),
            "invalid map accepted: {value}"
        );
    }
    let value = json!({"FeedbackActive":true});
    let values = value.as_object().unwrap();
    for target in ["Bus1.Outputs.Light1", "Bus1.Groups.All", "missing"] {
        assert!(p
            .validate_request(Some(target), &context(1), RequestKind::Apply(values))
            .is_err());
    }
    for cause in [RequestContext::default(), context(9)] {
        assert!(p
            .validate_request(Some(BUTTON), &cause, RequestKind::Apply(values))
            .is_err());
    }
    assert!(p
        .validate_request(Some(BUTTON), &context(1), RequestKind::Apply(values))
        .is_ok());
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_acceptance_requires_exact_capture_and_original_deadline() {
    use std::sync::Arc;
    let (_fixture, emulator, _broker, running, handle) = boot().await;
    handle
        .read_input_feedback(BUTTON, context(1), deadline())
        .await
        .unwrap();
    let snapshot = handle.snapshot().unwrap();
    let original = snapshot.feedbacks[BUTTON].clone();
    assert!(binding::accepted(&snapshot, BUTTON, &original, deadline()).is_ok());
    let equal = Arc::new((*original).clone());
    assert_eq!(*equal, *original);
    assert!(
        binding::accepted(&snapshot, BUTTON, &equal, deadline()).is_err(),
        "equal feedback data replaced exact owner capture"
    );
    assert!(
        binding::accepted(&snapshot, BUTTON, &original, Instant::now()).is_err(),
        "expired feedback capture accepted"
    );
    handle
        .read_input_feedback(BUTTON, context(1), deadline())
        .await
        .unwrap();
    assert!(
        binding::accepted(&handle.snapshot().unwrap(), BUTTON, &original, deadline()).is_err(),
        "retired same-value feedback capture accepted"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_trace_follows_latest_actual_family_and_unrelated_snapshot_replays() {
    const INPUT_TRACE: &str = "00-33333333333333333333333333333333-4444444444444444-01";
    let (_fixture, emulator, broker, running, handle) = boot().await;
    handle
        .read_input(
            BUTTON,
            context(1).with_traceparent(Some(INPUT_TRACE)),
            deadline(),
        )
        .await
        .unwrap();
    handle
        .read_input_feedback(BUTTON, context(1).with_traceparent(Some(TRACE)), deadline())
        .await
        .unwrap();
    for trace in [Some(TRACE), Some(INPUT_TRACE), None, Some(TRACE)] {
        if trace != Some(TRACE) {
            handle
                .read_input(BUTTON, context(1).with_traceparent(trace), deadline())
                .await
                .unwrap();
        } else {
            handle
                .read_input_feedback(BUTTON, context(1).with_traceparent(trace), deadline())
                .await
                .unwrap();
        }
        handle
            .read_output("Bus1.Outputs.Light1", context(1), deadline())
            .await
            .unwrap();
        let snapshot = handle.snapshot().unwrap();
        let row = state::projections(&snapshot)
            .into_iter()
            .find(|s| s.state.target == BUTTON)
            .unwrap();
        // BridgeKit suppresses unchanged values on the wire, but every replay
        // still replaces its remembered context. Check the whole replay here.
        assert_eq!(
            row.context.traceparent(),
            trace,
            "unrelated replay replaced latest aggregate input cause"
        );
        assert_eq!(row.state.values["FeedbackAvailable"], true);
        assert_eq!(row.state.values["Status"], true);
    }
    emulator
        .emulator
        .control(
            Control::InputEvent {
                bus: 0,
                frame: 0x008000,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    state(&broker, BUTTON, |s| {
        s["Status"] == false && s["FeedbackAvailable"] == true
    })
    .await;
    let messages = broker.received_on(&format!("DALI/State/Controller/{BUTTON}"));
    assert_eq!(
        messages.last().unwrap().user_property("traceparent"),
        None,
        "native input action inherited feedback trace"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_revision_and_native_address_clear_retire_cached_actual_state() {
    let (_fixture, emulator, broker, running, handle) = boot().await;
    for target in [MODULE, BUTTON] {
        handle
            .set_input_feedback(target, 1, true, context(1), deadline())
            .await
            .unwrap();
    }
    handle
        .clear_short_address(
            MODULE,
            1,
            context(1),
            Instant::now() + Duration::from_secs(6),
        )
        .await
        .unwrap();
    for target in [MODULE, BUTTON] {
        state(&broker, target, |s| {
            s["FeedbackQuality"] == "stale" && s["FeedbackActive"].is_null()
        })
        .await;
    }
    assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_projection_distinguishes_unknown_unsupported_and_false() {
    use std::sync::Arc;
    let (_fixture, emulator, _broker, running, handle) = boot().await;
    handle
        .read_input_feedback(BUTTON, context(1), deadline())
        .await
        .unwrap();
    let original = handle.snapshot().unwrap();
    for (quality, supported, active, expected) in [
        ("complete", Some(true), Some(false), "current"),
        ("complete", Some(false), None, "unsupported"),
        ("partial", None, None, "partial"),
    ] {
        let mut snapshot = original.clone();
        let capture = Arc::make_mut(snapshot.feedbacks.get_mut(BUTTON).unwrap());
        let values = capture.report.feedback.as_mut().unwrap();
        values.quality = quality;
        values.supported = supported;
        values.active = active;
        let row = state::projections(&snapshot)
            .into_iter()
            .find(|s| s.state.target == BUTTON)
            .unwrap();
        assert_eq!(row.state.values["FeedbackQuality"], expected);
        assert_eq!(
            row.state.values["FeedbackActive"],
            active.map_or(Value::Null, Value::Bool)
        );
        assert_eq!(row.state.values["FeedbackAvailable"], expected == "current");
    }
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_held_read_refuses_replaced_same_value_capture() {
    use crate::service::tests::Pause;
    use std::sync::{atomic::Ordering, Arc};
    let (_fixture, emulator, _broker, running, handle) = boot().await;
    let pause = Arc::new(Pause::default());
    pause.armed.store(true, Ordering::Release);
    let mut p = proxy(&handle);
    p.read_pause = Some(pause.clone());
    let cause = context(1);
    let mut reading = Box::pin(p.read_scope(Scope::Target(MODULE.into()), &cause));
    tokio::select! {
        result=&mut reading=>panic!("feedback read did not pause after final evidence: {result:?}"),
        result=tokio::time::timeout(Duration::from_secs(2),pause.entered.notified())=>result.unwrap(),
    }
    let original = handle.snapshot().unwrap().feedbacks[MODULE].clone();
    handle
        .read_input_feedback(MODULE, context(1), deadline())
        .await
        .unwrap();
    assert_eq!(
        original.report,
        handle.snapshot().unwrap().feedbacks[MODULE].report
    );
    pause.release.notify_one();
    assert!(
        reading.await.is_err(),
        "held feedback read accepted a replacement capture"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_inventory_revision_stales_only_feedback_and_native_reads_preserve_it() {
    let (_fixture, emulator, broker, running, handle) = boot().await;
    handle
        .set_input_feedback(BUTTON, 1, true, context(1), deadline())
        .await
        .unwrap();
    handle
        .read_input(BUTTON, context(1), deadline())
        .await
        .unwrap();
    handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    state(&broker, BUTTON, |s| {
        s["FeedbackActive"] == true && s["Status"] == true
    })
    .await;
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("revision".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    let value = state(&broker, BUTTON, |s| s["FeedbackQuality"] == "stale").await;
    assert!(value["FeedbackActive"].is_null());
    assert!(value["FeedbackSupported"].is_null());
    assert_eq!(value["Status"], true);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_all_refresh_samples_only_established_scopes_and_returns_unique_rows() {
    use std::sync::Arc;
    let (_fixture, emulator, _broker, running, handle) = boot().await;
    let mut p = proxy(&handle);
    p.read_scope(Scope::All, &RequestContext::default())
        .await
        .unwrap();
    assert!(
        handle.snapshot().unwrap().feedbacks.is_empty(),
        "cold All probed indicators without established interest"
    );
    handle
        .read_input_feedback(MODULE, context(1), deadline())
        .await
        .unwrap();
    let previous = handle.snapshot().unwrap().feedbacks[MODULE].clone();
    let rows = p
        .read_scope(Scope::All, &RequestContext::default())
        .await
        .unwrap();
    assert!(
        !Arc::ptr_eq(&previous, &handle.snapshot().unwrap().feedbacks[MODULE]),
        "All skipped a sampled feedback scope"
    );
    assert!(!handle.snapshot().unwrap().feedbacks.contains_key(BUTTON));
    for target in [MODULE, BUTTON] {
        assert_eq!(rows.iter().filter(|s| s.state.target == target).count(), 1);
    }
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_coalesced_watcher_uses_latest_same_value_input_cause() {
    use crate::service::tests::Pause;
    use std::sync::{atomic::Ordering, Arc};
    const INPUT_TRACE: &str = "00-33333333333333333333333333333333-4444444444444444-01";
    let pause = Arc::new(Pause::default());
    let (_fixture, emulator, broker, running, handle) = boot_paused(Some(pause.clone())).await;
    handle
        .read_input(
            BUTTON,
            context(1).with_traceparent(Some(INPUT_TRACE)),
            deadline(),
        )
        .await
        .unwrap();
    handle
        .read_input_feedback(BUTTON, context(1).with_traceparent(Some(TRACE)), deadline())
        .await
        .unwrap();
    state(&broker, BUTTON, |s| {
        s["FeedbackActive"] == false && s["Status"] == true
    })
    .await;
    pause.armed.store(true, Ordering::Release);
    handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), pause.entered.notified())
        .await
        .unwrap();
    handle
        .set_input_feedback(
            BUTTON,
            1,
            true,
            context(1).with_traceparent(Some(TRACE)),
            deadline(),
        )
        .await
        .unwrap();
    handle
        .read_input(
            BUTTON,
            context(1).with_traceparent(Some(INPUT_TRACE)),
            deadline(),
        )
        .await
        .unwrap();
    handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    pause.release.notify_one();
    state(&broker, BUTTON, |s| {
        s["FeedbackActive"] == true && s["Status"] == true
    })
    .await;
    let messages = broker.received_on(&format!("DALI/State/Controller/{BUTTON}"));
    assert_eq!(
        messages.last().unwrap().user_property("traceparent"),
        Some(INPUT_TRACE),
        "coalesced State replay resurrected older feedback trace"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn feedback_scalar_ignored_native_write_is_rejected_with_actual_false() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let fixture = Fixture::new();
    fixture.add_button();
    let mut pty = crate::service::hat::tests::Pty::new();
    let path = pty.path.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let sent = count.clone();
    let peer = tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(&mut pty.peer);
        let mut reader = BufReader::new(read);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap() == 0 {
                return;
            }
            let reply = match line.as_str() {
                "v\n" => "V013204\n",
                "d\n" => "D24\n",
                "h01A0\n" => "J64\n",
                "h0190\n" => "J00\n",
                "l010080\n" | "l010081\n" => "J01\n",
                "l01008C\n" => "JFF\n",
                "l01FE30\n" => "J00\n",
                "l01FE8E\n" => "J20\n",
                "l01FC4F\n" => "J01\n",
                "l01FC4E\n" => "N\n",
                "l01FC10\n" => {
                    sent.fetch_add(1, Ordering::SeqCst);
                    "N\n"
                }
                other => panic!("unexpected ignored-feedback native command: {other:?}"),
            };
            write.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    let errors = broker.received_on("DALI/Error/Controller").len();
    assert!(broker.send_with(
        &format!("DALI/Desired/Controller/{MODULE}/FeedbackActive"),
        "true",
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
    ));
    state(&broker, MODULE, |s| {
        s["FeedbackActive"] == false && s["FeedbackAvailable"] == true
    })
    .await;
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Error/Controller")
                .len()
                > errors)
            .await,
        "ignored feedback scalar write was acknowledged as confirmed"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(
        !peer.is_finished(),
        "literal feedback peer failed before its oracle"
    );
    assert!(running.stop().await);
    peer.abort();
    assert!(peer.await.unwrap_err().is_cancelled());
}
