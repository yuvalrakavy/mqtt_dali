use super::*;

#[tokio::test]
async fn configuration_launch_exchange_has_strict_public_json_entry() {
    let fixture = Fixture::new().await;
    let response = fixture.request(
        "POST",
        "/api/launch",
        &[
            ("Origin", "http://dali.test"),
            ("Content-Type", "application/json"),
        ],
        r#"{"token":"bad","extra":true}"#,
    ).await;
    assert_eq!(
        response.status, 400,
        "launch exchange must validate its own strict body before cookie authentication"
    );
    fixture.close().await;
}

#[test]
fn configuration_launch_credentials_are_single_use_expiring_bounded_and_session_compatible() {
    let now = Instant::now();
    let mut auth = Auth::new(
        digest("secret"),
        Policy {
            attempts: 100,
            ..Policy::default()
        },
    );
    let token = auth.issue_launch(now).ok().unwrap();
    assert_eq!(token.len(), 64);
    assert!(auth.redeem_launch(&"0".repeat(64), None, now).is_err());
    let (cookie, access) = auth.redeem_launch(&token, None, now).ok().unwrap();
    assert_ne!(cookie, token);
    assert!(auth.access(&cookie, now).is_some());
    assert_eq!(access.expires, now + Duration::from_secs(1800));
    assert!(
        auth.redeem_launch(&token, None, now).is_err(),
        "launch credential was reusable"
    );
    let expired = auth.issue_launch(now).ok().unwrap();
    assert!(
        auth.redeem_launch(&expired, None, now + Duration::from_secs(120))
            .is_err(),
        "launch survived its two-minute deadline"
    );
    for _ in 0..16 {
        auth.issue_launch(now).ok().unwrap();
    }
    assert!(
        auth.issue_launch(now).is_err(),
        "unbounded pending launch credentials"
    );
    assert!(auth.issue_launch(now + Duration::from_secs(120)).is_ok());
    let mut other = Auth::new(digest("secret"), Policy::default());
    assert!(
        other.redeem_launch(&token, None, now).is_err(),
        "launch survived auth-owner restart"
    );
    let mut capacity = Auth::new(
        digest("secret"),
        Policy {
            sessions: 0,
            ..Policy::default()
        },
    );
    let token = capacity.issue_launch(now).ok().unwrap();
    assert!(matches!(
        capacity.redeem_launch(&token, None, now),
        Err(Refusal::Capacity)
    ));
    assert!(matches!(
        capacity.redeem_launch(&token, None, now),
        Err(Refusal::Password)
    ));
}

#[tokio::test]
async fn configuration_launch_http_uses_cookie_origin_csrf_and_no_query_authority() {
    let fixture = Fixture::new().await;
    let value = fixture.handle.configuration_link().unwrap();
    let url = value["url"].as_str().unwrap();
    assert_eq!(value["expires_in_ms"], 120000);
    assert!(url.starts_with("http://dali.test/login#token="));
    let token = url.split_once("#token=").unwrap().1;
    assert_eq!(
        fixture
            .request("GET", &format!("/api/session?token={token}"), &[], "")
            .await
            .status,
        401
    );
    let body = json!({"token":token}).to_string();
    for headers in [
        vec![("Content-Type", "application/json")],
        vec![
            ("Origin", "http://evil.test"),
            ("Content-Type", "application/json"),
        ],
    ] {
        assert_eq!(
            fixture
                .request("POST", "/api/launch", &headers, &body)
                .await
                .status,
            403
        );
    }
    let headers = [
        ("Origin", "http://dali.test"),
        ("Content-Type", "application/json"),
    ];
    let reply = fixture
        .request("POST", "/api/launch", &headers, &body)
        .await;
    assert_eq!(reply.status, 200);
    let cookie = reply
        .header("set-cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert!(
        reply
            .header("set-cookie")
            .unwrap()
            .contains("HttpOnly; SameSite=Strict")
    );
    assert!(!cookie.contains(token));
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert_eq!(
        fixture
            .request("POST", "/api/launch", &headers, &body)
            .await
            .status,
        401
    );
    assert_eq!(
        fixture
            .request("GET", "/api/snapshot", &[("Cookie", cookie)], "")
            .await
            .status,
        200
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/logout",
                &[("Cookie", cookie), ("Origin", "http://dali.test")],
                ""
            )
            .await
            .status,
        403
    );
    let csrf = reply.json()["csrf"].as_str().unwrap().to_owned();
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/logout",
                &[
                    ("Cookie", cookie),
                    ("Origin", "http://dali.test"),
                    ("X-CSRF-Token", &csrf)
                ],
                ""
            )
            .await
            .status,
        204
    );
    assert_eq!(
        fixture
            .request("GET", "/api/session", &[("Cookie", cookie)], "")
            .await
            .status,
        401
    );
    assert!(fixture.handle.configuration_url().is_some());
    fixture.server.begin_stop();
    assert!(
        fixture.handle.configuration_link().is_err(),
        "stopping HTTP issued a new link"
    );
    assert!(fixture.handle.configuration_url().is_none());
    fixture.close().await;
}

