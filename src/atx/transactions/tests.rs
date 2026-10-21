use super::*;
use crate::atx::monitor::Event;
use crate::atx::wire::{Command, DaliFrame};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::broadcast;
use tokio::time::timeout;

fn query() -> Command {
    Command::Send {
        bus: 0,
        frame: DaliFrame::Bits16(0x0190),
        repeat: false,
    }
}

#[tokio::test]
async fn verified_repeat_profiles_consume_their_declared_terminal_count() {
    for (policy, wire, replies) in [
        (
            RepeatPolicy::OneTerminal,
            b"N\n".as_slice(),
            vec![BusReply::NoResponse],
        ),
        (
            RepeatPolicy::TwoTerminals,
            b"N\nN\n".as_slice(),
            vec![BusReply::NoResponse, BusReply::NoResponse],
        ),
    ] {
        let (link, mut hat, _) = fixture(policy).await;
        let task = request(
            &link,
            Command::Send {
                bus: 0,
                frame: DaliFrame::Bits16(0x01A3),
                repeat: true,
            },
            Duration::from_secs(1),
        );
        assert_eq!(line(&mut hat).await, b"t01A3\n");
        hat.write_all(wire).await.unwrap();
        assert_eq!(task.await.unwrap().unwrap(), Response::Dali(replies));
        state(&link, SessionState::Ready).await;
        link.close().await.unwrap();
    }
}

async fn line(hat: &mut DuplexStream) -> Vec<u8> {
    timeout(Duration::from_secs(1), async {
        let mut output = Vec::new();
        for _ in 0..32 {
            let byte = hat.read_u8().await.unwrap();
            output.push(byte);
            if byte == b'\n' {
                return output;
            }
        }
        panic!("outgoing command line was not bounded");
    })
    .await
    .expect("HAT command was not written")
}

async fn state(link: &Transactions<DuplexStream>, expected: SessionState) {
    let mut status = link.observe_status();
    timeout(
        Duration::from_secs(1),
        status.wait_for(|value| *value == expected),
    )
    .await
    .unwrap_or_else(|_| panic!("expected {expected:?}, got {:?}", link.status()))
    .unwrap();
}

async fn initialize(link: &Transactions<DuplexStream>, hat: &mut DuplexStream) {
    let clone = link.clone();
    let init = tokio::spawn(async move { clone.initialize(Duration::from_secs(1)).await });
    assert_eq!(line(hat).await, b"v\n");
    hat.write_all(b"V013204\n").await.unwrap();
    assert_eq!(init.await.unwrap().unwrap().firmware, "32");
    state(link, SessionState::Ready).await;
}

async fn fixture(
    repeat: RepeatPolicy,
) -> (
    Transactions<DuplexStream>,
    DuplexStream,
    broadcast::Receiver<Event>,
) {
    let (stream, mut hat) = tokio::io::duplex(4096);
    let (link, events) = Transactions::start(stream, repeat);
    assert_eq!(
        link.execute(query(), Duration::from_secs(1)).await,
        Err(TransactionError::NeedsInitialization)
    );
    initialize(&link, &mut hat).await;
    (link, hat, events)
}

fn request(
    link: &Transactions<DuplexStream>,
    command: Command,
    within: Duration,
) -> tokio::task::JoinHandle<Result<Response, TransactionError>> {
    let link = link.clone();
    tokio::spawn(async move { link.execute(command, within).await })
}

#[tokio::test]
async fn explicit_initialization_and_one_owner_separate_async_events_from_replies() {
    let (link, mut hat, mut events) = fixture(RepeatPolicy::Unverified).await;
    let task = request(&link, query(), Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"h0190\n");
    hat.write_all(b"2J66\nD24\nH0190\nL020401\nJ06\n")
        .await
        .unwrap();
    assert_eq!(
        task.await.unwrap().unwrap(),
        Response::Dali(vec![BusReply::Value(6)])
    );
    let mut input_seen = false;
    while let Ok(event) = events.try_recv() {
        if matches!(
            event,
            Event::Packet {
                packet: Ok(super::super::wire::Packet {
                    kind: super::super::wire::Kind::Dali(DaliFrame::Bits24(0x020401)),
                    ..
                }),
                ..
            }
        ) {
            input_seen = true;
        }
    }
    assert!(
        input_seen,
        "async input frame disappeared into command response handling"
    );
    link.close().await.unwrap();
}

