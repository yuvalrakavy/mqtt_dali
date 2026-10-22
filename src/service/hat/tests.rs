use super::*;
use crate::service::tests::{context, deadline, start, wait_for, Fixture};
use crate::service::{MetadataEdit, Phase};
use std::ffi::CStr;
use std::os::fd::{FromRawFd, OwnedFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(in crate::service) struct Pty {
    pub(in crate::service) peer: Serial,
    _slave: OwnedFd,
    pub(in crate::service) path: PathBuf,
}
impl Pty {
    pub(in crate::service) fn new() -> Self {
        let (mut master, mut slave) = (-1, -1);
        let mut name = [0 as libc::c_char; 1024];
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    name.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let path = PathBuf::from(unsafe { CStr::from_ptr(name.as_ptr()) }.to_str().unwrap());
        Self {
            peer: Serial::from_owned_fd(master).unwrap(),
            _slave: slave,
            path,
        }
    }
}
async fn line(peer: &mut Serial) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut line = Vec::new();
        loop {
            let byte = peer.read_u8().await.unwrap();
            line.push(byte);
            if byte == b'\n' {
                return line;
            }
            assert!(line.len() < 32);
        }
    })
    .await
    .expect("HAT query missing")
}
async fn quiet(peer: &mut Serial) {
    assert!(
        tokio::time::timeout(Duration::from_millis(100), peer.read_u8())
            .await
            .is_err(),
        "wrote through unresolved HAT response"
    );
}
async fn drive(hat: &mut Hat, predicate: impl Fn(&Hardware) -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate(&hat.hardware()) {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("hardware did not settle: {:?}", hat.hardware()));
}
fn settings(path: PathBuf) -> Settings {
    let mut settings = Settings::new(path, RepeatPolicy::Unverified);
    settings.quiet = Duration::from_millis(20);
    settings.timing = Timing {
        setup: Duration::from_millis(80),
        poll: Duration::from_secs(60),
        retry: Duration::from_millis(20),
    };
    settings
}

