use super::*;
use std::{
    future::poll_fn,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap());
    let (client, peer) = tokio::join!(client, listener.accept());
    (client.unwrap(), peer.unwrap().0)
}
async fn drain(stream: &mut TcpStream, bytes: &mut [u8]) -> (Drain, usize) {
    let mut buffer = ReadBuf::new(bytes);
    let result = poll_fn(|cx| Pin::new(&mut *stream).poll_drain(cx, &mut buffer))
        .await
        .unwrap();
    (result, buffer.filled().len())
}
#[tokio::test]
async fn tcp_drain_checks_kernel_bytes_without_cached_readiness_and_distinguishes_eof() {
    let (mut client, mut peer) = pair().await;
    let mut bytes = [0; 64];
    assert_eq!(drain(&mut client, &mut bytes).await, (Drain::Empty, 0));
    peer.write_all(b"0L008001\nJ7F\n").await.unwrap();
    // MSG_PEEK waits for actual queued bytes WITHOUT priming Tokio's readiness.
    // This proves try_read's cached WouldBlock cannot stand in for true Empty.
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let count = unsafe {
                libc::recv(
                    std::os::fd::AsRawFd::as_raw_fd(&client),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            if count == b"0L008001\nJ7F\n".len() as isize {
                break;
            }
            if count < 0 {
                assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (result, count) = drain(&mut client, &mut bytes).await;
    assert_eq!(
        result,
        Drain::Data,
        "queued TCP bytes were reported as empty"
    );
    assert_eq!(&bytes[..count], b"0L008001\nJ7F\n");
    assert_eq!(drain(&mut client, &mut bytes).await, (Drain::Empty, 0));
    peer.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), client.readable())
        .await
        .unwrap()
        .unwrap();
    // Readability can retain a previous ready bit; wait for the actual FIN.
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let result = drain(&mut client, &mut bytes).await;
            if result.0 == Drain::Data {
                assert_eq!(result.1, 0, "EOF consumed unexpected bytes");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("TCP EOF was reported as empty forever");
}

#[tokio::test]
async fn tcp_connection_delegates_io_and_wakes_reader_after_empty_drain() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = Endpoint::Tcp(listener.local_addr().unwrap());
    let (opened, peer) = tokio::join!(endpoint.open(), listener.accept());
    let mut connection = opened.unwrap();
    let mut peer = peer.unwrap().0;
    let mut bytes = [0; 2];
    let mut buffer = ReadBuf::new(&mut bytes);
    assert_eq!(
        poll_fn(|cx| Pin::new(&mut connection).poll_drain(cx, &mut buffer))
            .await
            .unwrap(),
        Drain::Empty
    );
    let reader = tokio::spawn(async move {
        connection.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"J\n");
        connection.write_all(b"v\n").await.unwrap();
        connection.flush().await.unwrap();
        connection.shutdown().await.unwrap();
    });
    peer.write_all(b"J").await.unwrap();
    tokio::task::yield_now().await;
    peer.write_all(b"\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), peer.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"v\n");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), peer.read(&mut bytes))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    reader.await.unwrap();
}

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[tokio::test(start_paused = true)]
async fn tcp_connect_deadline_and_cancellation_drop_the_owned_pending_future() {
    for cancel in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(dropped.clone());
        let task = tokio::spawn(connect(async move {
            let _guard = guard;
            std::future::pending::<io::Result<TcpStream>>().await
        }));
        tokio::task::yield_now().await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            let started = tokio::time::Instant::now();
            tokio::time::advance(Duration::from_secs(2)).await;
            let result = tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .expect("TCP connect deadline extended beyond its original two-second budget")
                .unwrap();
            assert_eq!(
                tokio::time::Instant::now() - started,
                Duration::from_secs(2),
                "TCP connect deadline extended beyond its original two-second budget"
            );
            assert_eq!(
                result.err().unwrap().kind(),
                io::ErrorKind::TimedOut,
                "TCP connect did not honor the original two-second deadline"
            );
        }
        assert!(
            dropped.load(Ordering::SeqCst),
            "TCP connect future escaped timeout/cancellation"
        );
    }
}
