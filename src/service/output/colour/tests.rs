use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    dali::colour as codec,
    emulator::{pty, session::Control},
    service::{
        tests::{context, deadline, start, wait_for, Fixture},
        Hardware,
    },
};
use std::time::Duration;
mod peer;
use crate::service::{ServiceError, ServiceHandle};
use peer::{Peer, Profile};
const TARGET: &str = "Bus1.Outputs.Light1";

#[test]
fn group_color_and_individual_confirmation_require_fresh_normalized_rgbwaf_mode() {
    let mut sample = Observation {
        dt8: Some(true),
        gear_features: Some(Reply::Value(0)),
        features: Some(Reply::Value(Features::decode(0xc0))),
        assignments: (1..=3)
            .map(|value| Reply::Value(AssignedColour::decode(value)))
            .collect(),
        status: Some(Reply::Value(Status::decode(0x80))),
        control: Some(Reply::Value(0x80)),
        quality: Quality::Current,
        values: [ValueQuery::Red, ValueQuery::Green, ValueQuery::Blue]
            .into_iter()
            .map(|query| (query, Reply::Value(Value::Byte(ByteValue::Known(127)))))
            .collect(),
        ..Observation::default()
    };
    let intent = Intent::rgbwaf([
        Some(codec::Channel::new(127).unwrap()),
        None,
        None,
        None,
        None,
        None,
    ])
    .unwrap();
    assert!(sample.confirms(intent));
    for control in [Some(Reply::Value(0x40)), Some(Reply::NoResponse), None] {
        sample.control = control;
        assert!(
            !sample.confirms(intent),
            "postwrite unnormalized RGBWAF confirms while ColorAvailable=false"
        );
    }
}

