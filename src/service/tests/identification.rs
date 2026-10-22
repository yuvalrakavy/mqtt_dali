use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    emulator::{pty, session::Control},
};

const TARGET: &str = "Bus1.Outputs.Light1";
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";
mod peer;
fn until() -> Instant {
    Instant::now() + Duration::from_secs(4)
}
async fn command(
    broker: &FakeBroker,
    command: &str,
    target: &str,
    revision: u64,
) -> serde_json::Value {
    let topic = format!("test/identify/{command}/{:032x}", rand::random::<u128>());
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":command,"target":target}).to_string(),
        &SendOptions::default()
            .response_topic(&topic)
            .user_property("target-generation", context(revision).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b
                .received_on(&topic)
                .is_empty())
            .await,
        "identification command did not complete"
    );
    let replies = broker.received_on(&topic);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].user_property("traceparent"), Some(TRACE));
    assert_eq!(
        replies[0].user_property("target-generation"),
        context(revision).generation()
    );
    serde_json::from_slice(&replies[0].payload).unwrap()
}
async fn physical(emulator: &crate::emulator::session::Emulator) -> serde_json::Value {
    emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap()["buses"]
        .clone()
}
fn scenario(level: u8) -> crate::emulator::scenario::Scenario {
    serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"lamp","short_address":0,"level":level}]}]}))
    .unwrap()
}

