use super::*;
use crate::{
    atx::transactions::BusReply,
    dali::gear::{Destination, Query, ShortAddress},
    service::tests::{context, model, snapshot},
};

#[tokio::test]
async fn managed_output_settings_identification_guards_submission_and_owner_race() {
    use crate::{
        atx::transactions::RepeatPolicy,
        emulator::{pty, session::Control},
        service::{
            output::settings::Patch,
            tests::{start, wait_for, Fixture},
        },
    };
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    // Prove submission itself refuses the active claim, before an owner can
    // mask a missing guard by rejecting a queued request later.
    let (mut unopened, blocked, _) = fixture();
    let mut refused = Box::pin(blocked.configure_output(
        "Bus1.Outputs.Light1",
        1,
        Patch {
            minimum_level: Some(5),
            ..Default::default()
        },
        context(1),
        Instant::now() + Duration::from_secs(2),
    ));
    poll_fn(|cx| {
        match refused.as_mut().poll(cx) {
            Poll::Ready(Err(ServiceError::Rejected(_))) => {}
            _ => panic!("settings submission queued work despite an active identification claim"),
        };
        Poll::Ready(())
    })
    .await;
    assert!(
        unopened.receive.try_recv().is_err(),
        "blocked settings reached the owner queue"
    );
    drop(refused);
    drop(unopened);
    let fixture = Fixture::new();
    fixture.add_output(1, "Peer");
    let emulator=pty::spawn(serde_json::from_value(serde_json::json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100},{"id":"peer","short_address":1,"level":100}]}]})).unwrap()).await.unwrap();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let claims = service.identification.clone();
    let running = start(
        service.with_uart(&emulator.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let location = Location::Output {
        bus: 0,
        short_address: 0,
    };
    let target = "Bus1.Outputs.Light1";
    let claim = Claim {
        status: Status {
            target: target.into(),
            generation: context(1).generation().unwrap().into(),
            phase: Stage::Blinking,
            actual_level: Some(100),
        },
        origin: handle
            .snapshot()
            .unwrap()
            .model
            .origin(location)
            .unwrap()
            .clone(),
        location,
    };
    let patch = Patch {
        minimum_level: Some(5),
        ..Default::default()
    };
    for after_submission in [false, true] {
        claims.send_replace(if after_submission {
            None
        } else {
            Some(claim.clone())
        });
        let mut call = Box::pin(handle.configure_output(
            target,
            1,
            patch.clone(),
            context(1),
            Instant::now() + Duration::from_secs(2),
        ));
        if after_submission {
            poll_fn(|cx| {
                assert!(call.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            claims.send_replace(Some(claim.clone()));
        }
        assert!(
            matches!(call.await,Err(ServiceError::Rejected(ref e))if e.contains("stop identification")),
            "settings ignored identification at submission/owner boundary"
        );
        assert_eq!(
            handle
                .read_output_settings(target, context(1), Instant::now() + Duration::from_secs(2))
                .await
                .unwrap()
                .outcome,
            "current",
            "identification blocked a read-only settings operation"
        );
        assert_eq!(
            handle
                .configure_output(
                    "Bus1.Outputs.Peer",
                    1,
                    patch.clone(),
                    context(1),
                    Instant::now() + Duration::from_secs(2)
                )
                .await
                .unwrap()
                .outcome,
            "configured",
            "identification blocked an unrelated fixture"
        );
    }
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["min_level"], 1,
        "claimed fixture was configured despite the guard"
    );
    assert_eq!(physical["buses"][0]["outputs"][1]["min_level"], 5);
    claims.send_replace(None);
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

fn fixture() -> (DaliService, ServiceHandle, Claim) {
    let (service, handle) = DaliService::new(
        ConfigPaths::in_directory(std::path::Path::new("/unused-identification-test")),
        "Controller".into(),
    );
    let model = model();
    let location = Location::Output {
        bus: 0,
        short_address: 0,
    };
    let claim = Claim {
        status: Status {
            target: "Bus1.Outputs.Light1".into(),
            generation: context(1).generation().unwrap().into(),
            phase: Stage::Starting,
            actual_level: None,
        },
        origin: model.origin(location).unwrap().clone(),
        location,
    };
    let mut snapshot = snapshot(model, Phase::Ready);
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
    service.identification.send_replace(Some(claim.clone()));
    (service, handle, claim)
}
async fn next(service: &mut DaliService) -> output::Request {
    let Message::Output(request) = service.receive.recv().await.unwrap() else {
        panic!("identification sent non-output work")
    };
    request
}
fn respond(service: Option<&DaliService>, request: output::Request, level: u8) {
    let sample = output::Observation {
        device_types: vec![],
        context: request.context.clone(),
        actual_level: Some(Query::ActualLevel.decode(BusReply::Value(level))),
        status: Some(Query::Status.decode(BusReply::Value(4))),
        colour: Some(output::colour::Observation {
            dt8: Some(false),
            quality: output::colour::Quality::Unsupported,
            ..Default::default()
        }),
        quality: output::Quality::Current,
        reason: None,
        stamp: output::Stamp {
            session: 1,
            actor: 2,
            bus: 3,
            observed_bus: 4,
        },
        bus: 0,
    };
    if let Some(service) = service {
        let mut snapshot = service.handle.snapshot().unwrap();
        snapshot
            .outputs
            .insert(request.target.clone(), sample.clone());
        service.state.send_replace(Some(snapshot));
    }
    drop(request._permit);
    let _ = request.reply.send(Ok(sample));
}
fn request() -> (Request, oneshot::Receiver<Result<Status, ServiceError>>) {
    let (reply, receive) = oneshot::channel();
    (
        Request {
            target: "Bus1.Outputs.Light1".into(),
            stop: false,
            context: context(1).with_traceparent(None),
            deadline: Instant::now() + Duration::from_secs(4),
            reply,
        },
        receive,
    )
}

#[tokio::test(start_paused = true)]
async fn identification_ready_capture_rechecks_latest_cache_before_first_mutation() {
    use std::{future::Future, task::Poll};
    let (mut service, handle, claim) = fixture();
    let (request, _started) = request();
    let (_stop, receive) = watch::channel(None);
    let mut running = Box::pin(run(
        handle,
        service.identification.clone(),
        claim,
        request,
        receive,
    ));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    let capture = next(&mut service).await;
    respond(None, capture, 73);
    let result = std::future::poll_fn(|cx| Poll::Ready(running.as_mut().poll(cx))).await;
    assert!(
        matches!(result, Poll::Ready(Err(_))),
        "identification accepted stale ready capture before its first mutation"
    );
    assert!(
        service.receive.try_recv().is_err(),
        "stale capture queued an identification write"
    );
}

#[tokio::test(start_paused = true)]
async fn identification_ready_restore_rejection_clears_active_status() {
    let (mut service, handle, claim) = fixture();
    let (request, started) = request();
    let (stop, receive) = watch::channel(None);
    let task = tokio::spawn(run(
        handle,
        service.identification.clone(),
        claim,
        request,
        receive,
    ));
    let capture = next(&mut service).await;
    respond(Some(&service), capture, 73);
    let pulse = next(&mut service).await;
    respond(Some(&service), pulse, 254);
    started.await.unwrap().unwrap();
    stop.send_replace(Some(Instant::now() + STEP));
    let restore = next(&mut service).await;
    respond(None, restore, 73);
    assert!(
        task.await.unwrap().is_err(),
        "identification confirmed restoration from superseded evidence"
    );
    assert_eq!(
        service
            .identification
            .borrow()
            .as_ref()
            .unwrap()
            .status
            .phase,
        Stage::Interrupted,
        "restore acceptance failure left identification permanently active"
    );
}

#[tokio::test(start_paused = true)]
async fn identification_expires_after_two_minutes_and_restores_once_with_original_epoch_and_trace_absence(
) {
    let (mut service, handle, claim) = fixture();
    let (request, started) = request();
    let (_stop, receive) = watch::channel(None);
    let task = tokio::spawn(run(
        handle,
        service.identification.clone(),
        claim,
        request,
        receive,
    ));
    let capture = next(&mut service).await;
    assert!(capture.intent.is_none());
    assert!(capture.include_colour);
    respond(Some(&service), capture, 73);
    let pulse = next(&mut service).await;
    assert_eq!(
        pulse.required_stamp,
        Some(output::Stamp {
            session: 1,
            actor: 2,
            bus: 3,
            observed_bus: 4
        }),
        "identification pulse discarded its captured physical epoch"
    );
    assert_eq!(
        pulse.context.traceparent(),
        None,
        "identification invented a trace for an absent cause"
    );
    assert!(
        !pulse.include_colour,
        "identification queried or set explicit DT8 colour"
    );
    respond(Some(&service), pulse, 254);
    assert_eq!(started.await.unwrap().unwrap().phase, Stage::Blinking);
    tokio::time::advance(Duration::from_secs(120)).await;
    let restore = next(&mut service).await;
    let output::WriteIntent::Base(intent) = restore.intent.unwrap() else {
        panic!("non-base identification write")
    };
    let frame = intent.frame(Destination::Short(ShortAddress::new(0).unwrap()));
    assert_eq!(
        frame, 73,
        "identification exceeded two minutes instead of restoring captured level"
    );
    assert_eq!(
        restore.required_stamp,
        Some(output::Stamp {
            session: 1,
            actor: 2,
            bus: 3,
            observed_bus: 4
        }),
        "restoration refreshed the original physical epoch"
    );
    assert_eq!(restore.context.traceparent(), None);
    respond(Some(&service), restore, 70);
    let result = task.await.unwrap().unwrap();
    assert_eq!(result.phase, Stage::Restored);
    assert_eq!(
        result.actual_level,
        Some(70),
        "restoration echoed captured intent instead of actual readback"
    );
    assert!(
        service.receive.try_recv().is_err(),
        "identification retried after terminal restoration"
    );
}

#[tokio::test(start_paused = true)]
async fn identification_startup_cancel_restores_and_failed_restore_stays_interrupted() {
    for fail in [false, true] {
        let (mut service, handle, claim) = fixture();
        let (request, started) = request();
        let (_stop, receive) = watch::channel(None);
        let task = tokio::spawn(run(
            handle,
            service.identification.clone(),
            claim,
            request,
            receive,
        ));
        let capture = next(&mut service).await;
        drop(started);
        respond(Some(&service), capture, 73);
        let restore = next(&mut service).await;
        let output::WriteIntent::Base(intent) = restore.intent.unwrap() else {
            panic!("non-base identification write")
        };
        let frame = intent.frame(Destination::Short(ShortAddress::new(0).unwrap()));
        assert_eq!(
            frame, 73,
            "startup cancellation issued a blink pulse instead of restoring"
        );
        if fail {
            drop(restore._permit);
            let _ = restore
                .reply
                .send(Err(ServiceError::Uncertain("injected restore loss".into())));
        } else {
            respond(Some(&service), restore, 73);
        }
        let result = task.await.unwrap();
        assert_eq!(
            result.is_err(),
            fail,
            "failed restoration was reported as confirmed"
        );
        assert_eq!(
            service
                .identification
                .borrow()
                .as_ref()
                .unwrap()
                .status
                .phase,
            if fail {
                Stage::Interrupted
            } else {
                Stage::Restored
            }
        );
    }
}

#[tokio::test]
async fn identification_origin_follows_rename_but_not_replacement_and_blocks_new_target_writes() {
    let (service, handle, claim) = fixture();
    let original = handle.snapshot().unwrap().model;
    let renamed = original
        .edit(
            1,
            &[Edit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            }],
        )
        .unwrap();
    service
        .state
        .send_replace(Some(snapshot(renamed.clone(), Phase::Ready)));
    let (target, cause) = continuing(&handle, &claim, &context(1)).unwrap();
    assert_eq!(target, "Bus1.Outputs.Kitchen");
    assert_eq!(cause.generation(), context(2).generation());
    assert!(
        blocks(&handle, &target),
        "rename-to-restore window admitted foreground intent"
    );
    let removed = renamed
        .edit(2, &[Edit::RemoveOutput { bus: 0, address: 0 }])
        .unwrap();
    let replaced = removed
        .edit(
            3,
            &[Edit::AddOutput {
                bus: 0,
                output: crate::config::Output {
                    short_address: 0,
                    name: "Kitchen".into(),
                    description: None,
                    color_representation: None,
                },
            }],
        )
        .unwrap();
    service
        .state
        .send_replace(Some(snapshot(replaced, Phase::Ready)));
    assert!(
        continuing(&handle, &claim, &context(1)).is_err(),
        "identification followed replacement at reused physical address"
    );
    assert!(!blocks(&handle, "Bus1.Outputs.Kitchen"));
}

#[tokio::test(start_paused = true)]
async fn identification_metadata_waiter_gets_permit_without_refreshing_authority() {
    use std::{future::Future, task::Poll};
    let (mut service, handle, _claim) = fixture();
    let held = handle.permit.clone().try_acquire_owned().unwrap();
    let mut rename = Box::pin(handle.edit(
        1,
        MetadataEdit::RenameOutput {
            bus: 0,
            address: 0,
            name: "Kitchen".into(),
        },
        context(1),
        Instant::now() + Duration::from_secs(2),
    ));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(rename.as_mut().poll(cx)))
            .await
            .is_pending(),
        "identification made inline rename Busy instead of waiting for its pulse"
    );
    drop(held);
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(rename.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    let Message::Change(request) = service.receive.recv().await.unwrap() else {
        panic!("rename never reached owner")
    };
    assert!(
        handle.permit.clone().try_acquire_owned().is_err(),
        "next blink pulse overtook a queued metadata waiter"
    );
    assert_eq!(request.context.generation(), context(1).generation());
    assert!(
        matches!(request.change, Change::Edit { expected: 1, .. }),
        "metadata wait rebased the original revision"
    );
    drop(request._permit);
    let _ = request
        .reply
        .send(Err(ServiceError::Rejected("test owner rejects".into())));
    assert!(rename.await.is_err());
}
