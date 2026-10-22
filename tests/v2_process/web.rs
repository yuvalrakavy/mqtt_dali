use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn http(address: std::net::SocketAddr, method: &str, path: &str, body: &str) -> String {
    timeout(Duration::from_secs(2),async {
        let mut stream=tokio::net::TcpStream::connect(address).await.unwrap();
        stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: dali.test\r\nOrigin: http://dali.test\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        let mut response=String::new();stream.read_to_string(&mut response).await.unwrap();response
    }).await.expect("canonical HTTP request did not settle")
}

#[tokio::test]
async fn web_canonical_daemon_accepts_cli_and_environment_credentials_without_logging_them() {
    for cli in [true, false] {
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let secret = "canonical-web-secret-8327bdee58";
        let mut command = fixture.command(&broker);
        command.args([
            "--web-bind",
            &address.to_string(),
            "--web-origin",
            "http://dali.test",
        ]);
        if cli {
            command
                .args(["--web-password", secret])
                .env("MQTT_DALI_WEB_PASSWORD", "unused-environment-password");
        } else {
            command.env("MQTT_DALI_WEB_PASSWORD", secret);
        }
        let mut child = command.spawn().unwrap();
        timeout(Duration::from_secs(4), async {
            loop {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "canonical daemon failed before authenticated HTTP startup"
                );
                if tokio::net::TcpStream::connect(address).await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("canonical web listener never opened");
        let identity = http(address, "GET", "/api/identity", "").await;
        assert!(identity.starts_with("HTTP/1.1 200"));
        assert!(identity.contains("Controller"));
        assert!(http(address, "GET", "/api/snapshot", "")
            .await
            .starts_with("HTTP/1.1 401"));
        let login = http(
            address,
            "POST",
            "/api/login",
            &json!({"password":secret}).to_string(),
        )
        .await;
        assert!(
            login.starts_with("HTTP/1.1 200"),
            "canonical credential did not authorize login: {login}"
        );
        assert!(login.contains("HttpOnly"));
        assert!(stop(&mut child).await.success());
        assert!(
            tokio::net::TcpStream::connect(address).await.is_err(),
            "canonical process exit retained web listener"
        );
        assert!(
            !fixture.log().contains(secret),
            "web credential appeared in process logs"
        );
        assert!(!fixture.log().contains("unused-environment-password"));
    }
}

#[tokio::test]
async fn web_missing_password_is_rejected_before_resources_start() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let output = timeout(
        Duration::from_secs(4),
        fixture
            .command(&broker)
            .env_remove("MQTT_DALI_WEB_PASSWORD")
            .args([
                "--web-bind",
                "127.0.0.1:8080",
                "--web-origin",
                "http://localhost:8080",
            ])
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .expect("missing web password started resources")
    .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("web requires a nonempty password"),
        "missing web password did not reach preflight validation"
    );
    assert!(!fixture.0.join("dali.v2.json").exists());
    assert!(!fixture.0.join("logs").exists());
}
