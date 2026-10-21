use super::*;
use crate::service::tests::{context, deadline, start, wait_for, Fixture};
use crate::service::{MetadataEdit, Phase};
use std::ffi::CStr;
use std::os::fd::{FromRawFd, OwnedFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Pty {
    peer: Serial,
    _slave: OwnedFd,
    path: PathBuf,
}
impl Pty {
    fn new() -> Self {
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
    settings.timing = Timing {
        setup: Duration::from_millis(80),
        poll: Duration::from_secs(60),
        retry: Duration::from_millis(20),
    };
    settings
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
async fn late_version_keeps_the_same_owner_and_blocks_queries_until_retired() {
    let mut pty = Pty::new();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    drive(&mut hat, |h| matches!(h, Hardware::WaitingLateReply { .. })).await;
    for _ in 0..3 {
        tokio::select! {_=hat.changed()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    }
    quiet(&mut pty.peer).await;
    pty.peer.write_all(b"V013204\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
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
async fn reset_required_waits_for_actor_accepted_boot_without_reopening() {
    let mut pty = Pty::new();
    let mut hat = Hat::new(Some(settings(pty.path.clone())), vec![0]);
    drive(&mut hat, |h| matches!(h, Hardware::Initializing)).await;
    assert_eq!(line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"invalid\n").await.unwrap();
    drive(&mut hat, |h| matches!(h, Hardware::ResetRequired)).await;
    let original = hat.session.as_ref().unwrap().link.clone();
    assert_eq!(
        original
            .fresh_boot_boundary(Duration::from_millis(50))
            .await,
        Err(TransactionError::Desynchronized),
        "receive boundary admitted a poisoned session"
    );
    // Status observations and a late V do not prove a reset.
    pty.peer.write_all(b"D24\nV013204\n").await.unwrap();
    for _ in 0..5 {
        tokio::select! {_=hat.changed()=>{},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    }
    assert_eq!(original.status(), SessionState::ResetRequired);
    quiet(&mut pty.peer).await;
    pty.peer.write_all(b"B013204\n").await.unwrap();
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
async fn actual_transport_loss_reopens_only_for_monitoring_until_fresh_boot() {
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
    tokio::time::timeout(Duration::from_secs(2), async {
        while hat.boundary.is_none() {
            hat.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(
        hat.reset,
        "transport reopen cleared unresolved response ordering"
    );
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
