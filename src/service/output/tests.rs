use super::*;
use crate::atx::transactions::RepeatPolicy;
use crate::emulator::{pty, session::Control};
use crate::service::tests::{context, deadline, start, wait_for, Fixture};
use crate::service::{Hardware, MetadataEdit};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn dt7_managed_base_power_reports_actual_binary_contact_state() {
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
    ] {
        let fixture = Fixture::new();
        let saved = fixture.disk();
        let scenario=serde_json::from_value(json!({"version":1,"repeat":profile,"buses":[{"number":0,"outputs":[{"id":"relay","short_address":0,"device_type":7}]}]})).unwrap();
        let emulator = pty::spawn(scenario).await.unwrap();
        let broker = mqtt_test_broker::FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service.with_uart(&emulator.path, policy), &handle, &broker).await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let target = "Bus1.Outputs.Light1";
        let initial = handle
            .read_output(target, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(initial.power(), Some(false));
        assert_eq!(initial.brightness(), Some(0.0));
        for (power, actual) in [(true, 254), (false, 0), (true, 254)] {
            let sample = handle
                .apply_output(
                    target,
                    Intent::new(Some(power), None).unwrap(),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap();
            assert_eq!(sample.quality, Quality::Current);
            assert_eq!(sample.power(), Some(power));
            assert_eq!(
                sample.brightness(),
                Some(f64::from(actual) / 254.0),
                "managed DT7 observation echoed a virtual dimmer level"
            );
            let physical = emulator
                .emulator
                .control(Control::Snapshot, Duration::from_secs(1))
                .await
                .unwrap();
            assert_eq!(physical["buses"][0]["outputs"][0]["level"], actual);
        }
        assert_eq!(
            fixture.disk(),
            saved,
            "DT7 power changed saved names or bindings"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn output_reply_and_cache_share_the_final_acceptance_sample() {
    let fixture = Fixture::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let (mut service, handle) = fixture.service();
    service.output_acceptance_hook = Some(std::sync::Arc::new(|hat| {
        hat.power_cycle_output_samples_for_test();
    }));
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let result = handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await;
    let sample = handle.snapshot().unwrap().outputs["Bus1.Outputs.Light1"].clone();
    assert_eq!(sample.quality, Quality::Stale);
    assert_eq!(
        sample.level(),
        Some(LevelValue::Known(
            crate::dali::gear::Level::new(100).unwrap()
        ))
    );
    assert!(
        matches!(result, Err(ServiceError::Rejected(_))),
        "caller accepted current output while the final cached sample was stale: {result:?}"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn output_reply_ready_before_expiry_is_uncertain_when_caller_resumes_late() {
    use std::{future::Future, task::Poll};
    let fixture = Fixture::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let until = Instant::now() + Duration::from_millis(150);
    let mut read = Box::pin(handle.read_output("Bus1.Outputs.Light1", context(1), until));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    wait_for(&handle, |s| s.outputs.contains_key("Bus1.Outputs.Light1")).await;
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(read.await, Err(ServiceError::Uncertain(_))),
        "ready output reply bypassed original caller expiry"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn missing_fixture_and_bus_loss_preserve_truth_and_do_not_need_mqtt() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    broker.refuse_connections(true);
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    emulator
        .emulator
        .control(
            Control::SetAddress {
                id: "ceiling-a".into(),
                short_address: None,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    let missing = handle
        .read_output("Bus1.Outputs.Light1", context(1), deadline())
        .await
        .unwrap();
    assert_eq!(missing.quality, Quality::Unavailable);
    assert!(!missing.available());
    assert_eq!(missing.power(), None, "missing fixture was reported off");
    assert!(matches!(
        handle
            .apply_output(
                "Bus1.Outputs.Light1",
                Intent::new(Some(true), None).unwrap(),
                context(1),
                deadline()
            )
            .await,
        Err(ServiceError::Uncertain(_))
    ));
    let healthy = handle
        .apply_output(
            "Bus1.Outputs.Light2",
            Intent::new(None, Some(1.0)).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        healthy.brightness(),
        Some(1.0),
        "missing fixture blocked a healthy target"
    );
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().buses[0]
            .outputs
            .len(),
        2,
        "missing fixture was removed from inventory"
    );
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    wait_for(&handle, |s| {
        s.outputs["Bus1.Outputs.Light2"].quality == Quality::Stale
    })
    .await;
    assert!(!handle.snapshot().unwrap().outputs["Bus1.Outputs.Light2"].available());
    assert!(handle
        .read_output("Bus1.Outputs.Light2", context(1), deadline())
        .await
        .is_err());
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("offline bus".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: true,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    wait_for(&handle,|s|matches!(&s.hardware,Hardware::Ready{buses,..} if buses.iter().any(|bus|bus.bus==0&&bus.status==2))).await;
    let restored = handle
        .read_output("Bus1.Outputs.Light2", context(1), deadline())
        .await
        .unwrap();
    assert_eq!(restored.quality, Quality::Current);
    assert!(running.stop().await);
    assert!(
        handle
            .snapshot()
            .unwrap()
            .outputs
            .values()
            .all(|o| !o.available()),
        "closed service retained available outputs"
    );
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn output_service_uses_real_readback_and_keeps_original_generations() {
    let fixture = Fixture::new();
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let target = "Bus1.Outputs.Light1";
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    let initial = handle
        .read_output(target, context(1).with_traceparent(Some(trace)), deadline())
        .await
        .unwrap();
    assert_eq!(
        initial.level(),
        Some(LevelValue::Known(
            crate::dali::gear::Level::new(100).unwrap()
        ))
    );
    assert_eq!(initial.quality, Quality::Current);
    assert_eq!(
        initial.context.traceparent(),
        Some(trace),
        "queued output read lost its original trace without OTel"
    );
    for (power, brightness, raw) in [
        (None, Some(1.0), 254),
        (Some(false), None, 0),
        (Some(true), None, 254),
        (Some(true), Some(0.5), 127),
    ] {
        let observed = handle
            .apply_output(
                target,
                Intent::new(power, brightness).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            observed.level(),
            Some(LevelValue::Known(
                crate::dali::gear::Level::new(raw).unwrap()
            ))
        );
        assert_eq!(observed.power(), Some(raw > 0));
        let physical = emulator
            .emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            physical["buses"][0]["outputs"][0]["level"],
            json!(raw),
            "service reply echoed intent without changing the independent emulator"
        );
    }
    let old = handle.snapshot().unwrap().outputs[target].clone();
    handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(
        !handle.snapshot().unwrap().outputs.contains_key(target),
        "rename carried stale observations into a new token"
    );
    assert!(handle
        .read_output("Bus1.Outputs.Kitchen", old.context.clone(), deadline())
        .await
        .is_err());
    assert!(
        handle
            .apply_output(
                "Bus1.Outputs.Kitchen",
                Intent::new(Some(false), None).unwrap(),
                old.context.clone(),
                deadline()
            )
            .await
            .is_err(),
        "retired output token reached physical hardware"
    );
    assert!(
        handle
            .apply_output(
                "Bus1.Outputs.Kitchen",
                Intent::new(Some(false), None).unwrap(),
                RequestContext::default(),
                deadline()
            )
            .await
            .is_err(),
        "missing output token reached physical hardware"
    );
    let unchanged = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        unchanged["buses"][0]["outputs"][0]["level"],
        json!(127),
        "refused output requests changed physical state"
    );
    let fresh = handle
        .read_output(
            "Bus1.Outputs.Kitchen",
            RequestContext::default(),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(fresh.context.generation(), context(2).generation());
    assert_eq!(
        fresh.context.traceparent(),
        None,
        "headerless read inherited another caller's trace"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
