use super::*;

#[tokio::test]
async fn browser_assets_signin_is_public_but_application_routes_require_the_session() {
    let fixture = Fixture::new().await;
    let login = fixture.request("GET", "/login", &[], "").await;
    assert_eq!(login.status, 200);
    assert!(
        login
            .header("content-type")
            .is_some_and(|v| v.starts_with("text/html")),
        "sign-in route does not serve the browser application"
    );
    assert!(login.body.contains("/assets/login.js"));
    assert!(!login.body.contains("PrivateLight"));
    for path in ["/assets/login.js", "/assets/login.css"] {
        let reply = fixture.request("GET", path, &[], "").await;
        assert_eq!(
            reply.status, 200,
            "minimal sign-in asset is unavailable: {path}"
        );
        assert!(!reply.body.is_empty());
    }
    for path in [
        "/",
        "/buses/0",
        "/buses/0/groups",
        "/buses/0/scenes",
        "/settings",
        "/diagnostics",
        "/assets/app.js",
        "/assets/app.css",
    ] {
        let reply = fixture.request("GET", path, &[], "").await;
        assert_eq!(
            reply.status, 303,
            "browser application bypassed authentication: {path}"
        );
        assert_eq!(reply.header("location"), Some("/login"));
    }
    let (cookie, csrf) = fixture.login().await;
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await
        .json();
    assert_eq!(
        snapshot["inventory"]["Inventory"]["buses"][0]["name"], "PrivateBus",
        "browser decoder's Inventory envelope differs from the actual backend"
    );
    for path in [
        "/",
        "/buses/0",
        "/buses/0/groups",
        "/buses/0/scenes",
        "/settings",
        "/diagnostics",
        "/assets/app.js",
        "/assets/app.css",
    ] {
        let reply = fixture
            .request("GET", path, &[("Cookie", &cookie)], "")
            .await;
        assert_eq!(
            reply.status, 200,
            "authenticated browser asset or route is unavailable: {path}"
        );
        assert_eq!(reply.header("cache-control"), Some("no-store"));
        assert!(!reply
            .header("content-security-policy")
            .unwrap()
            .contains("unsafe-inline"));
    }
    for path in ["/buses/4", "/buses/nope", "/assets/unknown.js", "/unknown"] {
        assert_eq!(
            fixture
                .request("GET", path, &[("Cookie", &cookie)], "")
                .await
                .status,
            404,
            "unknown browser route exposed a filesystem or fallback asset"
        );
    }
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/logout",
                &[
                    ("Cookie", &cookie),
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
            .request("GET", "/assets/app.js", &[("Cookie", &cookie)], "")
            .await
            .status,
        303,
        "revoked session can still fetch protected application code"
    );
    fixture.close().await;
}
