use super::*;
mod observations;
mod scene_saved_view;
mod scene_settings;
mod scenes;
use crate::config_store::test_hooks::{Hooks, Stage, Step};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const EPOCH: &str = "0123456789abcdef0123456789abcdef";
fn command(revision: u64, target: Option<&str>, name: &str, params: Value) -> Value {
    json!({"generation":format!("{EPOCH}.{revision}"),"target":target,"command":name,"params":params})
}
async fn submit(fixture: &Fixture, cookie: &str, csrf: &str, body: &Value) -> Reply {
    fixture
        .request(
            "POST",
            "/api/jobs",
            &[
                ("Cookie", cookie),
                ("Origin", "http://dali.test"),
                ("X-CSRF-Token", csrf),
                ("Content-Type", "application/json"),
            ],
            &body.to_string(),
        )
        .await
}
async fn finished(fixture: &Fixture, cookie: &str, path: &str) -> Value {
    timeout(Duration::from_secs(20), async {
        loop {
            let reply = fixture
                .request("GET", path, &[("Cookie", cookie)], "")
                .await;
            assert_eq!(
                reply.status, 200,
                "job result is not inspectable: {}",
                reply.body
            );
            let value = reply.json();
            if value["status"] != "running" {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("HTTP job never settled")
}

#[tokio::test]
async fn web_jobs_save_metadata_with_original_authority_and_strict_parameters() {
    let fixture = Fixture::new().await;
    let (cookie, csrf) = fixture.login().await;
    let rename = command(
        1,
        Some("PrivateBus.Outputs.PrivateLight"),
        "rename",
        json!({"expected_revision":1,"name":"Kitchen"}),
    );
    let reply = submit(&fixture, &cookie, &csrf, &rename).await;
    assert_eq!(
        reply.status, 202,
        "authenticated command was not admitted as a job: {}",
        reply.body
    );
    let path = reply.header("location").unwrap().to_owned();
    let result = finished(&fixture, &cookie, &path).await;
    assert_eq!(
        result["status"], "succeeded",
        "metadata job failed: {result}"
    );
    assert_eq!(result["result"]["revision"], 2);
    let disk: Value =
        serde_json::from_slice(&fs::read(fixture.path.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(
        disk["buses"][0]["outputs"][0]["name"], "Kitchen",
        "HTTP success did not save metadata"
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &rename).await.status,
        409,
        "retired target authority was replaced with current authority"
    );
    let stale = command(
        1,
        None,
        "describe",
        json!({"expected_revision":2,"description":"stale"}),
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &stale).await.status,
        409,
        "stale controller generation admitted HTTP work"
    );
    let stale_revision = command(
        2,
        None,
        "describe",
        json!({"expected_revision":1,"description":"stale"}),
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &stale_revision)
            .await
            .status,
        409,
        "stale HTTP revision admitted work"
    );
    let mut loose = command(
        2,
        None,
        "describe",
        json!({"expected_revision":2,"description":"bad","extra":true}),
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &loose).await.status,
        400,
        "HTTP parameters bypassed strict command parser"
    );
    loose["params"].as_object_mut().unwrap().remove("extra");
    loose["unexpected"] = json!(true);
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &loose).await.status,
        400,
        "HTTP envelope accepted unknown fields"
    );
    let no_generation =
        json!({"command":"describe","params":{"expected_revision":2,"description":null}});
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &no_generation)
            .await
            .status,
        400
    );
    let list = fixture
        .request("GET", "/api/jobs", &[("Cookie", &cookie)], "")
        .await;
    assert_eq!(
        list.json()["jobs"].as_array().unwrap().len(),
        1,
        "refused HTTP requests created jobs"
    );
    assert!(
        !list.body.contains(fixture.path.to_str().unwrap()),
        "job records exposed a private path"
    );
    assert_eq!(
        fixture
            .request("GET", "/api/jobs/not-an-id", &[("Cookie", &cookie)], "")
            .await
            .status,
        404
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_jobs_authentication_csrf_and_body_wait_revocation_precede_admission() {
    let fixture = Fixture::new().await;
    let (cookie, csrf) = fixture.login().await;
    let body = command(
        1,
        None,
        "describe",
        json!({"expected_revision":1,"description":"never save"}),
    )
    .to_string();
    for path in ["/api/jobs", "/api/jobs/00000000000000000000000000000000"] {
        assert_eq!(
            fixture.request("GET", path, &[], "").await.status,
            401,
            "job inspection bypassed authentication"
        );
    }
    assert_eq!(
        fixture
            .request(
                "POST",
                "/api/jobs",
                &[("Cookie", &cookie), ("Content-Type", "application/json")],
                &body
            )
            .await
            .status,
        403,
        "job submission bypassed CSRF"
    );
    let mut pending = TcpStream::connect(fixture.server.address).await.unwrap();
    let headers=format!("POST /api/jobs HTTP/1.1\r\nHost: dali.test\r\nConnection: close\r\nCookie: {cookie}\r\nOrigin: http://dali.test\r\nX-CSRF-Token: {csrf}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",body.len());
    pending.write_all(headers.as_bytes()).await.unwrap();
    pending.write_all(&body.as_bytes()[..1]).await.unwrap();
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.handlers.available_permits() == 32 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("body upload never entered middleware");
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
    pending.write_all(&body.as_bytes()[1..]).await.unwrap();
    let mut response = String::new();
    timeout(
        Duration::from_secs(2),
        pending.read_to_string(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        response.starts_with("HTTP/1.1 401"),
        "session revoked during body upload admitted a command: {response}"
    );
    assert_eq!(
        fixture
            .handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .revision,
        1
    );
    let (cookie, _) = fixture.login().await;
    let jobs = fixture
        .request("GET", "/api/jobs", &[("Cookie", &cookie)], "")
        .await;
    assert!(jobs.json()["jobs"].as_array().unwrap().is_empty());
    fixture.close().await;
}

struct SaveHold {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    hooks: Hooks,
}
impl SaveHold {
    fn new() -> Self {
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let (inside, resume) = (entered.clone(), release.clone());
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == Step::AfterRename(Stage::Mapping) && !inside.swap(true, Ordering::SeqCst)
                {
                    let deadline = std::time::Instant::now() + Duration::from_secs(5);
                    while !resume.load(Ordering::SeqCst) {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "web held save was not released"
                        );
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(())
            })),
        };
        Self {
            entered,
            release,
            hooks,
        }
    }
    async fn entered(&self) {
        timeout(Duration::from_secs(1), async {
            while !self.entered.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("HTTP command never reached held save");
    }
}
impl Drop for SaveHold {
    fn drop(&mut self) {
        self.release.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn web_jobs_survive_request_and_session_closure() {
    let hold = SaveHold::new();
    let fixture = Fixture::configured(|s| s.with_config_test_hooks(hold.hooks.clone())).await;
    let (cookie, csrf) = fixture.login().await;
    let body = command(
        1,
        None,
        "describe",
        json!({"expected_revision":1,"description":"survived browser"}),
    );
    let reply = submit(&fixture, &cookie, &csrf, &body).await;
    assert_eq!(reply.status, 202);
    let path = reply.header("location").unwrap().to_owned();
    hold.entered().await;
    assert_eq!(
        fixture
            .request("GET", &path, &[("Cookie", &cookie)], "")
            .await
            .json()["status"],
        "running",
        "disconnect canceled the owned HTTP job"
    );
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
    let (cookie, _) = fixture.login().await;
    hold.release.store(true, Ordering::SeqCst);
    let result = finished(&fixture, &cookie, &path).await;
    assert_eq!(
        result["status"], "succeeded",
        "logout canceled an admitted HTTP job: {result}"
    );
    assert_eq!(
        fixture
            .handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .description
            .as_deref(),
        Some("survived browser")
    );
    assert!(
        fixture
            ._broker
            .wait_until(Duration::from_secs(1), |broker| broker
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(|message| String::from_utf8_lossy(&message.payload)
                    .contains("survived browser")))
            .await,
        "HTTP save bypassed shared inventory publication"
    );
    fixture.close().await;
}

async fn hardware_ready(fixture: &Fixture) {
    timeout(Duration::from_secs(3), async {
        loop {
            if matches!(
                fixture.handle.snapshot().unwrap().hardware,
                crate::service::Hardware::Ready { .. }
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("private emulator UART never became ready");
}
async fn run(fixture: &Fixture, cookie: &str, csrf: &str, body: Value) -> Value {
    let reply = submit(fixture, cookie, csrf, &body).await;
    assert_eq!(
        reply.status, 202,
        "typed HTTP command refused: {}",
        reply.body
    );
    let result = finished(fixture, cookie, reply.header("location").unwrap()).await;
    assert_eq!(
        result["status"], "succeeded",
        "typed HTTP job failed: {result}"
    );
    result["result"].clone()
}

#[tokio::test]
async fn web_jobs_dispatch_real_discovery_assignment_and_groups_without_mqtt_completion() {
    use crate::emulator::{pty, session::Control};
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "outputs":[{"id":"existing","short_address":0,"level":77},{"id":"new","short_address":null,"level":13}],
        "inputs":[{"id":"new-panel","short_address":null,"instances":[1,2,3,4]}]}]})).unwrap()).await.unwrap();
    let fixture = Fixture::configured(|s| {
        s.with_uart(
            emulator.path.clone(),
            crate::atx::transactions::RepeatPolicy::OneTerminal,
        )
    })
    .await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    let out = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            None,
            "assign_next_output",
            json!({"bus":0,"expected_revision":1}),
        ),
    )
    .await;
    assert_eq!(
        out["outcome"], "assigned",
        "HTTP output assignment fabricated completion"
    );
    assert_eq!(out["address"], 1);
    let input = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            None,
            "assign_next_input",
            json!({"bus":0,"expected_revision":2}),
        ),
    )
    .await;
    assert_eq!(input["outcome"], "assigned");
    assert_eq!(input["address"], 0);
    let discovered = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            3,
            None,
            "discover_address",
            json!({"bus":0,"space":"inputs","address":0,"expected_revision":3}),
        ),
    )
    .await;
    assert_eq!(
        discovered["added_items"], 4,
        "HTTP discovery did not add real input instances"
    );
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            4,
            None,
            "create_group",
            json!({"bus":0,"group":5,"name":"Dining","expected_revision":4}),
        ),
    )
    .await;
    let assigned = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            Some("PrivateBus.Outputs.PrivateLight"),
            "assign_group",
            json!({"group":5,"member":true,"expected_revision":5}),
        ),
    )
    .await;
    assert_eq!(
        assigned["outcome"], "verified",
        "HTTP group assignment did not reach hardware"
    );
    let groups = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            Some("PrivateBus.Outputs.PrivateLight"),
            "read_groups",
            json!({}),
        ),
    )
    .await;
    assert_eq!(groups["probe"]["mask"], 32);
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(physical["buses"][0]["outputs"][0]["groups"], 32);
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 77);
    assert_eq!(physical["buses"][0]["outputs"][1]["short_address"], 1);
    assert_eq!(physical["buses"][0]["inputs"][0]["short_address"], 0);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn web_jobs_capture_header_trace_and_authoritative_absence_across_the_queue() {
    let fixture = Fixture::new().await;
    let (cookie, csrf) = fixture.login().await;
    let trace = "00-11111111111111111111111111111111-2222222222222222-01";
    let body = command(
        1,
        None,
        "describe",
        json!({"expected_revision":1,"description":"traced"}),
    );
    let reply = fixture
        .request(
            "POST",
            "/api/jobs",
            &[
                ("Cookie", &cookie),
                ("Origin", "http://dali.test"),
                ("X-CSRF-Token", &csrf),
                ("Content-Type", "application/json"),
                ("traceparent", trace),
            ],
            &body.to_string(),
        )
        .await;
    assert_eq!(reply.status, 202);
    finished(&fixture, &cookie, reply.header("location").unwrap()).await;
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            None,
            "describe",
            json!({"expected_revision":2,"description":"headerless"}),
        ),
    )
    .await;
    let contexts = fixture
        .server
        .state
        .jobs
        .observed_contexts
        .lock()
        .unwrap()
        .clone();
    assert_eq!(contexts.len(), 2);
    assert_eq!(
        contexts[0].traceparent(),
        Some(trace),
        "HTTP queue lost its original trace header"
    );
    assert_eq!(
        contexts[1].traceparent(),
        None,
        "headerless HTTP job inherited an unrelated trace"
    );
    assert_eq!(
        contexts[0].generation(),
        Some(format!("{EPOCH}.1").as_str())
    );
    assert_eq!(
        contexts[1].generation(),
        Some(format!("{EPOCH}.2").as_str())
    );
    fixture.close().await;
}

