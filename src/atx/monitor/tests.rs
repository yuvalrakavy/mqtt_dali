use super::super::wire::{DaliFrame, Kind};
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

async fn next(events: &mut broadcast::Receiver<Event>) -> Event {
    timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("continuous HAT reader stopped delivering events")
        .unwrap()
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
