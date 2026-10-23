use super::*;
use crate::emulator::model::Reply;
use std::sync::atomic::Ordering;

fn profile(kind: &str, level: u8) -> crate::emulator::scenario::Scenario {
    serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{
        "id":"lamp","short_address":0,"level":level,"device_type":8,"groups":5,
        "colour":{"active":kind,"auto_activation":true,"xy":[10000,20000],
            "temperature":{"value":250,"coolest":200,"warmest":400,"physical_coolest":153,"physical_warmest":500},
            "rgbwaf":[80,90,100,110]}}]}]})).unwrap()
}
fn original_control() -> Vec<u16> {
    vec![0xa38f, 0xc108, 0x01ed, 0xc108, 0x01e2]
}
fn active(peer: &peer::Peer) -> serde_json::Value {
    let snapshot = peer.model.lock().unwrap().snapshot();
    let gear = &snapshot["buses"][0]["outputs"][0];
    let color = &gear["colour_state"]["actual"];
    let kind = color["active"].as_str().unwrap();
    json!({"kind":kind,"value":color[kind],"control":if kind=="rgbwaf" {gear["colour_state"]["actual_control"].clone()} else {json!(null)}})
}

#[tokio::test]
async fn identification_color_restores_native_xy_and_rgbwaf_with_original_power_and_control() {
    for (kind, pending) in [
        ("xy", "temperature"),
        ("rgbwaf", "temperature"),
        ("rgbwaf", "rgbwaf"),
    ] {
        for level in [0, 73] {
            let fixture = Fixture::new();
            let mut staged = if kind == "rgbwaf" {
                original_control()
            } else {
                vec![]
            };
            if pending == "temperature" {
                staged.extend([0xa35e, 0xc301, 0xc108, 0x01e7]);
            } else {
                // Pending channels also change the mode; restore the original control.
                staged.extend([
                    0xa301, 0xc302, 0xc503, 0xc108, 0x01eb, 0xa340, 0xc108, 0x01ed,
                ]);
            }
            let peer = peer::Peer::profile(profile(kind, level), staged, None);
            let before = active(&peer);
            let broker = FakeBroker::start().await;
            let (service, handle) = fixture.service();
            let running = start(
                service.with_uart(&peer.path, RepeatPolicy::Unverified),
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
            assert_eq!(
                command(&broker, "blink", TARGET, 1).await["value"]["phase"],
                "blinking"
            );
            assert_ne!(
                active(&peer),
                before,
                "test did not activate a different staged color"
            );
            let result = command(&broker, "stop_blink", TARGET, 1).await;
            assert_eq!(
                result["value"]["phase"], "restored",
                "Stop did not confirm color restoration: {result}"
            );
            assert_eq!(
                active(&peer),
                before,
                "Stop failed to restore exact active color and control"
            );
            let native = peer.model.lock().unwrap().snapshot();
            let gear = &native["buses"][0]["outputs"][0];
            assert_eq!(
                gear["level"], level,
                "color restoration lost original power/brightness"
            );
            assert_eq!(gear["groups"], 5);
            assert_eq!(gear["colour"]["auto_activation"], true);
            assert_eq!(
                fixture.disk().revision,
                1,
                "color restoration changed saved configuration"
            );
            assert!(running.stop().await);
            peer.close().await;
        }
    }
}

#[tokio::test]
async fn identification_color_capture_requires_confirmed_type_features_and_active_values_before_pulsing(
) {
    for missing in [0x0199, 0x01f7, 0x01fa] {
        let fixture = Fixture::new();
        let peer = peer::Peer::new(None, true);
        peer.overrides.lock().unwrap().insert(missing, Reply::None);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::Unverified),
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
        let result = command(&broker, "blink", TARGET, 1).await;
        assert!(
            result.get("error").is_some(),
            "incomplete color capture started Blink: {result}"
        );
        assert!(
            !peer
                .frames
                .lock()
                .unwrap()
                .iter()
                .any(|frame| matches!(frame, 0x0105 | 0x0100)),
            "incomplete color capture mutated power"
        );
        assert_eq!(physical_peer_level(&peer), 73);
        assert!(running.stop().await);
        peer.close().await;
    }
}
fn physical_peer_level(peer: &peer::Peer) -> serde_json::Value {
    peer.model.lock().unwrap().snapshot()["buses"][0]["outputs"][0]["level"].clone()
}

#[tokio::test]
async fn identification_color_restore_requires_fresh_actual_control_not_prewrite_capture() {
    let fixture = Fixture::new();
    let mut staged = original_control();
    staged.extend([0xa301, 0xc302, 0xc503, 0xc108, 0x01eb]);
    let peer = peer::Peer::profile(profile("rgbwaf", 73), staged, None);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
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
    assert_eq!(
        command(&broker, "blink", TARGET, 1).await["value"]["phase"],
        "blinking"
    );
    peer.wrong_control.store(true, Ordering::SeqCst);
    let result = command(&broker, "stop_blink", TARGET, 1).await;
    assert!(
        result.get("error").is_some(),
        "incorrect RGBWAF control was reported restored from prewrite evidence"
    );
    assert_eq!(
        handle.snapshot().unwrap().identification.unwrap().phase,
        crate::service::identification::Stage::Interrupted
    );
    assert_eq!(
        active(&peer)["control"],
        128,
        "fixture did not exercise wrong post-write control"
    );
    assert!(running.stop().await);
    peer.close().await;
}

#[tokio::test]
async fn identification_color_paced_capture_and_restore_use_original_eight_second_budgets() {
    let fixture = Fixture::new();
    let peer = peer::Peer::profile(profile("rgbwaf", 73), original_control(), None);
    *peer.pace.lock().unwrap() = Duration::from_millis(50);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
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
    let before = active(&peer);
    let start = Instant::now();
    let reply = command(&broker, "blink", TARGET, 1).await;
    assert_eq!(
        reply["value"]["phase"], "blinking",
        "paced color capture expired: {reply}"
    );
    assert!(
        start.elapsed() > Duration::from_secs(2),
        "paced capture did not exceed the old base-step budget"
    );
    let start = Instant::now();
    let reply = command(&broker, "stop_blink", TARGET, 1).await;
    assert_eq!(
        reply["value"]["phase"], "restored",
        "paced color restoration expired: {reply}"
    );
    assert!(
        start.elapsed() > Duration::from_secs(4),
        "paced restoration did not exceed the old command budget"
    );
    assert_eq!(active(&peer), before);
    assert!(running.stop().await);
    peer.close().await;
}

#[tokio::test]
async fn identification_color_ignored_restore_does_not_report_sent_intent_as_success() {
    let fixture = Fixture::new();
    let peer = peer::Peer::new(None, true);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::Unverified),
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
    assert_eq!(
        command(&broker, "blink", TARGET, 1).await["value"]["phase"],
        "blinking"
    );
    peer.overrides.lock().unwrap().insert(0x01e7, Reply::None);
    let result = command(&broker, "stop_blink", TARGET, 1).await;
    assert!(
        result.get("error").is_some(),
        "ignored color restoration was reported successful from sent intent"
    );
    assert_eq!(
        handle.snapshot().unwrap().identification.unwrap().phase,
        crate::service::identification::Stage::Interrupted
    );
    assert_eq!(
        active(&peer)["value"]["value"],
        350,
        "fixture did not ignore color restoration"
    );
    assert!(running.stop().await);
    peer.close().await;
}