#[tokio::test]
async fn configuration_launch_mqtt_command_is_correlated_fenced_nonretained_and_advertised() {
    use mqtt_test_broker::SendOptions;
    let fixture = Fixture::configured(|service| {
        service.with_web(
            Settings::new("127.0.0.1:0".parse().unwrap(), "http://dali.test", "secret").unwrap(),
        )
    })
    .await;
    let broker = &fixture._broker;
    let handle = fixture.handle.clone();
    let reply_topic = "DALI/ConfigurationReply/Controller";
    let options = SendOptions::default()
        .response_topic(reply_topic)
        .correlation_data("launch-request")
        .user_property("target-generation", "0123456789abcdef0123456789abcdef.1");
    timeout(Duration::from_secs(2), async {
        loop {
            if broker
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(|p| {
                    serde_json::from_slice::<Value>(&p.payload).unwrap()["ConfigurationUrl"]
                        == "http://dali.test/login"
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    broker.send_with(
        "DALI/Command/Controller",
        r#"{"command":"get_configuration_link"}"#,
        &options,
    );
    timeout(Duration::from_secs(2), async {
        while broker.received_on(reply_topic).is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let replies = broker.received_on(reply_topic);
    assert_eq!(replies.len(), 1);
    assert!(!replies[0].retain);
    assert_eq!(
        replies[0].correlation_data().unwrap().as_ref(),
        b"launch-request"
    );
    let report: Value = serde_json::from_slice(&replies[0].payload).unwrap();
    assert!(
        report["value"]["url"]
            .as_str()
            .unwrap()
            .starts_with("http://dali.test/login#token=")
    );
    assert!(report.get("target").is_none());
    for (index, payload, generation) in [
        (0, r#"{"command":"get_configuration_link"}"#, "old.1"),
        (
            1,
            r#"{"command":"get_configuration_link","target":"Bus1.Outputs.Light1"}"#,
            "0123456789abcdef0123456789abcdef.1",
        ),
        (
            2,
            r#"{"command":"get_configuration_link","extra":true}"#,
            "0123456789abcdef0123456789abcdef.1",
        ),
    ] {
        let topic = format!("DALI/test/refused/{index}");
        let opts = SendOptions::default()
            .response_topic(&topic)
            .user_property("target-generation", generation);
        broker.send_with("DALI/Command/Controller", payload, &opts);
        timeout(Duration::from_secs(2), async {
            while broker.received_on(&topic).is_empty() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let result: Value = serde_json::from_slice(&broker.received_on(&topic)[0].payload).unwrap();
        assert!(
            result.get("value").is_none(),
            "invalid launch request issued a credential"
        );
    }
    fixture.close().await;
    assert!(handle.configuration_link().is_err());
}
