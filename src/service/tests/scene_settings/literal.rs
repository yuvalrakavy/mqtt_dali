use super::*;
use crate::service::scene_settings::Action;

#[tokio::test]
async fn scene_settings_literal_clears_pending_color_and_distinguishes_partial_writes() {
    for fault in [0, 1, 2, 4] {
        let fixture = Fixture::new();
        save_scene(&fixture);
        let peer = peer::Peer::new(fault);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let report = handle
            .change_scene_row(
                SCENE,
                Action::Set {
                    fixture: OUTPUT.into(),
                    setting: serde_json::from_value(base_setting(80)).unwrap(),
                    expected_revision: 1,
                },
                context(1),
                Instant::now() + Duration::from_secs(8),
            )
            .await
            .unwrap();
        assert_eq!(
            report.outcome,
            if fault == 0 {
                "verified"
            } else {
                "unconfirmed"
            },
            "partial scene write claimed verification"
        );
        assert_eq!(report.revision, 2);
        assert_eq!(fixture.disk().buses[0].scenes[0].contents.len(), 1);
        if fault == 0 {
            assert_eq!(peer.level.load(Ordering::SeqCst), 80);
            assert_eq!(
                peer.kind.load(Ordering::SeqCst),
                255,
                "pending temporary colour contaminated brightness-only scene"
            );
            let frames = peer.frames.lock().unwrap();
            let start = frames.iter().position(|f| f == "t0150").unwrap();
            assert_eq!(
                &frames[start..start + 9],
                &["t0150", "h01B0", "hA3F0", "hC108", "h01FA", "hC108", "h01EE", "hA350", "t0140"],
                "native scene plan did not select cleared masked report before staging and Store"
            );
        } else if fault == 1 {
            assert!(
                !peer.frames.lock().unwrap().iter().any(|f| f == "t0140"),
                "failed Remove was followed by Store"
            );
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn scene_settings_literal_adoption_is_read_only_and_repair_preserves_saved_intent() {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let peer = peer::Peer::new(0);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let read = handle
        .read_scene(
            OUTPUT,
            crate::dali::scenes::Slot::new(0).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    let reviewed = read.stored.unwrap();
    let adopted = handle
        .change_scene_row(
            SCENE,
            Action::Adopt {
                fixture: OUTPUT.into(),
                reviewed: reviewed.clone(),
                expected_revision: 1,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(
        peer.settings().is_empty(),
        "adoption programmed native scene settings"
    );
    assert_eq!(adopted.outcome, "adopted");
    assert_eq!(adopted.revision, 2);
    peer.level.store(77, Ordering::SeqCst);
    assert!(
        handle
            .change_scene_row(
                SCENE,
                Action::Repair {
                    fixture: OUTPUT.into(),
                    reviewed,
                    expected_revision: 2
                },
                context(1),
                deadline()
            )
            .await
            .is_err(),
        "repair accepted changed review"
    );
    assert!(
        peer.settings().is_empty(),
        "stale review programmed settings"
    );
    let changed = handle
        .read_scene(
            OUTPUT,
            crate::dali::scenes::Slot::new(0).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(changed.comparison).unwrap(),
        "different"
    );
    let repaired = handle
        .change_scene_row(
            SCENE,
            Action::Repair {
                fixture: OUTPUT.into(),
                reviewed: changed.stored.unwrap(),
                expected_revision: 2,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        peer.level.load(Ordering::SeqCst),
        42,
        "repair adopted hardware drift instead of saved intent"
    );
    assert_eq!(repaired.outcome, "verified");
    assert_eq!(repaired.revision, 2);
    assert_eq!(
        fixture.disk().buses[0].scenes[0].contents[0].setting.level,
        crate::config::scene_settings::Level::Value { value: 42 }
    );
    peer.level.store(77, Ordering::SeqCst);
    let repeated = handle
        .change_scene_row(
            SCENE,
            Action::Set {
                fixture: OUTPUT.into(),
                setting: repaired.saved.unwrap(),
                expected_revision: 2,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(repeated.outcome, "verified");
    assert_eq!(repeated.revision, 2);
    assert_eq!(
        peer.level.load(Ordering::SeqCst),
        42,
        "no-op desired Set skipped programming drifting hardware"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn scene_settings_literal_caller_timeout_keeps_saved_intent_and_refuses_new_uart_work() {
    let fixture = Fixture::new();
    save_scene(&fixture);
    let peer = peer::Peer::new(3);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let h = handle.clone();
    let task = tokio::spawn(async move {
        h.change_scene_row(
            SCENE,
            Action::Set {
                fixture: OUTPUT.into(),
                setting: serde_json::from_value(base_setting(80)).unwrap(),
                expected_revision: 1,
            },
            context(1),
            Instant::now() + Duration::from_secs(2),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(1), peer.held.notified())
        .await
        .unwrap();
    assert!(
        matches!(
            handle.read_output(OUTPUT, context(1), deadline()).await,
            Err(ServiceError::Busy)
        ),
        "active settings released the physical permit"
    );
    assert!(matches!(
        task.await.unwrap(),
        Err(ServiceError::Uncertain(_))
    ));
    assert_eq!(fixture.disk().revision, 2);
    let before = peer.frames.lock().unwrap().len();
    assert!(
        handle
            .read_output(OUTPUT, context(1), deadline())
            .await
            .is_err(),
        "timed-out settings admitted new UART work before ordering recovered"
    );
    assert_eq!(
        peer.frames.lock().unwrap().len(),
        before,
        "new UART command crossed unresolved settings terminal"
    );
    peer.release.notify_one();
    let permit = tokio::time::timeout(Duration::from_secs(3), handle.permit.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert!(running.stop().await);
}