async fn ready_output_hat(pty: &mut Pty) -> Hat {
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"V013204\n").await.unwrap();
    tokio::select! {result=hat.changed()=>result.unwrap(),_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    assert_eq!(line(&mut pty.peer).await, b"d\n");
    pty.peer.write_all(b"D24\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Ready { .. })).await;
    hat
}
fn queue_output(
    hat: &mut Hat,
    intent: Option<crate::dali::gear::Intent>,
    until: Instant,
) -> (
    tokio::sync::oneshot::Receiver<Result<output::Observation, ServiceError>>,
    std::sync::Arc<tokio::sync::Semaphore>,
) {
    let admission = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let (reply, receive) = tokio::sync::oneshot::channel();
    let route = output::Route {
        controller: "Controller".into(),
        bus: 0,
        address: crate::dali::gear::ShortAddress::new(0).unwrap(),
        context: context(1),
    };
    hat.enqueue_output(
        output::Request {
            target: "Bus1.Outputs.Light1".into(),
            intent: intent.map(output::WriteIntent::Base),
            include_colour: false,
            context: context(1),
            deadline: until,
            reply,
            _permit: admission.clone().try_acquire_owned().unwrap(),
        },
        route,
    );
    (receive, admission)
}
async fn output_complete(hat: &mut Hat) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while hat.completed.is_none() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn write_completion_uses_observed_level_instead_of_echoing_requested_brightness() {
    let mut pty = Pty::new();
    let mut hat = ready_output_hat(&mut pty).await;
    let (receive, _admission) = queue_output(
        &mut hat,
        Some(crate::dali::gear::Intent::new(None, Some(1.0)).unwrap()),
        Instant::now() + Duration::from_secs(1),
    );
    hat.changed().await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h00FE\n");
    pty.peer.write_all(b"N\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h01A0\n");
    pty.peer.write_all(b"J7F\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h0190\n");
    pty.peer.write_all(b"J04\n").await.unwrap();
    output_complete(&mut hat).await;
    let completed = hat.take_completed_output().unwrap();
    assert!(completed.error.is_none());
    assert_eq!(
        completed.observation.as_ref().unwrap().brightness(),
        Some(0.5),
        "write completion echoed requested brightness"
    );
    drop(completed);
    drop(receive);
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn output_samples_distinguish_mask_missing_collision_and_partial_answers() {
    use crate::dali::gear::{Level, LevelValue, QueryReply};
    use output::Quality;
    for (level, status, quality, expected, available) in [
        (
            b"JFF\n".as_slice(),
            b"J04\n".as_slice(),
            Quality::Partial,
            Some(LevelValue::UnavailableMask),
            true,
        ),
        (b"N\n", b"N\n", Quality::Unavailable, None, false),
        (b"X\n", b"J04\n", Quality::Partial, None, false),
        (
            b"J7F\n",
            b"N\n",
            Quality::Partial,
            Some(LevelValue::Known(Level::new(127).unwrap())),
            false,
        ),
        (
            b"J00\n",
            b"J00\n",
            Quality::Current,
            Some(LevelValue::Known(Level::new(0).unwrap())),
            true,
        ),
    ] {
        let mut pty = Pty::new();
        let mut hat = ready_output_hat(&mut pty).await;
        let (receive, _admission) =
            queue_output(&mut hat, None, Instant::now() + Duration::from_secs(1));
        let responder = tokio::spawn(async move {
            assert_eq!(line(&mut pty.peer).await, b"h01A0\n");
            pty.peer.write_all(b"L020401\n").await.unwrap();
            pty.peer.write_all(level).await.unwrap();
            assert_eq!(line(&mut pty.peer).await, b"h0190\n");
            pty.peer.write_all(status).await.unwrap();
            pty
        });
        output_complete(&mut hat).await;
        let completed = hat.take_completed_output().unwrap();
        assert!(completed.error.is_none());
        let observation = completed.observation.unwrap();
        assert_eq!(
            observation.quality, quality,
            "fixture sample quality was invented"
        );
        assert_eq!(
            observation.level(),
            expected,
            "fixture sample changed MASK/missing/collision meaning"
        );
        assert_eq!(observation.available(), available);
        if quality != Quality::Current {
            assert_eq!(
                observation.power(),
                None,
                "incomplete samples became known power"
            );
        }
        if level == b"X\n" {
            assert!(matches!(
                observation.actual_level,
                Some(QueryReply::ReceiveCollision(_))
            ));
        }
        let _ = completed.request.reply.send(Ok(observation));
        receive.await.unwrap().unwrap();
        let _pty = responder.await.unwrap();
        hat.close(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn physical_admission_outlives_frontend_loss_and_actual_transport_teardown() {
    use crate::dali::gear::Intent;
    let mut pty = Pty::new();
    let mut hat = ready_output_hat(&mut pty).await;
    let (receive, admission) = queue_output(
        &mut hat,
        Some(Intent::new(None, Some(1.0)).unwrap()),
        Instant::now() + Duration::from_secs(1),
    );
    hat.changed().await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h00FE\n");
    pty.peer.write_all(b"N\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h01A0\n");
    drop(receive);
    let link = hat.session.as_ref().unwrap().link.clone();
    hat.event(Ok(Event::Closed(CloseReason::Eof))).unwrap();
    assert_eq!(
        admission.available_permits(),
        0,
        "transport event released physical admission before backend teardown"
    );
    drop(pty);
    tokio::time::timeout(Duration::from_secs(1), async {
        while hat.job.as_ref().is_none_or(|job| job.1 != JobKind::Close) {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        admission.available_permits(),
        0,
        "frontend helper join released physical admission before transport join"
    );
    link.close().await.unwrap();
    assert_eq!(
        admission.available_permits(),
        0,
        "backend teardown released owner-held physical admission before collection"
    );
    output_complete(&mut hat).await;
    let completed = hat.take_completed_output().unwrap();
    assert!(matches!(completed.error, Some(ServiceError::Uncertain(_))));
    assert_eq!(admission.available_permits(), 0);
    drop(completed);
    assert_eq!(admission.available_permits(), 1);
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn canceled_or_expired_queued_output_retires_while_health_is_still_pending() {
    for cancel in [false, true] {
        let mut pty = Pty::new();
        let mut hat = ready_output_hat(&mut pty).await;
        let link = hat.session.as_ref().unwrap().link.clone();
        hat.spawn(JobKind::Query, async move {
            Done::Poll(query_health(&link, &[0], Instant::now() + Duration::from_secs(1)).await)
        });
        assert_eq!(line(&mut pty.peer).await, b"d\n");
        let (receive, admission) = queue_output(
            &mut hat,
            Some(crate::dali::gear::Intent::new(Some(true), None).unwrap()),
            Instant::now() + Duration::from_millis(20),
        );
        let mut receive = Some(receive);
        if cancel {
            drop(receive.take());
        }
        output_complete(&mut hat).await;
        assert!(
            hat.job
                .as_ref()
                .is_some_and(|job| job.1 == JobKind::Query && !job.0.is_finished()),
            "queued output waited for the independent health reply"
        );
        let completed = hat.take_completed_output().unwrap();
        assert_eq!(completed.error, Some(ServiceError::Expired));
        drop(completed);
        drop(receive);
        assert_eq!(
            admission.available_permits(),
            1,
            "queued cancellation retained service admission"
        );
        pty.peer.write_all(b"D24\n").await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while hat.job.is_some() {
                hat.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        quiet(&mut pty.peer).await;
        hat.close(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn queued_output_cannot_recapture_ready_after_unobserved_ordering_loss() {
    let mut pty = Pty::new();
    let mut hat = ready_output_hat(&mut pty).await;
    hat.settings.as_mut().unwrap().timing.retry = Duration::from_secs(60);
    let link = hat.session.as_ref().unwrap().link.clone();
    let query = link.clone();
    hat.spawn(JobKind::Query, async move {
        Done::Poll(
            query
                .execute(
                    Command::Send {
                        bus: 0,
                        frame: crate::atx::wire::DaliFrame::Bits16(0x0190),
                        repeat: false,
                    },
                    Duration::from_millis(20),
                )
                .await
                .map(|_| vec![]),
        )
    });
    assert_eq!(line(&mut pty.peer).await, b"h0190\n");
    let (receive, _admission) = queue_output(
        &mut hat,
        Some(crate::dali::gear::Intent::new(Some(true), None).unwrap()),
        Instant::now() + Duration::from_secs(1),
    );
    let captured = hat.physical.as_ref().unwrap().stamp;
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hat.job.as_ref().unwrap().0.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    pty.peer.write_all(b"N\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while link.status() != SessionState::Ready {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_ne!(link.observation_epoch(), captured.actor);
    output_complete(&mut hat).await;
    let completed = hat.take_completed_output().unwrap();
    assert!(
        completed.observation.is_none(),
        "stale queued output adopted a new UART epoch"
    );
    assert!(matches!(completed.error, Some(ServiceError::Rejected(_))));
    drop(completed);
    drop(receive);
    quiet(&mut pty.peer).await;
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn queued_output_refuses_a_power_cycle_even_when_health_completed_successfully() {
    let mut pty = Pty::new();
    let mut hat = ready_output_hat(&mut pty).await;
    let link = hat.session.as_ref().unwrap().link.clone();
    let query = link.clone();
    hat.spawn(JobKind::Query, async move {
        Done::Poll(query_health(&query, &[0], Instant::now() + Duration::from_secs(1)).await)
    });
    assert_eq!(line(&mut pty.peer).await, b"d\n");
    let (receive, _admission) = queue_output(
        &mut hat,
        Some(crate::dali::gear::Intent::new(Some(true), None).unwrap()),
        Instant::now() + Duration::from_secs(1),
    );
    let captured = hat.physical.as_ref().unwrap().stamp;
    pty.peer.write_all(b"D04\nD24\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while link.bus_epoch(0) == captured.bus || !hat.job.as_ref().unwrap().0.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    hat.next = Instant::now() + Duration::from_secs(60);
    output_complete(&mut hat).await;
    let completed = hat.take_completed_output().unwrap();
    assert!(
        completed.observation.is_none(),
        "queued output bypassed its captured bus epoch before hardware admission"
    );
    assert!(matches!(completed.error, Some(ServiceError::Rejected(_))));
    drop(completed);
    drop(receive);
    quiet(&mut pty.peer).await;
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn completed_readback_before_a_power_cycle_is_partial_evidence_not_current_state() {
    let mut pty = Pty::new();
    let mut hat = ready_output_hat(&mut pty).await;
    let link = hat.session.as_ref().unwrap().link.clone();
    let (receive, _admission) =
        queue_output(&mut hat, None, Instant::now() + Duration::from_secs(1));
    hat.changed().await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h01A0\n");
    pty.peer.write_all(b"J7F\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h0190\n");
    pty.peer.write_all(b"J04\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while !hat.job.as_ref().unwrap().0.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let before = link.bus_epoch(0);
    pty.peer.write_all(b"D04\nD24\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while link.bus_epoch(0) == before {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    output_complete(&mut hat).await;
    let completed = hat.take_completed_output().unwrap();
    let observation = completed.observation.as_ref().unwrap();
    assert_eq!(
        observation.quality,
        output::Quality::Stale,
        "post-sample power cycle became a fresh readback"
    );
    assert_eq!(
        observation.level(),
        Some(crate::dali::gear::LevelValue::Known(
            crate::dali::gear::Level::new(127).unwrap()
        )),
        "invalidated readback lost its partial diagnostic evidence"
    );
    assert_eq!(observation.power(), None);
    assert!(matches!(completed.error, Some(ServiceError::Rejected(_))));
    drop(completed);
    drop(receive);
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn managed_stop_with_active_output_joins_hardware_despite_retained_clients() {
    let fixture = Fixture::new();
    let mut pty = Pty::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(pty.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"V013204\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"d\n");
    pty.peer.write_all(b"D24\n").await.unwrap();
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .apply_output(
                "Bus1.Outputs.Light1",
                crate::dali::gear::Intent::new(Some(true), None).unwrap(),
                context(1),
                deadline(),
            )
            .await
    });
    assert_eq!(line(&mut pty.peer).await, b"h0105\n");
    pty.peer.write_all(b"N\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"h01A0\n");
    drop(pty._slave);
    assert!(
        running.stop().await,
        "active output prevented managed teardown"
    );
    assert!(pending.await.unwrap().is_err());
    assert_eq!(handle.snapshot().unwrap().hardware, Hardware::Closed);
    let closed = tokio::time::timeout(Duration::from_millis(300), pty.peer.read_u8())
        .await
        .expect("retained output client kept UART alive");
    assert!(
        closed.is_err(),
        "closed output owner produced extra UART bytes"
    );
}

#[tokio::test]
async fn renewed_input_cannot_starve_completed_helper_or_hide_its_panic() {
    for panic in [false, true] {
        let pty = Pty::new();
        let serial = Serial::open(&pty.path).await.unwrap();
        let (link, _) = Transactions::start(serial, RepeatPolicy::Unverified);
        let (flood, events) = broadcast::channel(128);
        let status = link.observe_status();
        let mut hat = Hat::new(Some(settings(pty.path)), vec![0]);
        hat.session = Some(Session {
            link,
            events,
            status,
        });
        hat.version = Some(Version {
            hardware: "01".into(),
            firmware: "32".into(),
            variant: "04".into(),
        });
        hat.next = Instant::now() + Duration::from_secs(60);
        hat.published = hat.hardware();
        hat.spawn(JobKind::Query, async move {
            assert!(!panic, "injected saturated helper panic");
            Done::Poll(Ok(vec![BusHealth {
                bus: 0,
                status: 2,
                channels: 4,
            }]))
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while !hat.job.as_ref().unwrap().0.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut fatal = false;
        for _ in 0..3 {
            for sequence in 1..=128 {
                flood
                    .send(Event::Packet {
                        sequence,
                        packet: Ok(crate::atx::wire::Packet {
                            bus: 0,
                            kind: Kind::BusStatus {
                                status: 0,
                                channels: 4,
                            },
                        }),
                    })
                    .unwrap();
            }
            if tokio::time::timeout(Duration::from_millis(150), hat.changed())
                .await
                .expect("helper collection waited after processing input")
                .is_err()
            {
                fatal = true;
                break;
            }
            if hat.job.is_none() {
                break;
            }
        }
        assert!(
            hat.job.is_none(),
            "renewed input starved completed helper collection"
        );
        assert_eq!(fatal, panic, "renewed input hid a completed helper panic");
        if !panic {
            assert_eq!(
                hat.observations[0].status, 0,
                "older helper replaced proactive observation"
            );
        }
        hat.close(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn closed_query_before_public_eof_joins_transport_before_retry() {
    let mut pty = Pty::new();
    let serial = Serial::open(&pty.path).await.unwrap();
    let (link, _real_events) = Transactions::start(serial, RepeatPolicy::Unverified);
    // Hold the public EOF until the actor's real result has reached the owner.
    let (forward, events) = broadcast::channel(128);
    let status = link.observe_status();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    hat.session = Some(Session {
        link: link.clone(),
        events,
        status,
    });
    let query = link.clone();
    hat.spawn(JobKind::Query, async move {
        Done::Setup(
            query
                .initialize(Duration::from_secs(1))
                .await
                .map(|v| (v, vec![])),
        )
    });
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    drop(pty);
    tokio::time::timeout(Duration::from_secs(1), async {
        while link.status() != SessionState::Closed || !hat.job.as_ref().unwrap().0.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    hat.published = hat.hardware();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), hat.changed())
            .await
            .unwrap()
            .is_ok(),
        "normal EOF became fatal before public closure delivery"
    );
    // Complete/discard the query, then start the real owned close helper.
    tokio::time::timeout(Duration::from_secs(1), async {
        while hat.job.as_ref().is_none_or(|job| job.1 != JobKind::Close) {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    let close_id = hat.job.as_ref().unwrap().0.id();
    forward.send(Event::Closed(CloseReason::Eof)).unwrap();
    tokio::time::timeout(Duration::from_millis(150), hat.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(
        !hat.discard_job,
        "delayed EOF canceled the owned close helper"
    );
    if let Some(job) = &hat.job {
        assert_eq!(job.0.id(), close_id);
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while hat.session.is_some() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    // A successful real close is authoritative even when the frontend is retained.
    link.close().await.unwrap();
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn closed_status_before_public_eof_starts_owned_teardown() {
    let pty = Pty::new();
    let serial = Serial::open(&pty.path).await.unwrap();
    let (link, _) = Transactions::start(serial, RepeatPolicy::Unverified);
    let (_forward, events) = broadcast::channel(128);
    let status = link.observe_status();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    hat.next = Instant::now() + Duration::from_secs(60);
    hat.session = Some(Session {
        link: link.clone(),
        events,
        status,
    });
    drop(pty);
    tokio::time::timeout(Duration::from_secs(1), async {
        while link.status() != SessionState::Closed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    hat.published = hat.hardware();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), hat.changed())
            .await
            .unwrap()
            .is_ok(),
        "normal EOF status became fatal before public closure delivery"
    );
    assert!(hat.closing && hat.job.as_ref().is_some_and(|j| j.1 == JobKind::Close));
    tokio::time::timeout(Duration::from_secs(1), async {
        while hat.session.is_some() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    link.close().await.unwrap();
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_actor_join_remains_fatal_when_public_evidence_is_delayed() {
    let mut pty = Pty::new();
    let serial = Serial::open(&pty.path).await.unwrap();
    let (link, _) = Transactions::start(serial, RepeatPolicy::Unverified);
    let (_forward, events) = broadcast::channel(128);
    let status = link.observe_status();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    hat.session = Some(Session {
        link: link.clone(),
        events,
        status,
    });
    let query = link.clone();
    hat.spawn(JobKind::Query, async move {
        Done::Setup(
            query
                .initialize(Duration::from_secs(1))
                .await
                .map(|v| (v, vec![])),
        )
    });
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    link.abort_worker_for_test();
    let error = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Err(error) = hat.changed().await {
                return error;
            }
        }
    })
    .await
    .expect("failed actor join became a retry loop");
    assert!(
        error.contains("HAT close incomplete"),
        "failure skipped actual actor join: {error}"
    );
    assert!(
        hat.session.is_some(),
        "failed join released the session for reopening"
    );
    assert!(hat.job.is_none());
    assert_eq!(link.close().await, Err(TransactionError::StreamLoss));
}

#[tokio::test]
async fn uart_owner_measures_a_quiet_interval_before_first_initialization_without_boot() {
    let mut pty = Pty::new();
    let mut config = settings(pty.path.clone());
    config.quiet = Duration::from_millis(250);
    let started = Instant::now();
    let mut hat = Hat::new(Some(config), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    assert!(
        started.elapsed() >= Duration::from_millis(250),
        "initial UART drain did not measure the quiet interval"
    );
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn healthy_uart_remains_independent_of_broker_and_metadata_updates_preserve_health() {
    let fixture = Fixture::new();
    let mut pty = Pty::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    broker.refuse_connections(true);
    let (service, handle) = fixture.service();
    let service = service.with_uart(pty.path.clone(), RepeatPolicy::Unverified);
    let running = start(service, &handle, &broker).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"V013204\n").await.unwrap();
    assert_eq!(line(&mut pty.peer).await, b"d\n");
    pty.peer.write_all(b"D24\n").await.unwrap();
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let hardware = handle.snapshot().unwrap().hardware;
    let saved = handle
        .edit(
            1,
            MetadataEdit::Description(Some("UART online".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        saved.hardware, hardware,
        "metadata update erased hardware facts"
    );
    assert_eq!(saved.phase, Phase::Ready);
    pty.peer.write_all(b"D04\n3L123456\n").await.unwrap();
    wait_for(&handle,|s|matches!(&s.hardware,Hardware::Ready { buses,.. } if buses.iter().any(|b|b.bus==0&&b.status==0))).await;
    drop(pty._slave);
    assert!(running.stop().await);
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Closed);
    assert_eq!(
        handle.snapshot().unwrap().hardware,
        Hardware::Closed,
        "closed service retained ready hardware"
    );
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_millis(300), pty.peer.read(&mut byte))
        .await
        .expect("retained frontend kept the UART descriptor alive");
    assert!(
        matches!(closed, Ok(0) | Err(_)),
        "UART produced unexpected shutdown data"
    );
}

#[tokio::test]
async fn late_version_recovers_the_same_owner_after_a_quiet_interval_without_boot() {
    let mut pty = Pty::new();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    drive(&mut hat, |h| {
        matches!(
            h,
            Hardware::WaitingLateReply { .. } | Hardware::Resynchronizing
        )
    })
    .await;
    let original = hat.session.as_ref().unwrap().link.clone();
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_ne!(
        original.status(),
        SessionState::Closed,
        "late reply recovery discarded the owned descriptor"
    );
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"V013204\n").await.unwrap();
    // The local test must continue polling the lifecycle while answering UART.
    tokio::select! {result=hat.changed()=>result.unwrap(),_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    assert_eq!(line(&mut pty.peer).await, b"d\n");
    pty.peer.write_all(b"D24\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Ready { .. })).await;
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn receive_corruption_recovers_after_quiet_without_boot_or_reopening() {
    let mut pty = Pty::new();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"invalid\n").await.unwrap();
    drive(&mut hat, |h| {
        matches!(h, Hardware::ResetRequired | Hardware::Resynchronizing)
    })
    .await;
    let original = hat.session.as_ref().unwrap().link.clone();
    // These remain observations while the measured quiet boundary is acquired.
    pty.peer.write_all(b"D24\nV013204\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_ne!(
        original.status(),
        SessionState::Closed,
        "reset discarded the poisoned owner"
    );
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn reopen_boundary_excludes_complete_and_partial_buffered_boot_records() {
    for partial in [false, true] {
        let mut pty = Pty::new();
        pty.peer
            .write_all(if partial { b"B0132" } else { b"B013204\n" })
            .await
            .unwrap();
        let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
        hat.reset = true;
        hat.requires_boot = true;
        tokio::time::timeout(Duration::from_secs(2), async {
            while hat.boundary.is_none() && hat.reset {
                hat.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        if partial {
            pty.peer.write_all(b"04\n").await.unwrap();
        }
        for _ in 0..5 {
            tokio::select! {_=hat.changed()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
        }
        assert!(hat.reset, "buffered pre-open boot cleared the reopen latch");
        quiet(&mut pty.peer).await;
        pty.peer.write_all(b"B013204\n").await.unwrap();
        drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
        assert_eq!(line(&mut pty.peer).await, b"v\n");
        hat.close(Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn ordinary_open_errors_retry_but_opener_panics_fail_the_owned_helper() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let pty = Pty::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = calls.clone();
    let mut config = settings(pty.path.clone());
    config.open_hook = Some(Arc::new(move || {
        if entered.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(io::Error::other("injected open failure"))
        } else {
            Ok(())
        }
    }));
    let mut hat = Hat::new(Some(config), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    let mut config = settings(pty.path);
    config.open_hook = Some(Arc::new(|| panic!("injected opener panic")));
    let mut hat = Hat::new(Some(config), vec![0]);
    let error = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Err(error) = hat.changed().await {
                return error;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        error.contains("HAT helper failed"),
        "helper panic became an availability retry: {error}"
    );
}

#[tokio::test]
async fn blocked_open_is_terminal_and_prevents_a_false_clean_managed_stop() {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    let fixture = Fixture::new();
    let pty = Pty::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(AtomicBool::new(false));
    let entered = calls.clone();
    let resume = release.clone();
    let completed = ended.clone();
    let (service, handle) = fixture.service();
    let mut service = service.with_uart(pty.path, RepeatPolicy::Unverified);
    service.uart.as_mut().unwrap().open_hook = Some(Arc::new(move || {
        entered.fetch_add(1, Ordering::SeqCst);
        let until = std::time::Instant::now() + Duration::from_secs(15);
        while !resume.load(Ordering::SeqCst) {
            assert!(
                std::time::Instant::now() < until,
                "test blocked opener not released"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        completed.store(true, Ordering::SeqCst);
        Ok(())
    }));
    let running = start(service, &handle, &broker).await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Connecting)).await;
    wait_for(&handle, |s| {
        matches!(s.hardware, Hardware::Unavailable { .. })
    })
    .await;
    let terminal = matches!(
        handle.snapshot().unwrap().hardware,
        Hardware::Unavailable {
            retrying: false,
            ..
        }
    );
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("open failed".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let stopped = running.stop().await;
    release.store(true, Ordering::SeqCst);
    assert!(!stopped, "managed stop forgot a blocked serial opener");
    assert!(terminal, "timed-out opener remained eligible for retry");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "timed-out opener was retried"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ended.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn actual_transport_loss_reopens_and_measures_quiet_before_initialization() {
    let mut first = Pty::new();
    let mut second = Pty::new();
    let path = std::env::temp_dir().join(format!(
        "dali-serial-{}-{:032x}",
        std::process::id(),
        rand::random::<u128>()
    ));
    struct Link(PathBuf);
    impl Drop for Link {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Link(path.clone());
    std::os::unix::fs::symlink(&first.path, &path).unwrap();
    let mut hat = Hat::new(Some(settings(path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut first.peer).await, b"v\n");
    first.peer.write_all(b"V013204\n").await.unwrap();
    hat.changed().await.unwrap();
    assert_eq!(line(&mut first.peer).await, b"d\n");
    first.peer.write_all(b"D24\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Ready { .. })).await;
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&second.path, &path).unwrap();
    drop(first);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert!(
        !hat.reset,
        "quiet recovery retained the general reopen latch"
    );
    assert_eq!(line(&mut second.peer).await, b"v\n");
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn eof_during_partial_write_retains_poison_discovered_during_teardown() {
    let mut first = Pty::new();
    let mut second = Pty::new();
    let path = std::env::temp_dir().join(format!(
        "dali-partial-reopen-{:032x}",
        rand::random::<u128>()
    ));
    struct Link(PathBuf);
    impl Drop for Link {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Link(path.clone());
    std::os::unix::fs::symlink(&first.path, &path).unwrap();
    let mut serial = Serial::open(&first.path).await.unwrap();
    let (blocked, waiting) = tokio::sync::oneshot::channel();
    serial.limit_writes(1, blocked);
    let (link, events) = Transactions::start(serial, RepeatPolicy::Unverified);
    let original = link.clone();
    let status = link.observe_status();
    let mut config = settings(path.clone());
    config.timing.setup = Duration::from_secs(2);
    let mut hat = Hat::new(Some(config), vec![0]);
    hat.session = Some(Session {
        link,
        events,
        status,
    });
    hat.advance().unwrap();
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.peer.read_u8().await.unwrap(), b'v');
    assert!(
        !original.write_is_poisoned(),
        "write was already canceled before the EOF observation"
    );
    // Schedule the owner's EOF observation ahead of the actor's write-future
    // cancellation, the valid ordering which public events alone cannot cover.
    hat.event(Ok(Event::Closed(CloseReason::Eof))).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&second.path, &path).unwrap();
    drop(first);
    tokio::time::timeout(Duration::from_secs(2), async {
        while hat.session.is_some() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(original.status(), SessionState::Closed);
    assert!(
        original.write_is_poisoned(),
        "partial write did not produce authoritative poison during close"
    );
    assert!(
        hat.requires_boot,
        "poison discovered after EOF was lost at the close handoff"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while hat.boundary.is_none() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(hat.reset && hat.requires_boot);
    quiet(&mut second.peer).await;
    second.peer.write_all(b"B013204\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut second.peer).await, b"v\n");
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[test]
fn an_earlier_query_snapshot_cannot_replace_a_later_proactive_bus_observation() {
    let mut hat = Hat::new(Some(settings(PathBuf::from("unused"))), vec![0]);
    hat.observations.push(BusHealth {
        bus: 0,
        status: 0,
        channels: 4,
    });
    hat.finished(Done::Poll(Ok(vec![BusHealth {
        bus: 0,
        status: 2,
        channels: 4,
    }])))
    .unwrap();
    assert_eq!(
        hat.observations[0].status, 0,
        "old poll result replaced a newer proactive observation"
    );
    hat.reset = true;
    hat.requires_boot = true;
    hat.needs_boundary = true;
    hat.finished(Done::Boundary(Err(TransactionError::Deadline)))
        .unwrap();
    assert!(
        hat.reset && hat.needs_boundary,
        "failed receive boundary cleared reset protection"
    );
}

#[tokio::test]
async fn failed_boundary_retries_on_a_quiet_transport_without_new_events() {
    let pty = Pty::new();
    let mut hat = Hat::new(Some(settings(pty.path)), vec![0]);
    hat.reset = true;
    hat.requires_boot = true;
    tokio::time::timeout(Duration::from_secs(2), async {
        while hat.boundary.is_none() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    // Retire any watch notification: retry must be driven by the timer alone.
    hat.session.as_mut().unwrap().status.borrow_and_update();
    while hat.session.as_mut().unwrap().events.try_recv().is_ok() {}
    hat.boundary = None;
    hat.needs_boundary = true;
    hat.finished(Done::Boundary(Err(TransactionError::Deadline)))
        .unwrap();
    let retried = tokio::time::timeout(Duration::from_millis(300), async {
        while hat.boundary.is_none() {
            hat.changed().await.unwrap();
        }
    })
    .await;
    assert!(
        retried.is_ok(),
        "failed receive boundary had no timer to retry on a quiet transport"
    );
    assert!(hat.reset, "retry invented fresh boot evidence");
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn boot_and_transport_loss_cannot_hide_an_already_panicked_helper() {
    for transport in [false, true] {
        let pty = Pty::new();
        let serial = Serial::open(&pty.path).await.unwrap();
        let (link, events) = Transactions::start(serial, RepeatPolicy::Unverified);
        let status = link.observe_status();
        let mut hat = Hat::new(Some(settings(pty.path)), vec![0]);
        hat.session = Some(Session {
            link,
            events,
            status,
        });
        hat.spawn(JobKind::Query, async {
            panic!("injected completed helper panic")
        });
        while !hat.job.as_ref().unwrap().0.is_finished() {
            tokio::task::yield_now().await;
        }
        let event = if transport {
            Event::Closed(CloseReason::Io("injected loss".into()))
        } else {
            Event::Packet {
                sequence: 1,
                packet: Ok(crate::atx::wire::Packet {
                    bus: 0,
                    kind: Kind::Boot(Version {
                        hardware: "01".into(),
                        firmware: "32".into(),
                        variant: "04".into(),
                    }),
                }),
            }
        };
        let immediate = hat.event(Ok(event));
        let fatal = match immediate {
            Err(_) => true,
            Ok(()) => tokio::time::timeout(Duration::from_millis(150), async {
                loop {
                    if hat.changed().await.is_err() {
                        return;
                    }
                }
            })
            .await
            .is_ok(),
        };
        assert!(fatal, "hardware event hid an already-panicked helper");
    }
}

#[tokio::test]
async fn standalone_emulator_models_health_power_fault_and_reboot_without_fixture_commands() {
    use crate::emulator::{pty, session::Control};
    let fixture = Fixture::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let scenario =
        serde_json::from_str(include_str!("../../../examples/emulator/basic-i4.json")).unwrap();
    let emulator = pty::spawn(scenario).await.unwrap();
    let (service, handle) = fixture.service();
    let service = service.with_uart(emulator.path, RepeatPolicy::Unverified);
    let running = start(service, &handle, &broker).await;
    wait_for(&handle,|s|matches!(&s.hardware,Hardware::Ready {buses,..} if buses.iter().any(|b|b.bus==0&&b.status==2))).await;
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
    wait_for(&handle,|s|matches!(&s.hardware,Hardware::Ready {buses,..} if buses.iter().any(|b|b.bus==0&&b.status==0))).await;
    let mut updates = handle.subscribe();
    emulator
        .emulator
        .control(Control::Reboot, Duration::from_secs(1))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2),async {
        let mut restarting=false;
        loop {
            let snapshot=updates.recv().await.unwrap();
            if snapshot.hardware==Hardware::Initializing {restarting=true;}
            if restarting && matches!(&snapshot.hardware,Hardware::Ready {buses,..} if buses.iter().any(|b|b.bus==0&&b.status==0)) {break;}
        }
    }).await.expect("emulator reboot did not reinitialize the same UART owner");
    handle
        .edit(
            1,
            MetadataEdit::Description(Some("no fixture writes".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn metadata_and_actual_close_remain_available_during_unanswered_initialization() {
    let fixture = Fixture::new();
    let mut pty = Pty::new();
    let broker = mqtt_test_broker::FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let service = service.with_uart(pty.path.clone(), RepeatPolicy::Unverified);
    let running = start(service, &handle, &broker).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    let saved = handle
        .edit(
            1,
            MetadataEdit::Description(Some("initializing".into())),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(saved.phase, Phase::Ready);
    assert_eq!(saved.hardware, Hardware::Initializing);
    drop(pty._slave);
    assert!(running.stop().await);
    assert_eq!(handle.snapshot().unwrap().hardware, Hardware::Closed);
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_millis(300), pty.peer.read(&mut byte))
        .await
        .expect("canceled setup retained its serial owner");
    assert!(matches!(closed, Ok(0) | Err(_)));
}

#[tokio::test]
async fn fresh_boot_keeps_a_completed_receive_boundary_instead_of_requiring_another_reboot() {
    let mut pty = Pty::new();
    let serial = Serial::open(&pty.path).await.unwrap();
    let (link, events) = Transactions::start(serial, RepeatPolicy::Unverified);
    let status = link.observe_status();
    let mut hat = Hat::new(Some(settings(pty.path)), vec![0]);
    hat.session = Some(Session {
        link,
        events,
        status,
    });
    hat.reset = true;
    hat.requires_boot = true;
    hat.needs_boundary = true;
    // The no-write boundary completed before the new Boot became readable.
    hat.spawn(JobKind::Boundary, async { Done::Boundary(Ok(1)) });
    while !hat.job.as_ref().unwrap().0.is_finished() {
        tokio::task::yield_now().await;
    }
    pty.peer.write_all(b"B013204\n").await.unwrap();
    let event = tokio::time::timeout(
        Duration::from_secs(1),
        hat.session.as_mut().unwrap().events.recv(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(matches!(event, Event::Packet { sequence: 1, .. }));
    hat.event(Ok(event)).unwrap();
    let recovered = tokio::time::timeout(Duration::from_millis(300), async {
        while hat.reset {
            hat.changed().await.unwrap();
        }
    })
    .await;
    assert!(
        recovered.is_ok(),
        "fresh Boot discarded its already-completed receive boundary"
    );
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
}