#[tokio::test]
async fn a_timed_out_exchange_keeps_its_slot_until_the_late_terminal_is_consumed() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let task = request(&link, query(), Duration::from_millis(50));
    line(&mut hat).await;
    assert_eq!(task.await.unwrap(), Err(TransactionError::Deadline));
    state(&link, SessionState::WaitingLateReply { remaining: 1 }).await;
    assert_eq!(
        link.execute(query(), Duration::from_secs(1)).await,
        Err(TransactionError::Desynchronized)
    );
    hat.write_all(b"J77\n").await.unwrap();
    state(&link, SessionState::Ready).await;
    let next = request(&link, query(), Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"h0190\n");
    hat.write_all(b"N\n").await.unwrap();
    assert_eq!(
        next.await.unwrap().unwrap(),
        Response::Dali(vec![BusReply::NoResponse])
    );
    link.close().await.unwrap();
}

#[tokio::test]
async fn canceling_after_one_of_two_terminals_keeps_exactly_one_outstanding() {
    let (link, mut hat, _) = fixture(RepeatPolicy::TwoTerminals).await;
    let command = Command::Send {
        bus: 0,
        frame: DaliFrame::Bits16(0xa500),
        repeat: true,
    };
    let task = request(&link, command, Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"tA500\n");
    hat.write_all(b"N\n").await.unwrap();
    state(&link, SessionState::Awaiting { remaining: 1 }).await;
    task.abort();
    let _ = task.await;
    state(&link, SessionState::WaitingLateReply { remaining: 1 }).await;
    hat.write_all(b"N\n").await.unwrap();
    state(&link, SessionState::Ready).await;
    let next = request(&link, query(), Duration::from_secs(1));
    line(&mut hat).await;
    hat.write_all(b"J09\n").await.unwrap();
    assert_eq!(
        next.await.unwrap().unwrap(),
        Response::Dali(vec![BusReply::Value(9)])
    );
    link.close().await.unwrap();
}

#[tokio::test]
async fn interference_cannot_be_repaired_by_an_external_or_late_original_terminal() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let task = request(&link, query(), Duration::from_secs(1));
    line(&mut hat).await;
    state(&link, SessionState::Awaiting { remaining: 1 }).await;
    hat.write_all(b"H0290\nJ55\nN\n").await.unwrap();
    assert_eq!(task.await.unwrap(), Err(TransactionError::Interference));
    state(&link, SessionState::ResetRequired).await;
    assert_eq!(
        link.execute(query(), Duration::from_secs(1)).await,
        Err(TransactionError::Desynchronized)
    );
    hat.write_all(b"B013204\n").await.unwrap();
    state(&link, SessionState::Uninitialized).await;
    initialize(&link, &mut hat).await;
    link.close().await.unwrap();
}

#[tokio::test]
async fn old_buffered_and_split_terminals_do_not_satisfy_a_new_command() {
    for split in [false, true] {
        let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
        hat.write_all(if split { b"J0" } else { b"JEE\n" })
            .await
            .unwrap();
        let task = request(&link, query(), Duration::from_secs(1));
        line(&mut hat).await;
        hat.write_all(if split { b"1\nJ06\n" } else { b"J06\n" })
            .await
            .unwrap();
        assert_eq!(
            task.await.unwrap().unwrap(),
            Response::Dali(vec![BusReply::Value(6)])
        );
        link.close().await.unwrap();
    }
}

#[tokio::test]
async fn busy_admission_and_unverified_modes_never_add_a_write() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    for (frame, repeat) in [
        (DaliFrame::Bits8(6), false),
        (DaliFrame::Bits16(0xa500), true),
    ] {
        assert_eq!(
            link.execute(
                Command::Send {
                    bus: 0,
                    frame,
                    repeat
                },
                Duration::from_secs(1)
            )
            .await,
            Err(TransactionError::UnsupportedMode)
        );
    }
    let task = request(&link, query(), Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"h0190\n");
    assert_eq!(
        link.execute(Command::Version, Duration::from_secs(1)).await,
        Err(TransactionError::Busy)
    );
    hat.write_all(b"N\n").await.unwrap();
    task.await.unwrap().unwrap();
    link.close().await.unwrap();
}

#[tokio::test]
async fn corruption_blank_collision_or_internal_lag_fail_before_a_buffered_terminal_can_succeed() {
    for burst in [
        b"Jxx\nJ01\n".to_vec(),
        b"\nJ01\n".to_vec(),
        b"N\n".repeat(300),
    ] {
        let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
        let task = request(&link, query(), Duration::from_secs(1));
        line(&mut hat).await;
        state(&link, SessionState::Awaiting { remaining: 1 }).await;
        hat.write_all(&burst).await.unwrap();
        assert_eq!(task.await.unwrap(), Err(TransactionError::StreamLoss));
        state(&link, SessionState::ResetRequired).await;
        link.close().await.unwrap();
    }
}

