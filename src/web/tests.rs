use super::*;
use crate::{
    config::ConfigPaths,
    service::{DaliService, Phase, ServiceHandle},
};
use auth::{Auth, Policy, Refusal};
use mqtt_bridge_kit::Bridge;
use mqtt_test_broker::FakeBroker;
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{timeout, Instant},
};
mod assets;
mod browser;
mod jobs;
mod launch;

#[test]
fn web_settings_validate_credentials_and_canonical_origin_without_resources() {
    let address = "127.0.0.1:8080".parse().unwrap();
    for origin in [
        "file://localhost",
        "http://u:p@localhost",
        "http://localhost/path",
        "http://localhost?x=1",
        "http://localhost/#x",
        "http://localhost:bad",
    ] {
        assert!(
            Settings::new(address, origin, "password").is_err(),
            "invalid web origin accepted: {origin}"
        );
    }
    assert!(Settings::new(address, "http://localhost", "").is_err());
    assert!(Settings::new(address, "http://localhost", &"x".repeat(1025)).is_err());
    assert_eq!(
        Settings::new(address, "http://LOCALHOST:80/", " ")
            .unwrap()
            .origin,
        "http://localhost"
    );
    assert!(
        Settings::new(address, "https://localhost:443/", "password")
            .unwrap()
            .secure
    );
}
#[test]
fn web_password_verifier_refuses_wrong_credentials_without_consuming_a_session() {
    let mut auth = Auth::new(
        digest("correct-secret"),
        Policy {
            sessions: 1,
            ..Policy::default()
        },
    );
    let now = Instant::now();
    assert!(
        matches!(
            auth.login("wrong-secret", None, now),
            Err(Refusal::Password)
        ),
        "password verifier accepted a wrong credential"
    );
    assert!(
        auth.login("correct-secret", None, now).is_ok(),
        "wrong password consumed a session slot"
    );
}

#[test]
fn web_auth_enforces_idle_absolute_activity_rotation_rate_and_capacity() {
    let now = Instant::now();
    let policy = Policy {
        idle: Duration::from_secs(10),
        absolute: Duration::from_secs(25),
        sessions: 2,
        attempts: 100,
        ..Policy::default()
    };
    let mut auth = Auth::new(digest("secret"), policy);
    let (one, first) = auth.login("secret", None, now).ok().unwrap();
    assert_eq!(one.len(), 64);
    assert_ne!(one, first.csrf);
    assert!(auth.access(&one, now + Duration::from_secs(9)).is_some());
    assert!(
        auth.access(&one, now + Duration::from_secs(10)).is_none(),
        "passive inspection extended idle expiry"
    );
    let (one, _) = auth.login("secret", None, now).ok().unwrap();
    assert!(auth.activity(&one, now + Duration::from_secs(9)));
    assert!(auth.activity(&one, now + Duration::from_secs(18)));
    assert!(auth.access(&one, now + Duration::from_secs(24)).is_some());
    assert!(
        auth.access(&one, now + Duration::from_secs(25)).is_none(),
        "user activity bypassed absolute expiry"
    );
    let (one, _) = auth.login("secret", None, now).ok().unwrap();
    let (two, _) = auth.login("secret", None, now).ok().unwrap();
    assert!(
        matches!(auth.login("secret", None, now), Err(Refusal::Capacity)),
        "session cap was ignored"
    );
    let (rotated, _) = auth.login("secret", Some(&one), now).ok().unwrap();
    assert_ne!(one, rotated);
    assert!(auth.access(&one, now).is_none());
    auth.logout(&two);
    assert!(auth.access(&two, now).is_none());
    let mut auth = Auth::new(
        digest("secret"),
        Policy {
            attempts: 2,
            window: Duration::from_secs(5),
            ..policy
        },
    );
    assert!(matches!(
        auth.login("wrong", None, now),
        Err(Refusal::Password)
    ));
    assert!(auth.login("secret", None, now).is_ok());
    assert!(
        matches!(auth.login("secret", None, now), Err(Refusal::Rate)),
        "login rate limit ignored successful attempts"
    );
    assert!(
        auth.login("secret", None, now + Duration::from_secs(5))
            .is_ok(),
        "login rate never recovered"
    );
    assert!(auth
        .login("wrong", Some(&rotated), now + Duration::from_secs(5))
        .is_err());
}

