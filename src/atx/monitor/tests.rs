use super::super::wire::{DaliFrame, Kind};
use super::*;
use crate::atx::test_support::HeldDrain;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

async fn next(events: &mut broadcast::Receiver<Event>) -> Event {
    timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("continuous HAT reader stopped delivering events")
        .unwrap()
}

struct HeldWrite {
    stream: tokio::io::DuplexStream,
    ready: Arc<AtomicBool>,
    entered: Option<oneshot::Sender<()>>,
}
impl AsyncRead for HeldWrite {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
impl DrainRead for HeldWrite {
    fn poll_drain(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<Drain>> {
        Pin::new(&mut self.stream).poll_drain(cx, buf)
    }
}
impl AsyncWrite for HeldWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if !self.ready.load(Ordering::Acquire) {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[tokio::test(start_paused = true)]
async fn a_transport_becoming_writable_after_expiry_never_accepts_new_bytes() {
    let (stream, mut peer) = tokio::io::duplex(128);
    let ready = Arc::new(AtomicBool::new(false));
    let (entered, waiting) = oneshot::channel();
    let (monitor, _) = Monitor::start(HeldWrite {
        stream,
        ready: ready.clone(),
        entered: Some(entered),
    });
    let deadline = tokio::time::Instant::now() + Duration::from_millis(10);
    let command = Command::Version;
    let mut writing = Box::pin(monitor.write_correlated_until(
        &command,
        deadline,
        Arc::new(AtomicU64::new(u64::MAX)),
    ));
    // Do not poll the write again until BOTH the clock and transport readiness
    // have changed; timer scheduling cannot rescue a missing per-poll guard.
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(
            writing.as_mut(),
            cx
        )))
        .await
        .is_pending()
    );
    waiting.await.unwrap();
    tokio::time::advance(Duration::from_millis(20)).await;
    ready.store(true, Ordering::Release);
    assert_eq!(
        writing.await,
        Err(TransportError::Deadline),
        "writable transport accepted a command after its original deadline"
    );
    assert!(
        timeout(Duration::from_millis(1), peer.read_u8())
            .await
            .is_err(),
        "expired transport wrote bytes"
    );
    monitor.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn quiet_fence_measures_silence_and_restarts_on_partial_raw_input() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, _) = Monitor::start(stream);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    let mut recovery = std::pin::pin!(monitor.quiet_fence(Duration::from_millis(250), deadline));
    for _ in 0..2 {
        tokio::select! {
            biased;
            result = &mut recovery => panic!("quiet fence completed before the interval: {result:?}"),
            _ = tokio::task::yield_now() => {},
        }
    }
    tokio::time::advance(Duration::from_millis(200)).await;
    hat.write_all(b"L0204").await.unwrap();
    monitor.fence(Duration::from_secs(1)).await.unwrap();
    tokio::time::advance(Duration::from_millis(249)).await;
    tokio::select! {
        biased;
        result = &mut recovery => panic!("raw partial bytes failed to restart quiet interval: {result:?}"),
        _ = tokio::task::yield_now() => {},
    }
    tokio::time::advance(Duration::from_millis(1)).await;
    recovery.await.unwrap();
    // Recovery resets only the old incomplete host framing. A subsequent
    // complete event must parse, rather than joining the abandoned prefix.
    let mut events = monitor.subscribe();
    hat.write_all(b"L020401\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            packet: Ok(Packet {
                kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                ..
            }),
            ..
        }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn quiet_fence_keeps_complete_input_events_visible() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"L020401\n").await.unwrap();
    let until = tokio::time::Instant::now() + Duration::from_secs(1);
    let fence = monitor
        .quiet_fence(Duration::from_millis(250), until)
        .await
        .unwrap();
    assert_eq!(fence.sequence, 1);
    assert!(fence.activity_generation > 0);
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            packet: Ok(Packet {
                kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                ..
            }),
            ..
        }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn quiet_fence_expires_under_continuous_input_without_hiding_events() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    let owner = monitor.clone();
    let recovery = tokio::spawn(async move {
        owner
            .quiet_fence(
                Duration::from_millis(100),
                tokio::time::Instant::now() + Duration::from_millis(200),
            )
            .await
    });
    tokio::task::yield_now().await;
    for _ in 0..4 {
        hat.write_all(b"L020401\n").await.unwrap();
        assert!(matches!(
            next(&mut events).await,
            Event::Packet { packet: Ok(_), .. }
        ));
        tokio::time::advance(Duration::from_millis(50)).await;
    }
    assert!(matches!(
        recovery.await.unwrap(),
        Err(TransportError::Deadline)
    ));
    monitor.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_delayed_canceled_or_expired_quiet_reset_cannot_discard_later_framing() {
    for expire in [false, true] {
        let (stream, mut hat) = tokio::io::duplex(1024);
        let held = Arc::new(AtomicBool::new(false));
        let (entered, waiting) = oneshot::channel();
        let (monitor, mut events) = Monitor::start(HeldDrain {
            stream,
            held: held.clone(),
            entered: Some(entered),
        });
        hat.write_all(b"L0204").await.unwrap();
        monitor.fence(Duration::from_secs(1)).await.unwrap();
        held.store(true, Ordering::Release);
        let owner = monitor.clone();
        let recovering = tokio::spawn(async move {
            owner
                .quiet_fence(
                    Duration::from_millis(20),
                    tokio::time::Instant::now() + Duration::from_millis(80),
                )
                .await
        });
        waiting.await.unwrap();
        if expire {
            tokio::time::advance(Duration::from_millis(80)).await;
            assert!(matches!(
                recovering.await.unwrap(),
                Err(TransportError::Deadline)
            ));
        } else {
            recovering.abort();
            assert!(recovering.await.unwrap_err().is_cancelled());
        }
        held.store(false, Ordering::Release);
        // This queues after the held request and confirms its reader execution
        // has completed before the later suffix is supplied.
        monitor.fence(Duration::from_secs(1)).await.unwrap();
        hat.write_all(b"01\n").await.unwrap();
        assert!(
            matches!(
                next(&mut events).await,
                Event::Packet {
                    packet: Ok(Packet {
                        kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                        ..
                    }),
                    ..
                }
            ),
            "old canceled/expired request discarded framing"
        );
        monitor.close().await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn new_bytes_during_a_quiet_fence_invalidate_its_reset_candidate() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let held = Arc::new(AtomicBool::new(true));
    let (entered, waiting) = oneshot::channel();
    let (monitor, mut events) = Monitor::start(HeldDrain {
        stream,
        held: held.clone(),
        entered: Some(entered),
    });
    let owner = monitor.clone();
    let recovering = tokio::spawn(async move {
        owner
            .quiet_fence(
                Duration::from_millis(20),
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await
    });
    waiting.await.unwrap();
    hat.write_all(b"L0204").await.unwrap();
    held.store(false, Ordering::Release);
    monitor.fence(Duration::from_secs(1)).await.unwrap();
    assert!(monitor.activity_generation() > 0);
    assert!(
        !recovering.is_finished(),
        "stale quiet candidate accepted new raw activity"
    );
    hat.write_all(b"01\n").await.unwrap();
    assert!(
        matches!(
            next(&mut events).await,
            Event::Packet {
                packet: Ok(Packet {
                    kind: Kind::Dali(DaliFrame::Bits24(0x020401)),
                    ..
                }),
                ..
            }
        ),
        "stale quiet candidate discarded the new partial record"
    );
    recovering.await.unwrap().unwrap();
    monitor.close().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn quiet_fence_reports_eof_and_never_rearms_partial_write_poison() {
    let (stream, mut hat) = tokio::io::duplex(1);
    let (monitor, _) = Monitor::start(stream);
    assert!(monitor
        .write(&Command::Version, Duration::from_millis(20))
        .await
        .is_err());
    assert!(monitor.write_is_poisoned());
    let result = monitor
        .quiet_fence(
            Duration::from_millis(250),
            tokio::time::Instant::now() + Duration::from_secs(1),
        )
        .await;
    assert!(matches!(result, Err(TransportError::IndeterminateWrite)));
    assert!(monitor.write_is_poisoned());
    // Consume the accepted prefix; closure must wake quiet acquisition instead
    // of requiring its full interval or inventing a framing reset.
    assert_eq!(hat.read_u8().await.unwrap(), b'v');
    monitor.close().await.unwrap();
    let (stream, hat) = tokio::io::duplex(1024);
    let (monitor, _) = Monitor::start(stream);
    let mut quiet = std::pin::pin!(monitor.quiet_fence(
        Duration::from_secs(5),
        tokio::time::Instant::now() + Duration::from_secs(6)
    ));
    tokio::select! { biased; result=&mut quiet=>panic!("unexpected quiet completion: {result:?}"), _=tokio::task::yield_now()=>{} }
    let started = tokio::time::Instant::now();
    drop(hat);
    assert!(matches!(quiet.await, Err(TransportError::Closed)));
    assert_eq!(
        started,
        tokio::time::Instant::now(),
        "EOF waited for the quiet timer"
    );
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn idle_events_and_interleaved_bus_traffic_arrive_without_any_command() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"1L010203\r\n3J7F\n2D24\nN\n").await.unwrap();
    for (sequence, bus, kind) in [
        (1, 1, Kind::Dali(DaliFrame::Bits24(0x010203))),
        (2, 3, Kind::Dali(DaliFrame::Bits8(0x7f))),
        (
            3,
            2,
            Kind::BusStatus {
                status: 2,
                channels: 4,
            },
        ),
        (4, 0, Kind::NoResponse),
    ] {
        assert_eq!(
            next(&mut events).await,
            Event::Packet {
                sequence,
                packet: Ok(Packet { bus, kind })
            }
        );
    }
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn writes_are_bytes_only_and_do_not_consume_async_or_reply_packets() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    let command = Command::Send {
        bus: 2,
        frame: DaliFrame::Bits16(0x0190),
        repeat: false,
    };
    let receipt = monitor
        .write(&command, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(receipt.bytes_accepted, 7);
    let mut bytes = [0; 7];
    timeout(Duration::from_secs(1), hat.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"2h0190\n");
    hat.write_all(b"L010203\n2J06\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            packet: Ok(Packet {
                kind: Kind::Dali(DaliFrame::Bits24(_)),
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            packet: Ok(Packet {
                kind: Kind::Dali(DaliFrame::Bits8(6)),
                ..
            }),
            ..
        }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn a_stalled_or_canceled_partial_write_poison_admission_but_monitoring_continues() {
    for cancel in [false, true] {
        let (stream, mut hat) = tokio::io::duplex(1);
        let (monitor, mut events) = Monitor::start(stream);
        let command = Command::Send {
            bus: 0,
            frame: DaliFrame::Bits16(0xfefe),
            repeat: false,
        };
        if cancel {
            assert!(timeout(
                Duration::from_millis(50),
                monitor.write(&command, Duration::from_secs(1))
            )
            .await
            .is_err());
        } else {
            assert_eq!(
                monitor.write(&command, Duration::from_millis(50)).await,
                Err(TransportError::Deadline)
            );
        }
        assert_eq!(next(&mut events).await, Event::WriteIndeterminate);
        assert_eq!(
            monitor
                .write(&Command::Version, Duration::from_millis(50))
                .await,
            Err(TransportError::IndeterminateWrite)
        );
        hat.write_all(b"L010203\n").await.unwrap();
        assert!(matches!(
            next(&mut events).await,
            Event::Packet {
                packet: Ok(Packet {
                    kind: Kind::Dali(DaliFrame::Bits24(_)),
                    ..
                }),
                ..
            }
        ));
        monitor.close().await.unwrap();
    }
}

#[tokio::test]
async fn subscriber_lag_is_explicit_and_reader_does_not_wait_for_a_subscriber() {
    let (stream, mut hat) = tokio::io::duplex(4096);
    let (monitor, mut events) = Monitor::start(stream);
    let bytes = b"N\n".repeat(300);
    hat.write_all(&bytes).await.unwrap();
    drop(hat);
    // EOF is emitted after all packet events; receiving it on a fresh observer
    // is unnecessary: sequence300 proves the reader passed the whole burst.
    let mut lagged = false;
    let mut last = 0;
    timeout(Duration::from_secs(2), async {
        loop {
            match events.recv().await {
                Err(broadcast::error::RecvError::Lagged(_)) => lagged = true,
                Ok(Event::Packet { sequence, .. }) => last = sequence,
                Ok(Event::Closed(CloseReason::Eof)) => break,
                other => panic!("unexpected monitor result: {other:?}"),
            }
        }
    })
    .await
    .expect("reader waited for lagging consumer");
    assert!(lagged, "lost events were not explicitly reported");
    assert_eq!(last, 300);
    assert_eq!(
        monitor
            .write(&Command::Version, Duration::from_secs(1))
            .await,
        Err(TransportError::Closed)
    );
}

#[tokio::test]
async fn malformed_lines_are_reported_and_closing_an_idle_reader_is_bounded() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"Jxx\nL010203\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { packet: Err(_), .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { packet: Ok(_), .. }
    ));
    timeout(Duration::from_secs(1), monitor.close())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next(&mut events).await, Event::Closed(CloseReason::Stopped));
    assert_eq!(
        monitor
            .write(&Command::Version, Duration::from_secs(1))
            .await,
        Err(TransportError::Closed)
    );
}

