use super::groups::call;
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

const OUTPUT: &str = "Bus1.Outputs.Light1";
const SCENE: &str = "Bus1.Scenes.Evening";
mod peer;

#[tokio::test]
async fn scene_operations_inflight_and_timed_out_recall_cannot_leave_actual_values_current() {
    let peer = peer::Peer::new(peer::Profile {
        hold_recall: true,
        ..peer::Profile::default()
    });
    let fixture = Fixture::new();
    save_named_scene(&fixture);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    handle
        .read_output(OUTPUT, context(1), Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let h = handle.clone();
    let task = tokio::spawn(async move {
        h.recall_scene(
            SCENE,
            None,
            2,
            context(2),
            Instant::now() + Duration::from_secs(2),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    assert_eq!(
        handle.snapshot().unwrap().outputs[OUTPUT].quality,
        output::Quality::Stale,
        "in-flight recall left possibly changed actual values current"
    );
    assert!(matches!(
        task.await.unwrap(),
        Err(ServiceError::Uncertain(_))
    ));
    assert_eq!(
        handle.snapshot().unwrap().outputs[OUTPUT].quality,
        output::Quality::Stale,
        "caller timeout exposed current values before physical cleanup"
    );
    peer.release.notify_one();
    let guard = tokio::time::timeout(Duration::from_secs(3), handle.permit.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(guard);
    assert!(running.stop().await);
}

fn save_named_scene(fixture: &Fixture) {
    let model = Model::new(fixture.disk())
        .unwrap()
        .edit(
            1,
            &[Edit::AddScene {
                bus: 0,
                scene: crate::config::Scene {
                    slot: 0,
                    name: "Evening".into(),
                    description: None,
                    contents: Vec::new(),
                },
            }],
        )
        .unwrap();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(model.inventory()).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn scene_operations_unknown_capability_does_not_turn_masked_level_into_ignored_colour() {
    let peer = peer::Peer::new(peer::Profile {
        device_type: "N\n",
        ..peer::Profile::default()
    });
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let report = handle
        .read_scene(
            OUTPUT,
            crate::dali::scenes::Slot::new(0).unwrap(),
            context(1),
            Instant::now() + Duration::from_secs(8),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(
        value["stored"]["dt8"],
        serde_json::Value::Null,
        "unknown DT8 evidence was changed into known absence"
    );
    assert_eq!(
        value["stored"]["quality"], "partial",
        "unknown capability was presented as an ignored scene"
    );
    assert_eq!(value["stored"]["color_type"], serde_json::Value::Null);
    assert!(running.stop().await);
}

#[tokio::test]
async fn scene_operations_final_physical_stamp_rejects_stale_recall_and_marks_stored_read_stale() {
    use std::sync::atomic::AtomicBool;
    for recall in [false, true] {
        let peer = peer::Peer::new(peer::Profile::default());
        let fixture = Fixture::new();
        save_named_scene(&fixture);
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let armed = Arc::new(AtomicBool::new(false));
        let trigger = armed.clone();
        service.output_acceptance_hook = Some(Arc::new(move |hat| {
            if trigger.swap(false, Ordering::SeqCst) {
                hat.power_cycle_output_samples_for_test();
            }
        }));
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        armed.store(true, Ordering::SeqCst);
        if recall {
            assert!(
                matches!(
                    handle
                        .recall_scene(
                            SCENE,
                            None,
                            2,
                            context(2),
                            Instant::now() + Duration::from_secs(2)
                        )
                        .await,
                    Err(ServiceError::Uncertain(_))
                ),
                "recall claimed sent after its physical stamp changed"
            );
        } else {
            let report = handle
                .read_scene(
                    OUTPUT,
                    crate::dali::scenes::Slot::new(0).unwrap(),
                    context(1),
                    Instant::now() + Duration::from_secs(8),
                )
                .await
                .unwrap();
            let value = serde_json::to_value(report).unwrap();
            assert_eq!(
                value["stored"]["quality"], "stale",
                "stored scene evidence was accepted after physical stamp changed"
            );
            assert_eq!(value["stored"]["stale"], true);
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_operations_failed_and_canceled_recalls_invalidate_cached_actual_values() {
    for (index, terminal, cancel) in [
        (0, "X\n", false),
        (1, "Z\n", false),
        (2, "J00\n", false),
        (3, "N\n", true),
    ] {
        let peer = peer::Peer::new(peer::Profile {
            recall: terminal,
            hold_recall: cancel,
            ..peer::Profile::default()
        });
        let fixture = Fixture::new();
        save_named_scene(&fixture);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        handle
            .read_output(OUTPUT, context(1), Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(
            handle.snapshot().unwrap().outputs[OUTPUT].quality,
            output::Quality::Current
        );
        if cancel {
            let h = handle.clone();
            let task = tokio::spawn(async move {
                h.recall_scene(
                    SCENE,
                    None,
                    2,
                    context(2),
                    Instant::now() + Duration::from_secs(2),
                )
                .await
            });
            tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
                .await
                .unwrap();
            assert!(
                matches!(
                    handle
                        .read_output(OUTPUT, context(1), Instant::now() + Duration::from_secs(1))
                        .await,
                    Err(ServiceError::Busy)
                ),
                "scene recall released physical ownership while awaiting its terminal"
            );
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            peer.release.notify_one();
        } else {
            let result = handle
                .recall_scene(
                    SCENE,
                    None,
                    2,
                    context(2),
                    Instant::now() + Duration::from_secs(2),
                )
                .await;
            assert!(
                matches!(result, Err(ServiceError::Uncertain(_))),
                "nonterminal recall evidence was accepted in case {index}"
            );
            assert_eq!(
                handle.snapshot().unwrap().outputs[OUTPUT].quality,
                output::Quality::Stale,
                "failed recall left cached actual values current"
            );
        }
        wait_for(&handle, |s| {
            s.outputs
                .get(OUTPUT)
                .is_some_and(|v| v.quality == output::Quality::Stale)
        })
        .await;
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_operations_masked_type_and_unsupported_type_are_not_inferred_from_actual_colour() {
    for (kind, quality) in [
        (255, "complete"),
        (0x40, "unsupported"),
        (0x31, "unsupported"),
    ] {
        let peer = peer::Peer::new(peer::Profile {
            kind,
            ..peer::Profile::default()
        });
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .read_scene(
                OUTPUT,
                crate::dali::scenes::Slot::new(0).unwrap(),
                context(1),
                Instant::now() + Duration::from_secs(8),
            )
            .await
            .unwrap();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(
            value["stored"]["quality"], quality,
            "masked or unsupported stored type was confused with actual colour"
        );
        assert_eq!(
            value["stored"]["color_type"],
            json!({"kind":if kind==255{"mask"}else{"value"},"raw":kind})
        );
        assert_eq!(
            value["stored"]["values"],
            json!({}),
            "actual colour contaminated stored report"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_operations_literal_stored_colours_preserve_masks_words_and_transport_failures() {
    use peer::{Peer, Profile};
    use std::collections::BTreeMap;
    let mut cases = Vec::new();
    for raw in [350, 65534, 65535] {
        cases.push((
            Profile {
                values: BTreeMap::from([(226, raw)]),
                ..Profile::default()
            },
            "complete",
            "temperature",
            json!({"kind":if raw==65535{"mask"}else{"value"},"raw":raw}),
        ));
    }
    cases.push((
        Profile {
            kind: 0x10,
            values: BTreeMap::from([(224, 65534), (225, 65535)]),
            ..Profile::default()
        },
        "complete",
        "x",
        json!({"kind":"value","raw":65534}),
    ));
    cases.push((
        Profile {
            kind: 0x80,
            values: BTreeMap::from([
                (233, 0),
                (234, 254),
                (235, 255),
                (236, 7),
                (237, 8),
                (238, 9),
                (239, 128),
            ]),
            ..Profile::default()
        },
        "complete",
        "blue",
        json!({"kind":"mask","raw":255}),
    ));
    for (high_failure, low_failure, kind) in [
        (Some((226, "N\n")), None, "no_response"),
        (Some((226, "X\n")), None, "receive_collision"),
        (None, Some((226, "N\n")), "no_response"),
        (None, Some((226, "Z\n")), "transmit_collision"),
    ] {
        let mut expected = json!({"kind":kind});
        if kind.ends_with("collision") {
            expected["code"] = serde_json::Value::Null;
        }
        cases.push((
            Profile {
                high_failure,
                low_failure,
                ..Profile::default()
            },
            "partial",
            "temperature",
            expected,
        ));
    }
    for (index, (profile, quality, field, expected)) in cases.into_iter().enumerate() {
        let stop_at_high = profile.high_failure.is_some();
        let expected_values = profile.values.clone();
        let peer = Peer::new(profile);
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .read_scene(
                OUTPUT,
                crate::dali::scenes::Slot::new(15).unwrap(),
                context(1),
                Instant::now() + Duration::from_secs(8),
            )
            .await
            .unwrap();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(
            value["stored"]["values"][field], expected,
            "stored scene case {index} decoded wrong evidence"
        );
        assert_eq!(value["stored"]["quality"], quality);
        if quality == "complete" {
            assert_eq!(
                value["stored"]["values"].as_object().unwrap().len(),
                expected_values.len()
            );
            for (selector, raw) in expected_values {
                let name = match selector {
                    224 => "x",
                    225 => "y",
                    226 => "temperature",
                    233 => "red",
                    234 => "green",
                    235 => "blue",
                    236 => "white",
                    237 => "amber",
                    238 => "free",
                    239 => "control",
                    _ => panic!("unknown literal selector"),
                };
                let masked = if selector < 233 {
                    raw == 65535
                } else {
                    raw == 255
                };
                assert_eq!(
                    value["stored"]["values"][name],
                    json!({"kind":if masked{"mask"}else{"value"},"raw":raw}),
                    "stored scene attribute {name} was omitted or decoded incorrectly"
                );
            }
        }
        assert_eq!(value["stored"]["level"], json!({"kind":"mask","raw":255}));
        assert!(
            handle.snapshot().unwrap().outputs.is_empty(),
            "stored colour was published as actual State"
        );
        let frames = peer.frames.lock().unwrap().clone();
        let first_scene = frames.iter().position(|v| *v == 0x01bf).unwrap();
        assert!(
            !frames[first_scene..].contains(&0x01a0),
            "actual-level read overwrote stored scene report"
        );
        if stop_at_high {
            let selected = frames.iter().rposition(|v| *v == 0xa3e2).unwrap();
            assert!(
                !frames[selected..].contains(&0x0198),
                "stored word reader continued after failed high byte"
            );
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_operations_mqtt_reads_off_and_mask_without_activation_then_recalls_group_and_bus() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Other");
    let model = Model::new(fixture.disk())
        .unwrap()
        .edit(
            1,
            &[Edit::AddScene {
                bus: 0,
                scene: crate::config::Scene {
                    slot: 0,
                    name: "Evening".into(),
                    description: None,
                    contents: Vec::new(),
                },
            }],
        )
        .unwrap();
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(model.inventory()).unwrap(),
    )
    .unwrap();
    let mut first = vec![255; 16];
    first[0] = 0;
    let mut second = vec![255; 16];
    second[0] = 150;
    let emulator = pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{
        "number":0,"outputs":[
            {"id":"first","short_address":0,"level":100,"groups":1,"scenes":first},
            {"id":"second","short_address":1,"level":80,"groups":2,"scenes":second}
        ]}]}))
        .unwrap(),
    )
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
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    for (slot, kind, raw) in [(0, "value", 0), (1, "mask", 255)] {
        let result = call(
            &broker,
            json!({"command":"read_scene","target":OUTPUT,"slot":slot}),
            1,
            &format!("test/scene-operations/read-{slot}"),
        )
        .await;
        assert_eq!(
            result["value"]["stored"]["level"],
            json!({"kind":kind,"raw":raw}),
            "stored scene read did not distinguish Off from MASK: {result}"
        );
        assert_eq!(result["value"]["stored"]["dt8"], false);
        assert_eq!(result["value"]["stored"]["quality"], "complete");
    }
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":"read_scene","target":OUTPUT,"slot":0}).to_string(),
        &SendOptions::default().response_topic("test/scene-operations/tokenless-read")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/scene-operations/tokenless-read")
                .is_empty())
            .await
    );
    let tokenless: serde_json::Value = serde_json::from_slice(
        &broker.received_on("test/scene-operations/tokenless-read")[0].payload,
    )
    .unwrap();
    assert_eq!(
        tokenless["value"]["stored"]["level"]["raw"], 0,
        "read_scene was classified as a mutation requiring a token"
    );
    let edited = call(
        &broker,
        json!({"command":"describe","description":"Unrelated","expected_revision":2}),
        2,
        "test/scene-operations/unrelated",
    )
    .await;
    assert_eq!(edited["value"]["revision"], 3);
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 100,
        "stored read activated the scene"
    );
    for target in [OUTPUT, "Bus1.Outputs.Other"] {
        handle
            .read_output(target, context(1), Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
    }
    let result = call(
        &broker,
        json!({"command":"recall_scene","target":SCENE,"group":0,"expected_revision":3}),
        2,
        "test/scene-operations/group",
    )
    .await;
    assert_eq!(
        result["value"]["outcome"], "sent",
        "native group recall failed: {result}"
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 0);
    assert_eq!(
        physical["buses"][0]["outputs"][1]["level"], 80,
        "group recall reached an unselected fixture"
    );
    for target in [OUTPUT, "Bus1.Outputs.Other"] {
        assert_eq!(
            handle.snapshot().unwrap().outputs[target].quality,
            output::Quality::Stale,
            "recall left cached fixture values current"
        );
    }
    let result = call(
        &broker,
        json!({"command":"recall_scene","target":SCENE,"expected_revision":3}),
        2,
        "test/scene-operations/bus",
    )
    .await;
    assert_eq!(result["value"]["outcome"], "sent");
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][1]["level"], 150,
        "bus recall did not reach every fixture"
    );
    assert_eq!(
        fixture.disk().revision,
        3,
        "recall persisted an active scene"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