#[tokio::test]
async fn mqtt_colour_complete_paced_write_uses_original_eight_second_budget() {
    use crate::config::ColorRepresentation;
    use mqtt_test_broker::SendOptions;
    let fixture = Fixture::new();
    let peer = Peer::start(Profile {
        features: 0xc2,
        status: 0x80,
        assignments: vec![1, 2, 3, 4, 5, 6],
        ..Profile::default()
    });
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+", Duration::from_secs(2))
            .await
    );
    handle
        .edit(
            1,
            crate::service::MetadataEdit::OutputColorRepresentation {
                bus: 0,
                address: 0,
                representation: Some(ColorRepresentation::Rgbwaf),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    peer.profile.lock().unwrap().pace = Duration::from_millis(50);
    let started = tokio::time::Instant::now();
    assert!(broker.publish(
        "DALI/Desired/Controller/Bus1.Outputs.Light1",
        r#"{"PowerState":true,"ColorRed":1}"#,
        &SendOptions::default()
            .user_property("target-generation", context(2).generation().unwrap())
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(12), |b| b
                .received_on("DALI/State/Controller/Bus1.Outputs.Light1")
                .last()
                .is_some_and(|m| {
                    serde_json::from_slice::<serde_json::Value>(&m.payload).is_ok_and(|v| {
                        v["ColorAvailable"] == true
                            && v["ColorRed"] == serde_json::json!(7.0 / 254.0)
                            && v["PowerState"] == true
                            && v["Brightness"] == 1.0
                    })
                }))
            .await,
        "complete paced color write expired or echoed intent instead of actual readback"
    );
    assert!(
        started.elapsed() > Duration::from_secs(6),
        "paced color path did not exercise the six-second budget boundary"
    );
    let frames = peer.commands.lock().unwrap();
    assert!(
        frames.contains(&0x01eb) && frames.contains(&0x0105),
        "paced test did not execute both color and power mutations"
    );
    drop(frames);
    stop(running, peer).await;
}
fn tc_intent() -> codec::Intent {
    codec::Intent::temperature(codec::Temperature::new(300).unwrap())
}
async fn boot(profile: Profile) -> (Fixture, Peer, mqtt_bridge_kit::Running, ServiceHandle) {
    let fixture = Fixture::new();
    let peer = Peer::start(profile);
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    (fixture, peer, running, handle)
}
async fn stop(running: mqtt_bridge_kit::Running, mut peer: Peer) {
    assert!(running.stop().await);
    peer.task.abort();
    let _ = (&mut peer.task).await;
}

#[tokio::test]
async fn colour_service_enumerates_multiple_types_and_rejects_invalid_progression() {
    let (_fixture, peer, running, handle) = boot(Profile {
        device_type: 255,
        next: vec![0, 2, 7, 8],
        ..Profile::default()
    })
    .await;
    let sample = handle
        .read_colour(TARGET, context(1), deadline())
        .await
        .unwrap()
        .colour
        .unwrap();
    assert_eq!(sample.dt8, Some(true), "multiple-type discovery missed DT8");
    assert_eq!(sample.type_prefix, vec![0, 2, 7, 8]);
    let frames = peer.commands.lock().unwrap().clone();
    let start = frames.iter().position(|v| *v == 0x0199).unwrap();
    assert_eq!(
        &frames[start + 1..start + 5],
        &[0x01a7; 4],
        "next-type enumeration was interleaved"
    );
    for next in [
        vec![1, 1, 8],
        vec![0, 9],
        vec![255],
        vec![0, 1, 2, 3, 4, 5, 6, 7, 254],
    ] {
        peer.profile.lock().unwrap().next = next;
        let before = peer.commands.lock().unwrap().len();
        let sample = handle
            .read_colour(TARGET, context(1), deadline())
            .await
            .unwrap()
            .colour
            .unwrap();
        assert_ne!(
            sample.dt8,
            Some(true),
            "invalid type enumeration guessed DT8"
        );
        let after = peer.commands.lock().unwrap().len();
        assert!(
            after - before <= 12,
            "type enumeration exceeded its bounded prefix"
        );
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_preflight_rejects_unsupported_activation_mode_and_assignments_without_writes(
) {
    let (_fixture, peer, running, handle) = boot(Profile::default()).await;
    let rgb = codec::Intent::rgbwaf([
        Some(codec::Channel::new(50).unwrap()),
        None,
        None,
        None,
        None,
        None,
    ])
    .unwrap();
    for (profile, intent) in [
        (
            Profile {
                device_type: 0,
                ..Profile::default()
            },
            tc_intent(),
        ),
        (
            Profile {
                features: 1,
                ..Profile::default()
            },
            tc_intent(),
        ),
        (
            Profile {
                gear_features: None,
                ..Profile::default()
            },
            tc_intent(),
        ),
        (
            Profile {
                features: 0xe2,
                ..Profile::default()
            },
            tc_intent(),
        ),
        (
            Profile {
                features: 0x60,
                control: 63,
                status: 0x80,
                ..Profile::default()
            },
            rgb,
        ),
        (
            Profile {
                features: 0x60,
                status: 0x80,
                assignments: vec![2, 3, 4],
                ..Profile::default()
            },
            rgb,
        ),
        (
            Profile {
                features: 0x60,
                status: 0x80,
                assignments: vec![0, 42, 255],
                ..Profile::default()
            },
            rgb,
        ),
    ] {
        *peer.profile.lock().unwrap() = profile;
        let result = handle
            .apply_colour(TARGET, intent, context(1), deadline())
            .await;
        assert!(
            matches!(result, Err(ServiceError::Rejected(_))),
            "invalid colour preflight reached mutation: {result:?}"
        );
        assert!(
            peer.setters().is_empty(),
            "rejected colour operation sent setters or ACTIVATE"
        );
        assert_eq!(
            handle.snapshot().unwrap().outputs[TARGET].quality,
            super::super::Quality::Current
        );
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_incidental_failures_preserve_successful_base_writes() {
    let (_fixture, peer, running, handle) = boot(Profile {
        device_type: 0,
        ..Profile::default()
    })
    .await;
    let initial = handle
        .read_colour(TARGET, context(1), deadline())
        .await
        .unwrap();
    assert_eq!(initial.colour.unwrap().quality, Quality::Unsupported);
    for (device_type, high_failure) in [(0, None), (8, None), (8, Some("N\n")), (8, Some("X03\n"))]
    {
        *peer.profile.lock().unwrap() = Profile {
            device_type,
            high_failure,
            ..Profile::default()
        };
        let result = handle
            .apply_output(
                TARGET,
                crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .expect("incidental colour failure rejected an observed base write");
        assert_eq!(result.power(), Some(false));
        let colour = result
            .colour
            .as_ref()
            .expect("base operation omitted fresh colour evidence");
        if high_failure.is_some() {
            assert_eq!(colour.quality, Quality::Partial);
        }
        assert_eq!(handle.snapshot().unwrap().outputs[TARGET], result);
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_ignored_activate_cannot_confirm_another_valid_active_kind() {
    let (_fixture, peer, running, handle) = boot(Profile {
        features: 3,
        ..Profile::default()
    })
    .await;
    let result = handle
        .apply_colour(
            TARGET,
            codec::Intent::xy(
                codec::Coordinate::new(1234).unwrap(),
                codec::Coordinate::new(2345).unwrap(),
            ),
            context(1),
            deadline(),
        )
        .await;
    assert!(
        matches!(result, Err(ServiceError::Uncertain(_))),
        "different active kind falsely confirmed colour write: {result:?}"
    );
    let snapshot = handle.snapshot().unwrap();
    let sample = snapshot.outputs[TARGET].colour.as_ref().unwrap();
    assert_eq!(sample.active(), Some(codec::ColourType::Temperature));
    assert_eq!(
        sample.value(ValueQuery::Temperature),
        Some(Value::Word(WordValue::Known(250)))
    );
    assert_eq!(peer.setters().last(), Some(&0x01e2));
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_failed_word_bytes_remain_partial_and_stop_before_low_on_failed_high() {
    let (_fixture, peer, running, handle) = boot(Profile::default()).await;
    for (high_failure, low_failure, expected) in [
        (Some("N\n"), None, Reply::NoResponse),
        (Some("X03\n"), None, Reply::ReceiveCollision(Some(3))),
        (None, Some("N\n"), Reply::NoResponse),
    ] {
        *peer.profile.lock().unwrap() = Profile {
            high_failure,
            low_failure,
            ..Profile::default()
        };
        let before = peer.commands.lock().unwrap().len();
        let sample = handle
            .read_colour(TARGET, context(1), deadline())
            .await
            .unwrap()
            .colour
            .unwrap();
        assert_eq!(sample.quality, Quality::Partial);
        assert_eq!(
            sample.values[&ValueQuery::Temperature],
            expected,
            "failed colour bytes fabricated a word"
        );
        if high_failure.is_some() {
            assert!(
                !peer.commands.lock().unwrap()[before..].contains(&0x0198),
                "failed word high still queried low register"
            );
        }
        assert_eq!(sample.value(ValueQuery::Temperature), None);
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_owns_cancelled_write_while_native_inputs_continue() {
    let fixture = Fixture::new();
    fixture.add_button();
    let mut peer = Peer::start(Profile {
        hold: Some(0x01e7),
        ..Profile::default()
    });
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| {
        s.input_listeners
            .contains("Bus1.Inputs.Panel.Instances.Button")
    })
    .await;
    let mut events = handle.subscribe_inputs();
    let client = handle.clone();
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    let request = tokio::spawn(async move {
        client
            .apply_colour_with_output(
                TARGET,
                tc_intent(),
                crate::dali::gear::Intent::new(Some(true), None).unwrap(),
                context(1).with_traceparent(Some(trace)),
                deadline(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), peer.held.notified())
        .await
        .unwrap();
    request.abort();
    let _ = request.await;
    assert!(
        matches!(
            handle.read_output(TARGET, context(1), deadline()).await,
            Err(ServiceError::Busy)
        ),
        "cancelled colour write released physical ownership early"
    );
    assert!(
        matches!(
            handle
                .edit(
                    1,
                    crate::service::MetadataEdit::RenameOutput {
                        bus: 0,
                        address: 0,
                        name: "Kitchen".into()
                    },
                    context(1),
                    deadline()
                )
                .await,
            Err(ServiceError::Busy)
        ),
        "rename crossed unfinished colour transaction"
    );
    peer.send(b"L008001\n").await;
    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.target, "Bus1.Inputs.Panel.Instances.Button");
    assert_eq!(
        event.context.traceparent(),
        None,
        "native input inherited colour command trace"
    );
    peer.send(b"N\n").await;
    wait_for(&handle, |s| {
        s.outputs.get(TARGET).is_some_and(|o| {
            o.colour
                .as_ref()
                .is_some_and(|c| c.quality == Quality::Current)
        })
    })
    .await;
    let sample = handle.snapshot().unwrap().outputs[TARGET].clone();
    assert_eq!(
        sample.context.traceparent(),
        Some(trace),
        "late owned colour completion lost original cause"
    );
    assert_eq!(sample.context.generation(), context(1).generation());
    handle
        .edit(
            1,
            crate::service::MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(!handle.snapshot().unwrap().outputs.contains_key(TARGET));
    assert!(
        handle
            .apply_colour(
                "Bus1.Outputs.Kitchen",
                tc_intent(),
                sample.context,
                deadline()
            )
            .await
            .is_err(),
        "renamed target accepted retired colour command"
    );
    assert!(running.stop().await);
    peer.task.abort();
    let _ = (&mut peer.task).await;
}

#[tokio::test]
async fn colour_service_final_epoch_invalidation_marks_colour_stale() {
    let fixture = Fixture::new();
    let peer = Peer::start(Profile::default());
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.output_acceptance_hook = Some(std::sync::Arc::new(|hat| {
        hat.power_cycle_output_samples_for_test()
    }));
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(handle
        .read_colour(TARGET, context(1), deadline())
        .await
        .is_err());
    let sample = handle.snapshot().unwrap().outputs[TARGET]
        .colour
        .clone()
        .unwrap();
    assert_eq!(
        sample.quality,
        Quality::Stale,
        "final UART invalidation left colour current"
    );
    assert_eq!(sample.active(), None);
    assert_eq!(sample.value(ValueQuery::Temperature), None);
    stop(running, peer).await;
}

#[derive(Clone, Copy)]
enum PacedOperation {
    Write,
    TargetRead,
    AllRead,
    ConfiguredRead,
}
async fn paced_base(operation: PacedOperation) {
    use mqtt_test_broker::SendOptions;
    let fixture = Fixture::new();
    let peer = Peer::start(Profile {
        features: 0xc2,
        status: 0x80,
        assignments: vec![1, 2, 3, 4, 5, 6],
        ..Profile::default()
    });
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(peer.path.clone(), RepeatPolicy::Unverified),
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
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+", Duration::from_secs(2))
            .await
    );
    if matches!(operation, PacedOperation::ConfiguredRead) {
        handle
            .edit(
                1,
                crate::service::MetadataEdit::OutputColorRepresentation {
                    bus: 0,
                    address: 0,
                    representation: Some(crate::config::ColorRepresentation::Rgbwaf),
                },
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert!(
            handle.snapshot().unwrap().outputs.get(TARGET).is_none(),
            "configured-read test already had color evidence"
        );
    } else {
        let sample = handle
            .read_colour(TARGET, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(sample.colour.unwrap().quality, Quality::Current);
    }
    // A realistic bounded response pace, not a claim about every HAT/fixture.
    // This profile needs 73 exchanges, so its complete sample exceeds 2 s.
    peer.profile.lock().unwrap().pace = Duration::from_millis(40);
    let started = tokio::time::Instant::now();
    match operation {
        PacedOperation::Write => {
            assert!(broker.send_with(
                "DALI/Desired/Controller/Bus1.Outputs.Light1",
                r#"{"PowerState":false}"#,
                &SendOptions::default()
                    .user_property("target-generation", context(1).generation().unwrap())
            ));
            assert!(
                broker
                    .wait_until(Duration::from_secs(6), |_| handle.snapshot().is_some_and(
                        |s| s
                            .outputs
                            .get(TARGET)
                            .is_some_and(|o| o.power() == Some(false)
                                && o.colour
                                    .as_ref()
                                    .is_some_and(|c| c.quality == Quality::Current))
                    ))
                    .await,
                "paced known-colour base write expired before full readback"
            );
            assert!(
                broker.received_on("DALI/Error/Controller").is_empty(),
                "paced known-colour base write reported an error"
            );
        }
        PacedOperation::TargetRead | PacedOperation::AllRead | PacedOperation::ConfiguredRead => {
            let payload = match operation {
                PacedOperation::TargetRead | PacedOperation::ConfiguredRead => {
                    r#"{"command":"refresh","target":"Bus1.Outputs.Light1"}"#
                }
                _ => r#"{"command":"refresh"}"#,
            };
            assert!(broker.send_with(
                "DALI/Command/Controller",
                payload,
                &SendOptions::default().response_topic("test/paced-colour")
            ));
            assert!(
                broker
                    .wait_until(Duration::from_secs(6), |b| !b
                        .received_on("test/paced-colour")
                        .is_empty())
                    .await
            );
            let reply: serde_json::Value =
                serde_json::from_slice(&broker.received_on("test/paced-colour")[0].payload)
                    .unwrap();
            assert!(
                reply.get("error").is_none(),
                "paced known-colour base read expired: {reply}"
            );
            assert_eq!(
                handle.snapshot().unwrap().outputs[TARGET]
                    .colour
                    .as_ref()
                    .unwrap()
                    .quality,
                Quality::Current
            );
        }
    }
    assert!(
        started.elapsed() > Duration::from_secs(2),
        "peer did not actually exercise the longer readback path"
    );
    stop(running, peer).await;
}
#[tokio::test]
async fn colour_service_paced_base_write_keeps_actual_colour() {
    paced_base(PacedOperation::Write).await;
}
#[tokio::test]
async fn colour_service_paced_target_read_keeps_actual_colour() {
    paced_base(PacedOperation::TargetRead).await;
}
#[tokio::test]
async fn colour_service_paced_all_read_keeps_actual_colour() {
    paced_base(PacedOperation::AllRead).await;
}

#[tokio::test]
async fn mqtt_colour_first_paced_refresh_uses_configured_read_budget() {
    paced_base(PacedOperation::ConfiguredRead).await;
}

#[tokio::test]
async fn colour_service_known_automatic_activation_accepts_a_colour_write() {
    let (_fixture, peer, running, handle) = boot(Profile {
        gear_features: Some(1),
        ..Profile::default()
    })
    .await;
    let actual = handle
        .apply_colour(TARGET, tc_intent(), context(1), deadline())
        .await
        .expect("known automatic activation was rejected");
    assert_eq!(actual.colour.unwrap().quality, Quality::Current);
    assert_eq!(peer.setters(), vec![0x01e7, 0x01e2]);
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_combined_operation_applies_colour_and_master_independently() {
    for automatic in [false, true] {
        let fixture = Fixture::new();
        let broker = mqtt_test_broker::FakeBroker::start().await;
        let mut scenario: crate::emulator::scenario::Scenario =
            serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json"))
                .unwrap();
        scenario.buses[0].outputs[0].short_address = Some(0);
        scenario.buses[0].outputs[0]
            .colour
            .as_mut()
            .unwrap()
            .auto_activation = automatic;
        let emulator = pty::spawn(scenario).await.unwrap();
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(emulator.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        for (base, colour, query, expected, level) in [
            (
                crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                tc_intent(),
                ValueQuery::Temperature,
                Value::Word(WordValue::Known(300)),
                0,
            ),
            (
                crate::dali::gear::Intent::new(None, Some(0.5)).unwrap(),
                codec::Intent::xy(
                    codec::Coordinate::new(1000).unwrap(),
                    codec::Coordinate::new(2000).unwrap(),
                ),
                ValueQuery::X,
                Value::Word(WordValue::Known(1000)),
                127,
            ),
            (
                crate::dali::gear::Intent::new(None, Some(0.0)).unwrap(),
                codec::Intent::rgbwaf([
                    Some(codec::Channel::new(80).unwrap()),
                    None,
                    None,
                    None,
                    None,
                    None,
                ])
                .unwrap(),
                ValueQuery::Red,
                Value::Byte(ByteValue::Known(80)),
                0,
            ),
        ] {
            let actual = handle
                .apply_colour_with_output(TARGET, colour, base, context(1), deadline())
                .await
                .unwrap();
            assert_eq!(
                actual.brightness(),
                Some(f64::from(level) / 254.0),
                "combined request did not apply the independent master level"
            );
            assert_eq!(
                actual.colour.as_ref().unwrap().value(query),
                Some(expected),
                "combined request did not read actual requested colour"
            );
            let physical = emulator
                .emulator
                .control(Control::Snapshot, Duration::from_secs(1))
                .await
                .unwrap();
            assert_eq!(physical["buses"][0]["outputs"][0]["level"], level);
        }
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn colour_service_combined_orders_commands_without_claiming_completed_fades() {
    let (_fixture, peer, running, handle) = boot(Profile {
        gear_features: Some(1),
        ..Profile::default()
    })
    .await;
    for (power, brightness, base_frame, off_first) in [
        (Some(false), None, 0x0100, true),
        (None, Some(0.0), 0x0000, true),
        (Some(true), None, 0x0105, false),
        (None, Some(64.0 / 254.0), 0x0040, false),
    ] {
        let before = peer.commands.lock().unwrap().len();
        handle
            .apply_colour_with_output(
                TARGET,
                tc_intent(),
                crate::dali::gear::Intent::new(power, brightness).unwrap(),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        let recorded = peer.commands.lock().unwrap().clone();
        let frames = &recorded[before..];
        let base = frames.iter().position(|f| *f == base_frame).unwrap();
        let setter = frames.iter().position(|f| *f == 0x01e7).unwrap();
        let activate = frames.iter().position(|f| *f == 0x01e2).unwrap();
        assert!(setter < activate);
        assert!(
            if off_first {
                base < setter
            } else {
                base > activate
            },
            "combined power/colour frame order is incorrect: {frames:04x?}"
        );
        assert_eq!(
            frames.iter().filter(|f| **f == base_frame).count(),
            1,
            "combined operation repeated its base mutation"
        );
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_combined_invalid_preflight_sends_neither_power_nor_colour() {
    let (_fixture, peer, running, handle) = boot(Profile::default()).await;
    for profile in [
        Profile {
            device_type: 0,
            ..Profile::default()
        },
        Profile {
            features: 1,
            ..Profile::default()
        },
        Profile {
            gear_features: None,
            ..Profile::default()
        },
    ] {
        *peer.profile.lock().unwrap() = profile;
        let result = handle
            .apply_colour_with_output(
                TARGET,
                tc_intent(),
                crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                context(1),
                deadline(),
            )
            .await;
        assert!(
            matches!(result, Err(ServiceError::Rejected(_))),
            "combined unsupported preflight was not rejected: {result:?}"
        );
        assert!(
            !peer.commands.lock().unwrap().contains(&0x0100),
            "combined preflight mutated master power before rejecting colour"
        );
        assert!(peer.setters().is_empty());
        assert_eq!(
            handle.snapshot().unwrap().outputs[TARGET].brightness(),
            Some(100.0 / 254.0)
        );
    }
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_combined_incomplete_colour_returns_actual_base_and_uncertain() {
    let (_fixture, peer, running, handle) = boot(Profile {
        features: 3,
        ..Profile::default()
    })
    .await;
    let colour = codec::Intent::xy(
        codec::Coordinate::new(1000).unwrap(),
        codec::Coordinate::new(2000).unwrap(),
    );
    let result = handle
        .apply_colour_with_output(
            TARGET,
            colour,
            crate::dali::gear::Intent::new(Some(false), None).unwrap(),
            context(1),
            deadline(),
        )
        .await;
    assert!(
        matches!(result, Err(ServiceError::Uncertain(_))),
        "complete base write concealed unconfirmed combined colour: {result:?}"
    );
    let actual = handle.snapshot().unwrap().outputs[TARGET].clone();
    assert_eq!(actual.power(), Some(false));
    assert_eq!(
        actual.colour.unwrap().active(),
        Some(codec::ColourType::Temperature)
    );
    stop(running, peer).await;
}

#[tokio::test]
async fn colour_service_reads_actual_tc_xy_rgb_and_keeps_master_independent() {
    let fixture = Fixture::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let mut scenario: crate::emulator::scenario::Scenario =
        serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json")).unwrap();
    scenario.buses[0].outputs[0].short_address = Some(0);
    let emulator = pty::spawn(scenario).await.unwrap();
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let target = "Bus1.Outputs.Light1";
    let read = handle
        .read_colour(target, context(1), deadline())
        .await
        .unwrap();
    let colour = read
        .colour
        .expect("colour service omitted actual DT8 evidence");
    assert_eq!(colour.quality, Quality::Current);
    assert_eq!(
        colour.values[&ValueQuery::Temperature],
        Reply::Value(Value::Word(WordValue::Known(250)))
    );
    let changed = handle
        .apply_colour(
            target,
            codec::Intent::temperature(codec::Temperature::new(450).unwrap()),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(changed.brightness(), Some(100.0 / 254.0));
    assert_eq!(
        changed.colour.unwrap().values[&ValueQuery::Temperature],
        Reply::Value(Value::Word(WordValue::Known(400))),
        "colour write echoed intent instead of actual clamp"
    );
    let changed = handle
        .apply_colour(
            target,
            codec::Intent::xy(
                codec::Coordinate::new(12345).unwrap(),
                codec::Coordinate::new(23456).unwrap(),
            ),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        changed.colour.unwrap().values[&ValueQuery::X],
        Reply::Value(Value::Word(WordValue::Known(12345)))
    );
    let changed = handle
        .apply_colour(
            target,
            codec::Intent::rgbwaf([
                Some(codec::Channel::new(101).unwrap()),
                Some(codec::Channel::new(102).unwrap()),
                Some(codec::Channel::new(103).unwrap()),
                None,
                None,
                None,
            ])
            .unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        changed.colour.unwrap().values[&ValueQuery::White],
        Reply::Value(Value::Byte(ByteValue::Known(4))),
        "RGB cleared masked WAF"
    );
    let changed = handle
        .apply_output(
            target,
            crate::dali::gear::Intent::new(Some(false), None).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(changed.power(), Some(false));
    assert_eq!(
        changed.colour.unwrap().values[&ValueQuery::Red],
        Reply::Value(Value::Byte(ByteValue::Known(101))),
        "base operation lost freshly read colour"
    );
    let refreshed = handle
        .read_output(
            target,
            crate::service::RequestContext::default(),
            deadline(),
        )
        .await
        .unwrap();
    assert!(
        refreshed.colour.is_some(),
        "tokenless base refresh forgot known colour target"
    );
    assert_eq!(refreshed.context.traceparent(), None);
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["rgbwaf"],
        serde_json::json!([101, 102, 103, 4, 5, 6])
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
