use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
    service::output::settings::Patch,
};
mod literal;
const TARGET: &str = "Bus1.Outputs.Light1";
fn limit() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
async fn physical(emulator: &crate::emulator::session::Emulator) -> serde_json::Value {
    emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()
}

#[tokio::test]
async fn managed_output_settings_mqtt_strict_params_original_trace_and_read_class() {
    let fixture = Fixture::new();
    let original = fs::read(&fixture.paths.current).unwrap();
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
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
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    for (index,(command,params,success)) in [
        ("read_output_settings",json!({}),true),
        ("configure_output",json!({"expected_revision":1,"settings":{"minimum_level":5,"power_on_level":255,"system_failure_level":0,"fade_time":15,"fade_rate":1}}),true),
        ("configure_output",json!({"expected_revision":1,"settings":{"minimum_level":null}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{"minimum_level":5,"fade_rate":null}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{"fade_rate":0}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{"fade_time":1.5}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{"physical_minimum":1}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{}}),false),
        ("configure_output",json!({"expected_revision":0,"settings":{"minimum_level":5}}),false),
        ("configure_output",json!({"expected_revision":1,"settings":{"minimum_level":5},"extra":true}),false),
        ("read_output_settings",json!({"settings":{}}),false),
    ].into_iter().enumerate(){
        let topic=format!("test/output-settings/{index}");let mut call=json!({"command":command,"target":TARGET});call.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
        assert!(broker.publish("DALI/Command/Controller",call.to_string(),&SendOptions::default().response_topic(&topic).user_property("target-generation",context(1).generation().unwrap()).user_property("traceparent",trace)));
        assert!(broker.wait_until(Duration::from_secs(3),|b|!b.received_on(&topic).is_empty()).await,"output settings MQTT command did not complete");
        let replies=broker.received_on(&topic);assert_eq!(replies.len(),1);let value:serde_json::Value=serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(value.get("error").is_none(),success,"strict output settings params or Read-class dispatch failed: {value}");
        if success {assert_eq!(value["value"]["outcome"],if command=="configure_output"{"configured"}else{"current"});assert_eq!(replies[0].user_property("target-generation"),context(1).generation());assert_eq!(replies[0].user_property("traceparent"),Some(trace));}
    }
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), original);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn managed_output_settings_final_physical_epoch_cut_rejects_old_samples() {
    for writing in [false, true] {
        let fixture = Fixture::new();
        let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":0}]}]})).unwrap()).await.unwrap();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        service.output_acceptance_hook =
            Some(Arc::new(|hat| hat.power_cycle_output_samples_for_test()));
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let result = if writing {
            handle
                .configure_output(
                    TARGET,
                    1,
                    Patch {
                        minimum_level: Some(5),
                        ..Default::default()
                    },
                    context(1),
                    limit(),
                )
                .await
        } else {
            handle
                .read_output_settings(TARGET, context(1), limit())
                .await
        };
        if writing {
            let r = result.unwrap();
            assert_eq!(
                r.outcome, "unconfirmed",
                "old physical epoch confirmed an output settings write"
            );
            assert!(r.settings.is_none() && r.readings.is_none());
        } else {
            assert!(
                result.is_err(),
                "old physical epoch accepted an output settings read"
            );
        }
        let native = physical(&emulator.emulator).await;
        assert_eq!(
            native["buses"][0]["outputs"][0]["level"], 0,
            "minimum edit turned on an off fixture"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn managed_output_settings_ready_reply_keeps_original_deadline_and_revision() {
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    for expired in [false, true] {
        let fixture = Fixture::new();
        let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100}]}]})).unwrap()).await.unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let until = Instant::now() + Duration::from_millis(250);
        let mut read = Box::pin(handle.read_output_settings(TARGET, context(1), until));
        poll_fn(|cx| {
            assert!(read.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        wait_for(&handle, |_| handle.permit.available_permits() == 1).await;
        if expired {
            tokio::time::sleep_until(until + Duration::from_millis(10)).await;
        } else {
            handle
                .edit(
                    1,
                    MetadataEdit::Description(Some("new revision".into())),
                    context(1),
                    limit(),
                )
                .await
                .unwrap();
        }
        assert!(
            read.await.is_err(),
            "held settings read renewed its original deadline or revision"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn managed_output_settings_emulator_all_fields_and_crossed_limits_keep_identity() {
    for two in [false, true] {
        let fixture = Fixture::new();
        let original = fs::read(&fixture.paths.current).unwrap();
        let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"repeat":if two {"two_terminals"} else {"one_terminal"},"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100,"groups":1}]}]})).unwrap()).await.unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &emulator.path,
                if two {
                    RepeatPolicy::TwoTerminals
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let initial = handle
            .read_output_settings(TARGET, context(1), limit())
            .await;
        assert!(
            initial.is_ok(),
            "managed output settings did not execute through native owner: {initial:?}"
        );
        let initial = initial.unwrap();
        assert_eq!(initial.outcome, "current");
        assert_eq!(initial.settings.as_ref().unwrap().fade_rate, Some(7));
        for (patch, min, max, level, fade) in [
            (
                Patch {
                    minimum_level: Some(150),
                    maximum_level: Some(200),
                    power_on_level: Some(255),
                    system_failure_level: Some(0),
                    fade_time: Some(15),
                    fade_rate: Some(1),
                },
                150,
                200,
                150,
                0xf1,
            ),
            (
                Patch {
                    minimum_level: Some(220),
                    maximum_level: Some(240),
                    power_on_level: Some(0),
                    system_failure_level: Some(255),
                    fade_time: Some(1),
                    fade_rate: Some(15),
                },
                220,
                240,
                220,
                0x1f,
            ),
            (
                Patch {
                    minimum_level: Some(5),
                    maximum_level: Some(50),
                    power_on_level: Some(127),
                    system_failure_level: Some(128),
                    fade_time: Some(0),
                    fade_rate: Some(7),
                },
                5,
                50,
                50,
                7,
            ),
        ] {
            let actual = handle
                .configure_output(TARGET, 1, patch.clone(), context(1), limit())
                .await
                .unwrap();
            assert_eq!(
                actual.outcome, "configured",
                "native settings lacked matching confirmation"
            );
            assert_eq!(actual.target_generation, context(1).generation().unwrap());
            assert!(!actual.configuration_changed);
            let values = actual.settings.unwrap();
            assert_eq!(
                (values.minimum_level, values.maximum_level, values.fade_raw),
                (Some(min), Some(max), Some(fade))
            );
            let native = physical(&emulator.emulator).await;
            let gear = &native["buses"][0]["outputs"][0];
            assert_eq!(
                gear["level"], level,
                "crossed limits caused unintended native clamping"
            );
            assert_eq!(gear["power_on_level"], patch.power_on_level.unwrap());
            assert_eq!(gear["failure_level"], patch.system_failure_level.unwrap());
            assert_eq!(
                fs::read(&fixture.paths.current).unwrap(),
                original,
                "hardware settings rewrote saved identity/configuration"
            );
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}
