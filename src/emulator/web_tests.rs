use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
async fn request(
    address: SocketAddr,
    host: &str,
    path: &str,
    extra: &str,
    body: Option<&str>,
) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let body = body.unwrap_or("");
    let method = if body.is_empty() { "GET" } else { "POST" };
    let value=format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{extra}\r\n{body}",body.len());
    stream.write_all(value.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(response).unwrap()
}
#[tokio::test]
async fn browser_host_origin_csrf_validation_and_idle_shutdown_are_enforced() {
    let scenario = serde_json::from_str("{\"version\":1,\"buses\":[{\"number\":0}]}").unwrap();
    let (emulator, _) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
        .await
        .unwrap();
    assert!(Server::bind("0.0.0.0:0".parse().unwrap(), emulator.clone())
        .await
        .is_err());
    let mut server = Server::bind("127.0.0.1:0".parse().unwrap(), emulator.clone())
        .await
        .unwrap();
    let host = server.address.to_string();
    assert!(
        request(server.address, "attacker.invalid", "/api/session", "", None)
            .await
            .starts_with("HTTP/1.1 403")
    );
    assert!(request(
        server.address,
        &host,
        "/api/session",
        "Origin: http://evil.invalid\r\n",
        None
    )
    .await
    .starts_with("HTTP/1.1 403"));
    let response = request(server.address, &host, "/api/session", "", None).await;
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("cache-control: no-store"));
    let json: serde_json::Value =
        serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
    let token = json["csrf"].as_str().unwrap();
    let body = "{\"op\":\"add_bus\",\"bus\":{\"number\":1}}";
    assert!(
        request(server.address, &host, "/api/control", "", Some(body))
            .await
            .starts_with("HTTP/1.1 403")
    );
    let headers = format!("Origin: http://{host}\r\nX-Emulator-CSRF: {token}\r\n");
    assert!(
        request(server.address, &host, "/api/control", &headers, Some(body))
            .await
            .starts_with("HTTP/1.1 200")
    );
    assert!(
        request(server.address, &host, "/api/control", &headers, Some(body))
            .await
            .starts_with("HTTP/1.1 422")
    );
    let mut idle = TcpStream::connect(server.address).await.unwrap();
    idle.write_all(b"GET /").await.unwrap();
    server.close().await.unwrap();
    let mut bytes = Vec::new();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), idle.read_to_end(&mut bytes))
            .await
            .is_ok()
    );
    emulator.close().await.unwrap();
}