#[tokio::test]
async fn close_also_cancels_an_in_progress_write() {
    let (stream, mut hat) = tokio::io::duplex(1);
    let (monitor, _events) = Monitor::start(stream);
    let writer = monitor.clone();
    let write = tokio::spawn(async move {
        writer
            .write(
                &Command::Send {
                    bus: 0,
                    frame: DaliFrame::Bits16(0xfefe),
                    repeat: false,
                },
                Duration::from_secs(2),
            )
            .await
    });
    let mut first_byte = [0; 1];
    timeout(Duration::from_secs(1), hat.read_exact(&mut first_byte))
        .await
        .unwrap()
        .unwrap();
    // One byte is read and one can remain buffered; the six-byte command is
    // still incomplete. Close must cancel, not await the write deadline.
    monitor
        .close()
        .await
        .expect("close left an in-progress writer running");
    let result = timeout(Duration::from_millis(100), write)
        .await
        .expect("close left an in-progress writer running")
        .unwrap();
    assert_eq!(result, Err(TransportError::Closed));
}

#[tokio::test]
async fn a_partial_idle_line_expires_without_needing_another_byte() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"L01").await.unwrap();
    assert_eq!(
        next(&mut events).await,
        Event::Packet {
            sequence: 1,
            packet: Err(DecodeError::LineDeadline)
        }
    );
    hat.write_all(b"0203\nJ01\n").await.unwrap();
    assert_eq!(
        next(&mut events).await,
        Event::Packet {
            sequence: 2,
            packet: Ok(Packet {
                bus: 0,
                kind: Kind::Dali(DaliFrame::Bits8(1))
            })
        }
    );
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn cancellation_before_the_writer_lock_does_not_poison_an_unwritten_command() {
    let (stream, _hat) = tokio::io::duplex(1024);
    let (monitor, _events) = Monitor::start(stream);
    let lock = monitor.inner.writer.lock().await;
    assert!(timeout(
        Duration::from_millis(50),
        monitor.write(&Command::Version, Duration::from_secs(1))
    )
    .await
    .is_err());
    drop(lock);
    assert_eq!(
        monitor
            .write(&Command::Version, Duration::from_secs(1))
            .await
            .unwrap()
            .bytes_accepted,
        2
    );
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn dropping_the_last_handle_stops_the_reader_but_dropping_a_clone_does_not() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    drop(monitor.clone());
    hat.write_all(b"N\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { packet: Ok(_), .. }
    ));
    drop(monitor);
    assert_eq!(next(&mut events).await, Event::Closed(CloseReason::Stopped));
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_millis(100), hat.read(&mut byte))
            .await
            .expect("last handle drop retained the stream")
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn close_releases_the_stream_even_when_monitor_handles_are_retained() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, _events) = Monitor::start(stream);
    let retained = monitor.clone();
    monitor.close().await.unwrap();
    let _late = retained.subscribe();
    assert_eq!(retained.status(), LinkStatus::Closed);
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_millis(100), hat.read(&mut byte))
            .await
            .expect("closed monitor retained the underlying stream")
            .unwrap(),
        0
    );
    assert_eq!(
        retained
            .write(&Command::Version, Duration::from_secs(1))
            .await,
        Err(TransportError::Closed)
    );
}