struct Fixture {
    path: PathBuf,
    running: mqtt_bridge_kit::Running,
    handle: ServiceHandle,
    server: Server,
    _broker: FakeBroker,
}
impl Fixture {
    async fn new() -> Self {
        Self::configured(|service| service).await
    }
    async fn configured(configure: impl FnOnce(DaliService) -> DaliService) -> Self {
        Self::with_inventory(json!({"schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef","name":"Controller","buses":[{"bus":0,"name":"PrivateBus","enabled":true,"outputs":[{"short_address":0,"name":"PrivateLight"}]}]}), configure).await
    }
    async fn with_inventory(
        inventory: Value,
        configure: impl FnOnce(DaliService) -> DaliService,
    ) -> Self {
        let path = std::env::temp_dir().join(format!("dali-web-{:032x}", rand::random::<u128>()));
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("dali.v2.json"),
            serde_json::to_vec(&inventory).unwrap(),
        )
        .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) =
            DaliService::new(ConfigPaths::in_directory(&path), "Controller".into());
        let running = Bridge::new("DALI")
            .instance("Controller")
            .broker(&broker.address())
            .start_managed(configure(service))
            .await
            .unwrap();
        timeout(Duration::from_secs(2), async {
            loop {
                if handle.snapshot().is_some_and(|s| s.phase == Phase::Ready) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let settings =
            Settings::new("127.0.0.1:0".parse().unwrap(), "http://dali.test", "secret").unwrap();
        let server = Server::start(
            settings,
            handle.clone(),
            Instant::now() + Duration::from_secs(1),
        )
        .await
        .unwrap();
        Self {
            path,
            running,
            handle,
            server,
            _broker: broker,
        }
    }
    async fn close(mut self) {
        assert!(
            self.server
                .close(Instant::now() + Duration::from_secs(1))
                .await,
            "web connection ownership did not settle"
        );
        assert!(self.running.stop().await);
        fs::remove_dir_all(&self.path).unwrap();
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Reply {
        request(self.server.address, method, path, headers, body).await
    }
    async fn login(&self) -> (String, String) {
        let reply = self
            .request(
                "POST",
                "/api/login",
                &[
                    ("Origin", "http://dali.test"),
                    ("Content-Type", "application/json"),
                ],
                r#"{"password":"secret"}"#,
            )
            .await;
        assert_eq!(reply.status, 200, "web login failed: {}", reply.body);
        let cookie = reply
            .header("set-cookie")
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        (cookie, reply.json()["csrf"].as_str().unwrap().into())
    }
}
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}
impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}
async fn open(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (Reply, BufReader<TcpStream>) {
    timeout(Duration::from_secs(2),async {
        let mut stream=TcpStream::connect(address).await.unwrap();
        let mut request=format!("{method} {path} HTTP/1.1\r\nHost: dali.test\r\nConnection: close\r\nContent-Length: {}\r\n",body.len());
        for (name,value) in headers { request.push_str(&format!("{name}: {value}\r\n")); }
        request.push_str("\r\n");request.push_str(body);
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut stream=BufReader::new(stream);
        let mut line=String::new();stream.read_line(&mut line).await.unwrap();
        let status=line.split_whitespace().nth(1).expect("missing HTTP response").parse().unwrap();
        let mut headers=Vec::new();
        loop { line.clear();stream.read_line(&mut line).await.unwrap(); if line=="\r\n" { break; } let (name,value)=line.split_once(':').unwrap();headers.push((name.to_ascii_lowercase(),value.trim().to_owned())); }
        (Reply { status,headers,body:String::new() },stream)
    }).await.expect("HTTP request did not settle")
}
async fn request(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Reply {
    let (mut reply, mut stream) = open(address, method, path, headers, body).await;
    timeout(
        Duration::from_secs(2),
        stream.read_to_string(&mut reply.body),
    )
    .await
    .unwrap()
    .unwrap();
    reply
}
async fn chunk(stream: &mut BufReader<TcpStream>) -> Option<String> {
    timeout(Duration::from_secs(2), async {
        let mut line = String::new();
        if stream.read_line(&mut line).await.unwrap() == 0 {
            return None;
        }
        let size = usize::from_str_radix(line.trim(), 16).unwrap();
        if size == 0 {
            return None;
        }
        assert!(size <= handlers::SNAPSHOT_BYTES + 100);
        let mut bytes = vec![0; size + 2];
        stream.read_exact(&mut bytes).await.unwrap();
        Some(String::from_utf8(bytes[..size].to_vec()).unwrap())
    })
    .await
    .expect("SSE did not wake or terminate")
}
async fn bootstrap(stream: &mut BufReader<TcpStream>) -> String {
    let snapshot = chunk(stream).await.expect("missing initial snapshot");
    assert!(snapshot.contains("event: snapshot\n"));
    let jobs = chunk(stream).await.expect("missing initial jobs");
    assert!(jobs.contains("event: jobs\n"));
    snapshot
}

#[tokio::test]
async fn web_http_protects_direct_urls_and_only_cookie_authenticates() {
    let fixture = Fixture::new().await;
    let identity = fixture.request("GET", "/api/identity", &[], "").await;
    assert_eq!(identity.status, 200);
    assert_eq!(identity.json().as_object().unwrap().len(), 3);
    assert!(
        !identity.body.contains("Private"),
        "public bootstrap leaked inventory"
    );
    for path in [
        "/api/snapshot",
        "/api/events",
        "/api/session",
        "/api/unknown",
    ] {
        let reply = fixture.request("GET", path, &[], "").await;
        assert_eq!(
            reply.status, 401,
            "direct protected API bypassed authentication"
        );
        assert!(!reply.body.contains("Private"));
    }
    let page = fixture.request("GET", "/buses/0", &[], "").await;
    assert_eq!(page.status, 303);
    assert_eq!(page.header("location"), Some("/login"));
    let (cookie, csrf) = fixture.login().await;
    let reply = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(reply.status, 200);
    assert!(reply.body.contains("PrivateLight"));
    assert!(!reply.body.contains("revision_high_water"));
    assert_eq!(reply.json()["hardware"]["state"], "disabled");
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    for headers in [
        vec![(
            "Authorization",
            format!("Bearer {}", cookie.split_once('=').unwrap().1),
        )],
        vec![("Cookie", format!("{cookie}; {cookie}"))],
        vec![("Cookie", "DaliSession=guessed".into())],
    ] {
        let borrowed: Vec<_> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(
            fixture
                .request("GET", "/api/snapshot", &borrowed, "")
                .await
                .status,
            401,
            "alternative or duplicate session token authenticated"
        );
    }
    let token = cookie.split_once('=').unwrap().1;
    assert_eq!(
        fixture
            .request("GET", &format!("/api/snapshot?token={token}"), &[], "")
            .await
            .status,
        401,
        "URL token authenticated"
    );
    let session = fixture
        .request("GET", "/api/session", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(session.json()["csrf"], csrf);
    fixture.close().await;
}

#[tokio::test]
async fn web_http_origin_csrf_rotation_and_logout_revoke_live_stream() {
    let fixture = Fixture::new().await;
    for headers in [
        vec![("Content-Type", "application/json")],
        vec![
            ("Origin", "http://evil.test"),
            ("Content-Type", "application/json"),
        ],
        vec![
            ("Origin", "http://dali.test"),
            ("Origin", "http://dali.test"),
            ("Content-Type", "application/json"),
        ],
    ] {
        assert_eq!(
            fixture
                .request("POST", "/api/login", &headers, r#"{"password":"secret"}"#)
                .await
                .status,
            403,
            "login origin check was bypassed"
        );
    }
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/login",
                &[
                    ("Origin", "http://dali.test"),
                    ("Content-Type", "application/json")
                ],
                r#"{"password":"secret","extra":1}"#
            )
            .await
            .status,
        400
    );
    let (cookie, csrf) = fixture.login().await;
    let (reply, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(bootstrap(&mut stream).await.contains("PrivateLight"));
    for (origin, token) in [
        ("http://evil.test", csrf.as_str()),
        ("http://dali.test", "bad"),
    ] {
        assert_eq!(
            fixture
                .request(
                    "POST",
                    "/api/logout",
                    &[
                        ("Cookie", &cookie),
                        ("Origin", origin),
                        ("X-CSRF-Token", token)
                    ],
                    ""
                )
                .await
                .status,
            403,
            "logout bypassed origin or CSRF protection"
        );
    }
    let logout = fixture
        .request(
            "POST",
            "/api/logout",
            &[
                ("Cookie", &cookie),
                ("Origin", "http://dali.test"),
                ("X-CSRF-Token", &csrf),
            ],
            "",
        )
        .await;
    assert_eq!(logout.status, 204);
    assert!(logout.header("set-cookie").unwrap().contains("Max-Age=0"));
    assert!(
        chunk(&mut stream).await.is_none(),
        "logout did not revoke an already-open SSE stream"
    );
    assert_eq!(
        fixture
            .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
            .await
            .status,
        401
    );
    let (cookie, _) = fixture.login().await;
    let rotated = fixture
        .request(
            "POST",
            "/api/login",
            &[
                ("Cookie", &cookie),
                ("Origin", "http://dali.test"),
                ("Content-Type", "application/json"),
            ],
            r#"{"password":"secret"}"#,
        )
        .await;
    assert_eq!(rotated.status, 200);
    assert_ne!(
        rotated
            .header("set-cookie")
            .unwrap()
            .split(';')
            .next()
            .unwrap(),
        cookie
    );
    assert_eq!(
        fixture
            .request("GET", "/api/session", &[("Cookie", &cookie)], "")
            .await
            .status,
        401,
        "login retained the previous session"
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_sse_idle_expiry_and_user_activity_preserve_fixed_absolute_expiry() {
    let fixture = Fixture::new().await;
    *fixture.server.state.auth.lock().unwrap() = Auth::new(
        digest("secret"),
        Policy {
            idle: Duration::from_millis(600),
            absolute: Duration::from_millis(1500),
            ..Policy::default()
        },
    );
    let (cookie, _) = fixture.login().await;
    let (_, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    bootstrap(&mut stream).await;
    for _ in 0..3 {
        assert_eq!(
            fixture
                .request("GET", "/api/session", &[("Cookie", &cookie)], "")
                .await
                .status,
            200
        );
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    assert!(
        chunk(&mut stream).await.is_none(),
        "passive SSE/session reads extended idle lifetime"
    );
    let (cookie, csrf) = fixture.login().await;
    let (_, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    bootstrap(&mut stream).await;
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(
            fixture
                .request(
                    "POST",
                    "/api/session/activity",
                    &[
                        ("Cookie", &cookie),
                        ("Origin", "http://dali.test"),
                        ("X-CSRF-Token", &csrf)
                    ],
                    ""
                )
                .await
                .status,
            204,
            "explicit activity did not extend idle lifetime"
        );
    }
    assert!(
        chunk(&mut stream).await.is_none(),
        "active SSE bypassed absolute session expiry"
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_managed_owner_serves_without_broker_and_closes_with_retained_handle() {
    let path =
        std::env::temp_dir().join(format!("dali-web-managed-{:032x}", rand::random::<u128>()));
    fs::create_dir(&path).unwrap();
    let broker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    // The private sink never sends CONNACK; no external broker is contacted.
    let broker_address = broker.local_addr().unwrap();
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let settings = Settings::new(address, "http://dali.test", "secret").unwrap();
    let (service, handle) = DaliService::new(ConfigPaths::in_directory(&path), "Controller".into());
    let running = Bridge::new("DALI")
        .instance("Controller")
        .broker(&broker_address.to_string())
        .start_managed(service.with_web(settings))
        .await
        .unwrap();
    timeout(Duration::from_secs(2), async {
        loop {
            if handle.snapshot().is_some_and(|s| s.phase == Phase::Ready) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("web service waited for MQTT before runtime attachment");
    let login = request(
        address,
        "POST",
        "/api/login",
        &[
            ("Origin", "http://dali.test"),
            ("Content-Type", "application/json"),
        ],
        r#"{"password":"secret"}"#,
    )
    .await;
    assert_eq!(
        login.status, 200,
        "web depended on a functioning MQTT broker"
    );
    let cookie = login
        .header("set-cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let csrf = login.json()["csrf"].as_str().unwrap().to_owned();
    let initial = handle.snapshot().unwrap();
    let generation = format!(
        "{}.{}",
        initial.model.inventory().inventory_epoch,
        initial.model.inventory().revision
    );
    let command = json!({"generation":generation,"command":"describe","target":null,
        "params":{"expected_revision":initial.model.inventory().revision,"description":"saved without broker"}});
    let admitted = request(
        address,
        "POST",
        "/api/jobs",
        &[
            ("Cookie", cookie),
            ("Origin", "http://dali.test"),
            ("X-CSRF-Token", &csrf),
            ("Content-Type", "application/json"),
        ],
        &command.to_string(),
    )
    .await;
    assert_eq!(
        admitted.status, 202,
        "HTTP command admission depended on a connected MQTT broker"
    );
    timeout(Duration::from_secs(2), async {
        loop {
            let result = request(
                address,
                "GET",
                admitted.header("location").unwrap(),
                &[("Cookie", cookie)],
                "",
            )
            .await
            .json();
            if result["status"] != "running" {
                assert_eq!(
                    result["status"], "succeeded",
                    "HTTP save waited for MQTT command completion: {result}"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("broker-absent HTTP command never completed");
    assert_eq!(
        handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .description
            .as_deref(),
        Some("saved without broker")
    );
    let (_, mut stream) = open(address, "GET", "/api/events", &[("Cookie", cookie)], "").await;
    bootstrap(&mut stream).await;
    let clean = timeout(Duration::from_secs(6), running.stop())
        .await
        .expect("managed web shutdown exceeded process bound");
    assert!(
        clean,
        "managed HTTP/broker-absent shutdown did not complete cleanly"
    );
    assert!(
        chunk(&mut stream).await.is_none(),
        "managed stop retained the HTTP live stream"
    );
    assert!(
        TcpStream::connect(address).await.is_err(),
        "retained service handle kept listener alive"
    );
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Closed);
    drop(broker);
    fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn web_http_rate_secure_cookie_and_escaped_password_bounds() {
    let fixture = Fixture::new().await;
    for _ in 0..10 {
        assert_eq!(
            fixture
                .request(
                    "POST",
                    "/api/login",
                    &[
                        ("Origin", "http://dali.test"),
                        ("Content-Type", "application/json")
                    ],
                    r#"{"password":"wrong"}"#
                )
                .await
                .status,
            401
        );
    }
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/login",
                &[
                    ("Origin", "http://dali.test"),
                    ("Content-Type", "application/json")
                ],
                r#"{"password":"secret"}"#
            )
            .await
            .status,
        429,
        "HTTP password guessing bypassed rate limit"
    );
    let password = "\n".repeat(1024);
    let mut https = Server::start(
        Settings::new(
            "127.0.0.1:0".parse().unwrap(),
            "https://dali.test",
            &password,
        )
        .unwrap(),
        fixture.handle.clone(),
        Instant::now() + Duration::from_secs(1),
    )
    .await
    .unwrap();
    let body = json!({"password":password}).to_string();
    assert!(body.len() > 2048);
    let reply = request(
        https.address,
        "POST",
        "/api/login",
        &[
            ("Origin", "https://dali.test"),
            ("Content-Type", "application/json"),
        ],
        &body,
    )
    .await;
    assert_eq!(
        reply.status, 200,
        "configured escaped password could not be submitted"
    );
    let cookie = reply.header("set-cookie").unwrap();
    for attribute in ["HttpOnly", "SameSite=Strict", "Path=/", "Secure"] {
        assert!(
            cookie.contains(attribute),
            "secure session cookie omitted required attribute"
        );
    }
    assert!(!cookie.contains("Domain="));
    assert!(!reply.body.contains("DaliSession"));
    assert!(https.close(Instant::now() + Duration::from_secs(1)).await);
    fixture.close().await;
}

#[tokio::test]
async fn web_stream_and_connection_caps_release_only_when_actual_io_drops() {
    let fixture = Fixture::new().await;
    let (cookie, _) = fixture.login().await;
    let mut streams = Vec::new();
    for _ in 0..8 {
        let (reply, mut stream) = open(
            fixture.server.address,
            "GET",
            "/api/events",
            &[("Cookie", &cookie)],
            "",
        )
        .await;
        assert_eq!(reply.status, 200);
        assert!(chunk(&mut stream).await.is_some());
        streams.push(stream);
    }
    let (excess, body) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(excess.status, 503, "SSE subscription cap was ignored");
    drop(body);
    drop(streams.pop());
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.streams.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (reply, mut replacement) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(chunk(&mut replacement).await.is_some());
    drop(replacement);
    drop(streams);
    timeout(Duration::from_secs(1), async {
        while fixture.server.connections.available_permits() != 64 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut idle = Vec::new();
    for _ in 0..64 {
        idle.push(TcpStream::connect(fixture.server.address).await.unwrap());
    }
    timeout(Duration::from_secs(1), async {
        while fixture.server.connections.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut excess = TcpStream::connect(fixture.server.address).await.unwrap();
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_secs(1), excess.read(&mut byte))
            .await
            .expect("accepted TCP connection cap was ignored")
            .unwrap(),
        0
    );
    drop(idle);
    fixture.close().await;
}

#[tokio::test]
async fn web_snapshot_stream_observes_latest_saved_inventory_and_occupied_bind_refuses() {
    let fixture = Fixture::new().await;
    let (cookie, _) = fixture.login().await;
    let (_, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert!(chunk(&mut stream).await.unwrap().contains("PrivateLight"));
    fixture
        .handle
        .edit(
            1,
            crate::service::MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "ChangedLight".into(),
            },
            mqtt_bridge_kit::RequestContext::new("0123456789abcdef0123456789abcdef.1").unwrap(),
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    let mut observed = false;
    for _ in 0..4 {
        if chunk(&mut stream).await.unwrap().contains("ChangedLight") {
            observed = true;
            break;
        }
    }
    assert!(observed, "SSE did not resnapshot confirmed inventory");
    let occupied = Settings::new(fixture.server.address, "http://dali.test", "secret").unwrap();
    assert!(
        Server::start(
            occupied,
            fixture.handle.clone(),
            Instant::now() + Duration::from_secs(1)
        )
        .await
        .is_err(),
        "second HTTP owner reused an occupied listener"
    );
    let expired =
        Settings::new("127.0.0.1:0".parse().unwrap(), "http://dali.test", "secret").unwrap();
    assert!(
        Server::start(expired, fixture.handle.clone(), Instant::now())
            .await
            .is_err(),
        "expired web startup opened a listener"
    );
    drop(stream);
    fixture.close().await;
}

#[tokio::test]
async fn web_incomplete_uploads_are_capacity_limited_and_expire() {
    let fixture = Fixture::new().await;
    let mut uploads = Vec::new();
    for _ in 0..32 {
        let mut stream = TcpStream::connect(fixture.server.address).await.unwrap();
        stream.write_all(b"POST /api/login HTTP/1.1\r\nHost: dali.test\r\nOrigin: http://dali.test\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: 8000\r\n\r\n{\"password\":").await.unwrap();
        uploads.push(stream);
    }
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.handlers.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .request("GET", "/api/identity", &[], "")
            .await
            .status,
        503,
        "HTTP handler capacity was ignored"
    );
    let mut first = BufReader::new(uploads.remove(0));
    let mut line = String::new();
    timeout(Duration::from_secs(6), first.read_line(&mut line))
        .await
        .expect("HTTP body read exceeded its request deadline")
        .unwrap();
    assert!(
        line.starts_with("HTTP/1.1 408"),
        "incomplete upload did not return request timeout"
    );
    drop(first);
    drop(uploads);
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.handlers.available_permits() != 32 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .request("GET", "/api/identity", &[], "")
            .await
            .status,
        200
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_forced_close_destroys_an_admitted_incomplete_body_reader() {
    let mut fixture = Fixture::new().await;
    let mut stream = TcpStream::connect(fixture.server.address).await.unwrap();
    stream.write_all(b"POST /api/login HTTP/1.1\r\nHost: dali.test\r\nOrigin: http://dali.test\r\nContent-Type: application/json\r\nContent-Length: 8000\r\n\r\n{\"password\":").await.unwrap();
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.handlers.available_permits() == 32 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("incomplete body never entered an HTTP handler");
    assert!(
        fixture
            .server
            .close(Instant::now() + Duration::from_millis(300))
            .await,
        "forced web shutdown did not destroy admitted body reader"
    );
    let mut byte = [0];
    let result = timeout(Duration::from_secs(1), stream.read(&mut byte))
        .await
        .unwrap();
    assert!(
        matches!(result, Ok(0) | Err(_)),
        "forced HTTP close retained a descriptor"
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_close_reports_incomplete_when_connection_drop_barrier_is_held() {
    let mut fixture = Fixture::new().await;
    // Isolate actual-drop accounting from ordinary graceful-close behavior.
    let held = fixture
        .server
        .connections
        .clone()
        .try_acquire_owned()
        .unwrap();
    assert!(
        !fixture
            .server
            .close(Instant::now() + Duration::from_millis(100))
            .await,
        "web close ignored an outstanding connection ownership permit"
    );
    drop(held);
    fixture.close().await;
}

#[tokio::test]
async fn web_close_destroys_idle_connections_and_invalidates_restart_sessions() {
    let fixture = Fixture::new().await;
    let (cookie, _) = fixture.login().await;
    let mut idle = TcpStream::connect(fixture.server.address).await.unwrap();
    idle.write_all(b"GET /api/snapshot HTTP/1.1\r\n")
        .await
        .unwrap();
    let address = fixture.server.address;
    let handle = fixture.handle.clone();
    fixture.close().await;
    let mut byte = [0];
    let result = timeout(Duration::from_secs(1), idle.read(&mut byte))
        .await
        .unwrap();
    assert!(
        matches!(result, Ok(0) | Err(_)),
        "web shutdown retained an idle accepted connection"
    );
    assert!(TcpStream::connect(address).await.is_err());
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Closed);
    let restarted = Fixture::new().await;
    assert_eq!(
        restarted
            .request("GET", "/api/session", &[("Cookie", &cookie)], "")
            .await
            .status,
        401,
        "restart accepted an old session"
    );
    restarted.close().await;
}