#[tokio::test]
async fn web_jobs_capacity_and_passive_polling_preserve_session_expiry() {
    let fixture = Fixture::new().await;
    // Capacity seam owns exactly the same permits as admitted request futures.
    let held = fixture
        .server
        .state
        .jobs
        .slots
        .clone()
        .try_acquire_many_owned(4)
        .unwrap();
    let (cookie, csrf) = fixture.login().await;
    let body = command(
        1,
        None,
        "describe",
        json!({"expected_revision":1,"description":"busy"}),
    );
    assert_eq!(
        submit(&fixture, &cookie, &csrf, &body).await.status,
        503,
        "HTTP job admission ignored active capacity"
    );
    assert_eq!(
        fixture
            .handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .revision,
        1
    );
    drop(held);
    let result = run(&fixture, &cookie, &csrf, body).await;
    assert_eq!(result["revision"], 2);
    {
        let mut auth = fixture.server.state.auth.lock().unwrap();
        *auth = Auth::new(
            digest("secret"),
            Policy {
                idle: Duration::from_millis(300),
                absolute: Duration::from_secs(1),
                ..Policy::default()
            },
        );
    }
    let (cookie, _) = fixture.login().await;
    let path = fixture
        .request("GET", "/api/jobs", &[("Cookie", &cookie)], "")
        .await
        .json()["jobs"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let limit = Instant::now() + Duration::from_millis(500);
    let mut expired = false;
    while Instant::now() < limit {
        let reply = fixture
            .request(
                "GET",
                &format!("/api/jobs/{path}"),
                &[("Cookie", &cookie)],
                "",
            )
            .await;
        if reply.status == 401 {
            expired = true;
            break;
        }
        assert_eq!(reply.status, 200);
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert!(expired, "passive job polling renewed the session");
    fixture.close().await;
}

#[tokio::test]
async fn web_jobs_private_save_failures_and_uncertainty_are_inspectable_and_recoverable() {
    for uncertain in [false, true] {
        let once = Arc::new(AtomicBool::new(false));
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                let at = if uncertain {
                    Step::DirectorySync(Stage::Mapping)
                } else {
                    Step::BeforeRename(Stage::Mapping)
                };
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("private fixture secret /var/lib/dali/config.json".into())
                } else {
                    Ok(())
                }
            })),
        };
        let fixture = Fixture::configured(|s| s.with_config_test_hooks(hooks)).await;
        let (cookie, csrf) = fixture.login().await;
        let reply = submit(
            &fixture,
            &cookie,
            &csrf,
            &command(
                1,
                None,
                "describe",
                json!({"expected_revision":1,"description":"durable?"}),
            ),
        )
        .await;
        assert_eq!(reply.status, 202);
        let result = finished(&fixture, &cookie, reply.header("location").unwrap()).await;
        assert_eq!(result["status"], "failed");
        assert_eq!(
            result["error"]["code"],
            if uncertain { "uncertain" } else { "refused" }
        );
        assert!(
            !result.to_string().contains("private fixture secret"),
            "actual HTTP failure leaked private service error"
        );
        if uncertain {
            assert!(matches!(
                fixture.handle.snapshot().unwrap().phase,
                Phase::Uncertain(_)
            ));
            let recovered = run(
                &fixture,
                &cookie,
                &csrf,
                command(1, None, "recover", json!({})),
            )
            .await;
            assert_eq!(recovered["revision"], 2);
            assert_eq!(fixture.handle.snapshot().unwrap().phase, Phase::Ready);
        } else {
            assert_eq!(
                fixture
                    .handle
                    .snapshot()
                    .unwrap()
                    .model
                    .inventory()
                    .revision,
                1
            );
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn web_jobs_server_close_drops_pending_command_futures_with_retained_state() {
    let hold = SaveHold::new();
    let mut fixture = Fixture::configured(|s| s.with_config_test_hooks(hold.hooks.clone())).await;
    let (cookie, csrf) = fixture.login().await;
    assert_eq!(
        submit(
            &fixture,
            &cookie,
            &csrf,
            &command(
                1,
                None,
                "describe",
                json!({"expected_revision":1,"description":"pending shutdown"})
            )
        )
        .await
        .status,
        202
    );
    hold.entered().await;
    let retained = fixture.server.state.clone();
    // Keep HTTP graceful shutdown occupied independently of the job future.
    let mut upload = TcpStream::connect(fixture.server.address).await.unwrap();
    upload.write_all(b"POST /api/login HTTP/1.1\r\nHost: dali.test\r\nOrigin: http://dali.test\r\nContent-Type: application/json\r\nContent-Length: 8000\r\n\r\n{\"password\":").await.unwrap();
    timeout(Duration::from_secs(1), async {
        while fixture.server.state.handlers.available_permits() == 32 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    fixture.server.begin_stop();
    timeout(Duration::from_millis(200), async {
        while retained.jobs.slots.available_permits() != 4 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("web stop did not cancel command futures while HTTP was draining");
    assert!(
        fixture
            .server
            .close(Instant::now() + Duration::from_millis(300))
            .await,
        "web stop did not collect its command owner"
    );
    assert_eq!(
        retained.jobs.slots.available_permits(),
        4,
        "web shutdown retained a pending command future"
    );
    timeout(Duration::from_secs(1), async {
        while !matches!(
            fixture.handle.snapshot().unwrap().phase,
            Phase::Uncertain(_)
        ) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("web shutdown did not close the pending service receiver");
    hold.release.store(true, Ordering::SeqCst);
    drop(upload);
    fixture.close().await;
}