#[tokio::test]
async fn concurrent_writers_keep_each_encoded_line_whole() {
    let (stream, mut hat) = tokio::io::duplex(1);
    let (monitor, _events) = Monitor::start(stream);
    let mut tasks = tokio::task::JoinSet::new();
    for value in 0..10 {
        let writer = monitor.clone();
        tasks.spawn(async move {
            writer
                .write(
                    &Command::Send {
                        bus: 0,
                        frame: DaliFrame::Bits16(value),
                        repeat: false,
                    },
                    Duration::from_secs(2),
                )
                .await
        });
    }
    let mut bytes = [0; 60];
    timeout(Duration::from_secs(2), hat.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    let mut lines: Vec<_> = bytes
        .chunks_exact(6)
        .map(|line| String::from_utf8(line.to_vec()).unwrap())
        .collect();
    lines.sort();
    assert_eq!(
        lines,
        (0..10)
            .map(|value| format!("h{value:04X}\n"))
            .collect::<Vec<_>>()
    );
    while let Some(result) = timeout(Duration::from_secs(2), tasks.join_next())
        .await
        .unwrap()
    {
        assert_eq!(result.unwrap().unwrap().bytes_accepted, 6);
    }
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn reader_panic_is_visible_and_closes_write_admission() {
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };
    struct PanicStream;
    impl crate::atx::stream::DrainRead for PanicStream {
        fn poll_drain(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<crate::atx::stream::Drain>> {
            panic!("injected drain panic");
        }
    }
    impl tokio::io::AsyncRead for PanicStream {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            panic!("injected reader panic");
        }
    }
    impl tokio::io::AsyncWrite for PanicStream {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }
    let (monitor, mut events) = Monitor::start(PanicStream);
    assert_eq!(
        next(&mut events).await,
        Event::Closed(CloseReason::WorkerStopped)
    );
    assert_eq!(monitor.status(), LinkStatus::Closed);
    assert_eq!(
        monitor
            .write(&Command::Version, Duration::from_secs(1))
            .await,
        Err(TransportError::Closed)
    );
}

#[tokio::test]
async fn receive_fence_drains_preloaded_terminals_before_acknowledging_the_boundary() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"JEE\n").await.unwrap();
    let fence = monitor.fence(Duration::from_secs(1)).await.unwrap();
    assert_eq!(
        fence.minimum_sequence, 2,
        "preloaded terminal crossed receive fence"
    );
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { sequence: 1, .. }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn receive_fence_excludes_an_old_partial_line_but_not_the_next_complete_line() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(b"J0").await.unwrap();
    let fence = monitor.fence(Duration::from_secs(1)).await.unwrap();
    assert_eq!(fence.minimum_sequence, 2);
    hat.write_all(b"1\nN\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            sequence: 1,
            packet: Ok(Packet {
                kind: Kind::Dali(DaliFrame::Bits8(1)),
                ..
            })
        }
    ));
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            sequence: 2,
            packet: Ok(Packet {
                kind: Kind::NoResponse,
                ..
            })
        }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn receive_fence_does_not_exclude_an_extra_packet_for_a_line_already_discarded() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    hat.write_all(&[b'X'; 100]).await.unwrap();
    let fence = monitor.fence(Duration::from_secs(1)).await.unwrap();
    assert_eq!(fence.minimum_sequence, 2);
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            sequence: 1,
            packet: Err(DecodeError::LineTooLong)
        }
    ));
    hat.write_all(b"\nN\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet {
            sequence: 2,
            packet: Ok(Packet {
                kind: Kind::NoResponse,
                ..
            })
        }
    ));
    monitor.close().await.unwrap();
}