#[tokio::test]
async fn identification_blinks_real_fixture_and_stop_restores_captured_level() {
    for level in [0, 100] {
        let fixture = Fixture::new();
        let emulator = pty::spawn(scenario(level)).await.unwrap();
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
        let reply = command(&broker, "blink", TARGET, 1).await;
        assert_eq!(
            reply["value"]["phase"], "blinking",
            "Blink did not start owned identification: {reply}"
        );
        assert_eq!(
            physical(&emulator.emulator).await[0]["outputs"][0]["level"],
            254,
            "Blink did not recall actual maximum"
        );
        wait_for(&handle, |s| {
            s.outputs.get(TARGET).and_then(|s| s.brightness()) == Some(0.0)
        })
        .await;
        let reply = command(&broker, "stop_blink", TARGET, 1).await;
        assert_eq!(
            reply["value"]["phase"], "restored",
            "Stop did not confirm restoration: {reply}"
        );
        assert_eq!(
            physical(&emulator.emulator).await[0]["outputs"][0]["level"],
            level,
            "Stop failed to restore captured actual level"
        );
        assert_eq!(
            fixture.disk().revision,
            1,
            "identification changed saved inventory"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn identification_changed_rename_restores_but_failed_and_noop_rename_keep_blinking() {
    let fixture = Fixture::new();
    let emulator = pty::spawn(scenario(73)).await.unwrap();
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
    let reply = command(&broker, "blink", TARGET, 1).await;
    assert_eq!(
        reply["value"]["phase"], "blinking",
        "Blink did not start owned identification: {reply}"
    );
    assert!(handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "".into()
            },
            context(1),
            until()
        )
        .await
        .is_err());
    assert!(
        command(&broker, "blink", TARGET, 1)
            .await
            .get("error")
            .is_some(),
        "failed rename ended active identification"
    );
    handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Light1".into(),
            },
            context(1),
            until(),
        )
        .await
        .unwrap();
    assert!(
        command(&broker, "blink", TARGET, 1)
            .await
            .get("error")
            .is_some(),
        "no-op rename ended active identification"
    );
    let revision = handle.snapshot().unwrap().model.inventory().revision;
    handle
        .edit(
            revision,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            },
            context(1),
            until(),
        )
        .await
        .unwrap();
    wait_for(&handle, |s| {
        s.outputs
            .get("Bus1.Outputs.Kitchen")
            .and_then(|s| s.level())
            == Some(crate::dali::gear::LevelValue::Known(
                crate::dali::gear::Level::new(73).unwrap(),
            ))
    })
    .await;
    assert_eq!(
        physical(&emulator.emulator).await[0]["outputs"][0]["level"],
        73,
        "changed rename did not restore continuing fixture"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn identification_refuses_foreground_write_and_failed_save_keeps_session() {
    let fixture = Fixture::new();
    let emulator = pty::spawn(scenario(83)).await.unwrap();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    service.hooks = Hooks {
        callback: Some(Arc::new(|step| {
            if step == Step::BeforeRename(Stage::Reservation) {
                Err("injected identification rename failure".into())
            } else {
                Ok(())
            }
        })),
    };
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
    let reply = command(&broker, "blink", TARGET, 1).await;
    assert_eq!(
        reply["value"]["phase"], "blinking",
        "Blink did not start owned identification: {reply}"
    );
    assert!(
        handle
            .apply_output(
                TARGET,
                crate::dali::gear::Intent::new(Some(false), None).unwrap(),
                context(1),
                until()
            )
            .await
            .is_err(),
        "foreground intent can be overwritten by identification restoration"
    );
    assert!(handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into()
            },
            context(1),
            until()
        )
        .await
        .is_err());
    assert_eq!(fixture.disk().revision, 1);
    assert!(
        command(&broker, "blink", TARGET, 1)
            .await
            .get("error")
            .is_some(),
        "failed save ended identification"
    );
    assert_eq!(
        command(&broker, "stop_blink", TARGET, 1).await["value"]["phase"],
        "restored"
    );
    assert_eq!(
        physical(&emulator.emulator).await[0]["outputs"][0]["level"],
        83
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn identification_capture_refuses_mask_and_fading_before_any_mutation() {
    for (level, status) in [(255, 4), (73, 20)] {
        let fixture = Fixture::new();
        let peer = peer::Peer::new(Some((level, status)), false);
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
        let reply = command(&broker, "blink", TARGET, 1).await;
        assert!(
            reply.get("error").is_some(),
            "identification accepted unknown or fading capture: {reply}"
        );
        assert!(
            peer.frames
                .lock()
                .unwrap()
                .iter()
                .all(|f| matches!(f, 0x01a0 | 0x0190)),
            "unknown or fading identification capture mutated the fixture"
        );
        assert!(running.stop().await);
        peer.close().await;
    }
}

#[tokio::test]
async fn identification_power_can_activate_staged_dt8_but_issues_no_explicit_colour_settings() {
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
        peer.model.lock().unwrap().snapshot()["buses"][0]["outputs"][0]["colour_state"]["actual"]
            ["temperature"]["value"],
        250
    );
    assert_eq!(
        command(&broker, "blink", TARGET, 1).await["value"]["phase"],
        "blinking"
    );
    assert_eq!(
        command(&broker, "stop_blink", TARGET, 1).await["value"]["phase"],
        "restored"
    );
    let snapshot = peer.model.lock().unwrap().snapshot();
    let gear = &snapshot["buses"][0]["outputs"][0];
    assert_eq!(
        gear["colour_state"]["actual"]["temperature"]["value"], 350,
        "identification hid ordinary DT8 power auto-activation behavior"
    );
    assert_eq!(gear["colour_state"]["temporary"]["temperature"], 65535);
    assert_eq!(gear["colour"]["auto_activation"], true);
    assert_eq!(gear["level"], 73);
    assert!(
        peer.frames
            .lock()
            .unwrap()
            .iter()
            .all(|f| matches!(f, 0x01a0 | 0x0190 | 0x0105 | 0x0100 | 0x0049)),
        "identification issued explicit colour or settings writes"
    );
    assert!(running.stop().await);
    peer.close().await;
}

#[tokio::test]
async fn identification_disable_and_removal_interrupt_without_restoring_retired_route() {
    for remove in [false, true] {
        let fixture = Fixture::new();
        let emulator = pty::spawn(scenario(73)).await.unwrap();
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
        assert_eq!(
            command(&broker, "blink", TARGET, 1).await["value"]["phase"],
            "blinking"
        );
        let edit = if remove {
            MetadataEdit::RemoveOutput { bus: 0, address: 0 }
        } else {
            MetadataEdit::ConfigureBus {
                bus: 0,
                name: "Bus1".into(),
                enabled: false,
            }
        };
        handle.edit(1, edit, context(1), until()).await.unwrap();
        wait_for(&handle, |s| {
            s.identification
                .as_ref()
                .is_some_and(|i| i.phase == crate::service::identification::Stage::Interrupted)
        })
        .await;
        assert_eq!(
            physical(&emulator.emulator).await[0]["outputs"][0]["level"],
            254,
            "identification restored a removed or disabled route"
        );
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}