#[tokio::test]
async fn closing_resolves_an_active_request_and_releases_the_transport() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let task = request(&link, query(), Duration::from_secs(1));
    line(&mut hat).await;
    link.close().await.unwrap();
    assert_eq!(task.await.unwrap(), Err(TransactionError::Closed));
    assert_eq!(link.status(), SessionState::Closed);
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_secs(1), hat.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn a_terminal_during_a_partial_write_cannot_report_success() {
    let (stream, mut hat) = tokio::io::duplex(1);
    let (link, _) = Transactions::start(stream, RepeatPolicy::Unverified);
    initialize(&link, &mut hat).await;
    let task = request(&link, query(), Duration::from_millis(80));
    assert_eq!(
        timeout(Duration::from_secs(1), hat.read_u8())
            .await
            .unwrap()
            .unwrap(),
        b'h'
    );
    hat.write_all(b"J06\n").await.unwrap();
    let result = task.await.unwrap();
    assert!(
        result.is_err(),
        "an apparent reply made a partial write successful: {result:?}"
    );
    state(&link, SessionState::ResetRequired).await;
    link.close().await.unwrap();
}

#[tokio::test]
async fn a_boot_during_partial_write_cancels_it_and_requires_a_later_boot_to_rearm() {
    let (stream, mut hat) = tokio::io::duplex(1);
    let (link, _) = Transactions::start(stream, RepeatPolicy::Unverified);
    initialize(&link, &mut hat).await;
    let task = request(&link, query(), Duration::from_secs(1));
    assert_eq!(
        timeout(Duration::from_secs(1), hat.read_u8())
            .await
            .unwrap()
            .unwrap(),
        b'h'
    );
    hat.write_all(b"B013204\n").await.unwrap();
    assert_eq!(task.await.unwrap(), Err(TransactionError::Reboot));
    state(&link, SessionState::ResetRequired).await;
    assert_eq!(
        link.initialize(Duration::from_secs(1)).await,
        Err(TransactionError::Desynchronized)
    );
    // Discard bytes already accepted before the boot. There must be no resumed
    // suffix/newline; the emulated HAT reset drops its partial command buffer.
    let mut pending = [0; 32];
    let count = timeout(Duration::from_millis(50), hat.read(&mut pending))
        .await
        .unwrap()
        .unwrap();
    assert!(
        !pending[..count].contains(&b'\n'),
        "write resumed after boot"
    );
    assert!(
        timeout(Duration::from_millis(30), hat.read(&mut pending))
            .await
            .is_err(),
        "canceled writer resumed after draining its accepted prefix"
    );
    hat.write_all(b"B013204\n").await.unwrap();
    state(&link, SessionState::Uninitialized).await;
    initialize(&link, &mut hat).await; // Also proves persistent write poison cleared.
    link.close().await.unwrap();
}

#[tokio::test]
async fn a_boot_completing_an_old_partial_line_is_still_a_safety_event() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    hat.write_all(b"B0132").await.unwrap();
    let task = request(&link, query(), Duration::from_secs(1));
    line(&mut hat).await;
    state(&link, SessionState::Awaiting { remaining: 1 }).await;
    hat.write_all(b"04\nJ06\n").await.unwrap();
    assert_eq!(task.await.unwrap(), Err(TransactionError::Reboot));
    state(&link, SessionState::Uninitialized).await;
    link.close().await.unwrap();
}

#[tokio::test]
async fn timed_out_or_canceled_initialization_does_not_use_a_new_version_as_a_barrier() {
    for cancel in [false, true] {
        let (stream, mut hat) = tokio::io::duplex(1024);
        let (link, _) = Transactions::start(stream, RepeatPolicy::Unverified);
        let clone = link.clone();
        let task = tokio::spawn(async move { clone.initialize(Duration::from_millis(80)).await });
        assert_eq!(line(&mut hat).await, b"v\n");
        state(&link, SessionState::Awaiting { remaining: 1 }).await;
        if cancel {
            task.abort();
            let _ = task.await;
        } else {
            assert_eq!(task.await.unwrap(), Err(TransactionError::Deadline));
        }
        state(&link, SessionState::WaitingLateReply { remaining: 1 }).await;
        assert_eq!(
            link.initialize(Duration::from_secs(1)).await,
            Err(TransactionError::Desynchronized)
        );
        hat.write_all(b"V013204\n").await.unwrap();
        state(&link, SessionState::Uninitialized).await;
        initialize(&link, &mut hat).await;
        link.close().await.unwrap();
    }
}

