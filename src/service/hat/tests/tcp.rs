use super::*;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[tokio::test]
async fn tcp_open_timeout_retries_while_uart_open_timeout_remains_terminal() {
    for tcp in [false, true] {
        let config = if tcp {
            Settings::tcp("127.0.0.1:9".parse().unwrap(), RepeatPolicy::Unverified)
        } else {
            settings(PathBuf::from("/not-opened"))
        };
        let mut hat = Hat::new(Some(config), vec![0]);
        hat.finished(Done::Open(Err(io::ErrorKind::TimedOut.into())))
            .unwrap();
        assert_eq!(
            hat.terminal, !tcp,
            "TCP async timeout inherited the UART blocking-worker terminal policy"
        );
        assert!(
            matches!(hat.hardware(), Hardware::Unavailable { retrying, .. } if retrying == tcp)
        );
        assert!(hat.session.is_none());
    }
}

#[tokio::test]
async fn tcp_new_connection_clears_old_parser_poison_with_only_an_immediate_boot() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (opened, peer) = tokio::join!(TcpStream::connect(address), listener.accept());
    let mut peer = peer.unwrap().0;
    peer.write_all(b"B000004\n").await.unwrap();
    let mut config = Settings::tcp(address, RepeatPolicy::Unverified);
    config.quiet = Duration::from_millis(10);
    config.timing = Timing {
        setup: Duration::from_secs(1),
        poll: Duration::from_secs(60),
        retry: Duration::from_millis(10),
    };
    let mut hat = Hat::new(Some(config), vec![0]);
    hat.requires_boot = true;
    hat.reset = true;
    hat.finished(Done::Open(Ok(Connection::Tcp(opened.unwrap()))))
        .unwrap();
    assert!(
        !hat.requires_boot && !hat.needs_boundary && !hat.reset,
        "new TCP parser lifetime retained old partial-write poison"
    );
    let worker = tokio::spawn(async move {
        let mut peer = BufReader::new(peer);
        let mut line = String::new();
        for (command, response) in [("v\n", "V000004\n"), ("d\n", "D24\n")] {
            tokio::time::timeout(Duration::from_secs(2), peer.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(line, command);
            peer.get_mut().write_all(response.as_bytes()).await.unwrap();
            line.clear();
        }
        let mut eof = [0];
        assert_eq!(
            peer.read(&mut eof).await.unwrap(),
            0,
            "closed HAT retained a TCP descriptor"
        );
    });
    drive(&mut hat, |h| matches!(h, Hardware::Ready { .. })).await;
    hat.close(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
}