struct RetryDrain {
    bytes: &'static [u8],
    retries: usize,
}
impl tokio::io::AsyncRead for RetryDrain {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Pending
    }
}
impl tokio::io::AsyncWrite for RetryDrain {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}
impl crate::atx::stream::DrainRead for RetryDrain {
    fn poll_drain(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<crate::atx::stream::Drain>> {
        if self.retries > 0 {
            self.retries -= 1;
            context.waker().wake_by_ref();
            return std::task::Poll::Pending;
        }
        if self.bytes.is_empty() {
            return std::task::Poll::Ready(Ok(crate::atx::stream::Drain::Empty));
        }
        let count = buffer.remaining().min(self.bytes.len());
        buffer.put_slice(&self.bytes[..count]);
        self.bytes = &self.bytes[count..];
        std::task::Poll::Ready(Ok(crate::atx::stream::Drain::Data))
    }
}

#[tokio::test]
async fn retry_pending_is_not_evidence_of_an_empty_receive_buffer() {
    let (monitor, mut events) = Monitor::start(RetryDrain {
        bytes: b"JEE\n",
        retries: 1,
    });
    let fence = monitor.fence(Duration::from_secs(1)).await.unwrap();
    assert_eq!(
        fence.minimum_sequence, 2,
        "a retry/yield let an old terminal cross the fence"
    );
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { sequence: 1, .. }
    ));
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn an_endless_drain_retry_has_its_own_bound_and_can_be_closed() {
    let (monitor, _) = Monitor::start(RetryDrain {
        bytes: b"JEE\n",
        retries: usize::MAX,
    });
    assert_eq!(
        monitor.fence(Duration::from_secs(1)).await,
        Err(TransportError::Busy)
    );
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn the_first_accepted_write_excludes_a_partial_line_observed_after_the_fence() {
    let (stream, mut hat) = tokio::io::duplex(1024);
    let (monitor, mut events) = Monitor::start(stream);
    assert_eq!(
        monitor
            .fence(Duration::from_secs(1))
            .await
            .unwrap()
            .minimum_sequence,
        1
    );
    let lock = monitor.inner.writer.lock().await;
    let marker = Arc::new(AtomicU64::new(u64::MAX));
    let writer = monitor.clone();
    let stamp = Arc::clone(&marker);
    let write = tokio::spawn(async move {
        writer
            .write_correlated(&Command::Version, Duration::from_secs(1), stamp)
            .await
    });
    hat.write_all(b"J0").await.unwrap();
    assert_eq!(
        monitor
            .fence(Duration::from_secs(1))
            .await
            .unwrap()
            .minimum_sequence,
        2
    );
    assert_eq!(
        marker.load(Ordering::Acquire),
        u64::MAX,
        "write boundary was stamped while still waiting for preflight"
    );
    drop(lock);
    write.await.unwrap().unwrap();
    assert_eq!(
        marker.load(Ordering::Acquire),
        2,
        "observed partial line was omitted from first-write boundary"
    );
    hat.write_all(b"1\nJ06\n").await.unwrap();
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { sequence: 1, .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        Event::Packet { sequence: 2, .. }
    ));
    monitor.close().await.unwrap();
}