#[tokio::test]
async fn canceled_queued_requests_write_nothing_and_release_admission() {
    use std::future::{poll_fn, Future};
    use std::task::Poll;
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let mut status = link.observe_status();
    {
        let future = link.execute(query(), Duration::from_secs(1));
        tokio::pin!(future);
        poll_fn(|context| match future.as_mut().poll(context) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(result) => panic!("request completed before actor ran: {result:?}"),
        })
        .await;
    } // Drop the receiver before the actor gets to poll its admitted request.
    timeout(Duration::from_secs(1), status.changed())
        .await
        .unwrap()
        .unwrap();
    let mut byte = [0];
    assert!(
        timeout(Duration::from_millis(30), hat.read(&mut byte))
            .await
            .is_err(),
        "canceled queued request wrote to the UART"
    );
    let task = request(&link, query(), Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"h0190\n");
    hat.write_all(b"N\n").await.unwrap();
    task.await.unwrap().unwrap();
    link.close().await.unwrap();
}

#[tokio::test]
async fn status_observations_and_collision_codes_keep_their_bus_and_meaning() {
    let (link, mut hat, _) = fixture(RepeatPolicy::OneTerminal).await;
    let task = request(&link, Command::BusStatus { bus: 2 }, Duration::from_secs(1));
    assert_eq!(line(&mut hat).await, b"2d\n");
    hat.write_all(b"D01\n2D24\n").await.unwrap();
    assert_eq!(
        task.await.unwrap().unwrap(),
        Response::Status {
            status: 2,
            channels: 4
        }
    );
    for (wire, expected) in [
        (b"X4\n".as_slice(), BusReply::ReceiveCollision(Some(4))),
        (b"Z02\n".as_slice(), BusReply::TransmitCollision(Some(2))),
    ] {
        let task = request(&link, query(), Duration::from_secs(1));
        line(&mut hat).await;
        hat.write_all(wire).await.unwrap();
        assert_eq!(task.await.unwrap().unwrap(), Response::Dali(vec![expected]));
    }
    link.close().await.unwrap();
}

#[tokio::test]
async fn idle_input_traffic_is_drained_and_idle_reboots_invalidate_initialization() {
    let (link, mut hat, mut events) = fixture(RepeatPolicy::Unverified).await;
    for _ in 0..300 {
        hat.write_all(b"L020401\n").await.unwrap();
        timeout(Duration::from_secs(1), async {
            loop {
                if matches!(
                    events.recv().await.unwrap(),
                    Event::Packet {
                        packet: Ok(super::super::wire::Packet {
                            kind: super::super::wire::Kind::Dali(DaliFrame::Bits24(_)),
                            ..
                        }),
                        ..
                    }
                ) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    assert_eq!(link.status(), SessionState::Ready);
    hat.write_all(b"B013204\n").await.unwrap();
    state(&link, SessionState::Uninitialized).await;
    assert_eq!(
        link.execute(query(), Duration::from_secs(1)).await,
        Err(TransactionError::NeedsInitialization)
    );
    link.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_ready_terminal_at_expiry_is_retired_without_reporting_success() {
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let task = request(&link, query(), Duration::from_millis(100));
    line(&mut hat).await;
    state(&link, SessionState::Awaiting { remaining: 1 }).await;
    hat.write_all(b"J06\n").await.unwrap();
    tokio::time::advance(Duration::from_millis(100)).await;
    assert_eq!(
        task.await.unwrap(),
        Err(TransactionError::Deadline),
        "late ready terminal was reported as success"
    );
    state(&link, SessionState::Ready).await;
    link.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_caller_resuming_after_expiry_cannot_accept_an_already_ready_success() {
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    let (link, mut hat, _) = fixture(RepeatPolicy::Unverified).await;
    let future = link.execute(query(), Duration::from_millis(100));
    tokio::pin!(future);
    poll_fn(|context| match future.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(result) => panic!("unexpected early completion: {result:?}"),
    })
    .await;
    line(&mut hat).await;
    state(&link, SessionState::Awaiting { remaining: 1 }).await;
    hat.write_all(b"J06\n").await.unwrap();
    state(&link, SessionState::Ready).await;
    tokio::time::advance(Duration::from_millis(100)).await;
    assert_eq!(
        future.await,
        Err(TransactionError::Deadline),
        "ready oneshot bypassed the caller's deadline"
    );
    link.close().await.unwrap();
}
