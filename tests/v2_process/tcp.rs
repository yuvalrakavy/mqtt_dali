use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[tokio::test]
async fn hat_tcp_option_connects_without_uart_and_uses_raw_atx_bytes() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let mut child = fixture
        .command_with_transport(&broker, ["--hat-tcp", &endpoint])
        .spawn()
        .unwrap();
    let (stream, _) = timeout(Duration::from_secs(3), listener.accept())
        .await
        .expect("--hat-tcp did not connect to the emulator endpoint")
        .unwrap();
    let mut peer = BufReader::new(stream);
    let mut line = String::new();
    timeout(Duration::from_secs(2), peer.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line, "v\n", "TCP changed the ATX command encoding");
    peer.get_mut().write_all(b"V000004\n").await.unwrap();
    line.clear();
    timeout(Duration::from_secs(2), peer.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line, "d\n");
    peer.get_mut().write_all(b"D24\n").await.unwrap();
    inventory(&broker).await;
    assert!(
        stop(&mut child).await.success(),
        "TCP daemon did not stop cleanly"
    );
    line.clear();
    assert_eq!(
        timeout(Duration::from_secs(1), peer.read_line(&mut line))
            .await
            .unwrap()
            .unwrap(),
        0,
        "stopped process retained the TCP HAT connection"
    );
}

#[tokio::test]
async fn hat_tcp_invalid_or_ambiguous_options_fail_before_resources_start() {
    let broker = FakeBroker::start().await;
    for (options, expected) in [
        (
            vec!["--hat-tcp", "localhost:5580"],
            "--hat-tcp must be a numeric IP:port with a nonzero port",
        ),
        (
            vec!["--hat-tcp", "127.0.0.1:0"],
            "--hat-tcp must be a numeric IP:port with a nonzero port",
        ),
        (
            vec!["--hat-tcp", "[::1]:0"],
            "--hat-tcp must be a numeric IP:port with a nonzero port",
        ),
        (
            vec!["--hat-tcp", "127.0.0.1:5580", "--uart", "none"],
            "--hat-tcp and --uart are mutually exclusive",
        ),
        (
            vec!["--uart=/dev/serial0", "--hat-tcp=127.0.0.1:5580"],
            "--hat-tcp and --uart are mutually exclusive",
        ),
    ] {
        let fixture = Fixture::new();
        let mut command = fixture.command_with_transport(&broker, ["--instance", NAME]);
        let output = timeout(
            Duration::from_secs(3),
            command.args(options).stderr(Stdio::piped()).output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "missing TCP preflight error {expected}"
        );
        assert!(!fixture.0.join("dali.v2.json").exists());
        assert!(!fixture.0.join("logs").exists());
        assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
    }
}
