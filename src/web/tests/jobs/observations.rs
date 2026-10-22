use super::*;
use crate::emulator::{pty, session::Control};
use axum::response::IntoResponse;
use futures_util::StreamExt;
use std::pin::Pin;

const OUTPUT: &str = "PrivateBus.Outputs.PrivateLight";

#[tokio::test]
async fn web_identification_jobs_share_real_blink_stop_and_rename_restoration() {
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":91}]}]})).unwrap()).await.unwrap();
    let fixture = Fixture::configured(|s| {
        s.with_uart(
            &emulator.path,
            crate::atx::transactions::RepeatPolicy::Unverified,
        )
    })
    .await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    let before = control(&emulator.emulator, Control::Snapshot).await["buses"].clone();
    let noop = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(OUTPUT), "stop_blink", json!({})),
    )
    .await;
    assert_eq!(noop["phase"], "restored");
    assert_eq!(
        control(&emulator.emulator, Control::Snapshot).await["buses"],
        before,
        "no-op Stop changed physical state"
    );
    for (target, revision, params) in [
        (None, 1, json!({})),
        (Some("PrivateBus.Groups.All"), 1, json!({})),
        (Some(OUTPUT), 1, json!({"extra":true})),
        (Some(OUTPUT), 99, json!({})),
    ] {
        assert_ne!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(revision, target, "blink", params)
            )
            .await
            .status,
            202,
            "HTTP identification accepted malformed or retired authority"
        );
    }
    let started = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(OUTPUT), "blink", json!({})),
    )
    .await;
    assert_eq!(
        started["phase"], "blinking",
        "HTTP Blink bypassed managed identification"
    );
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await
        .json();
    assert_eq!(
        snapshot["identification"]["phase"], "blinking",
        "HTTP snapshot omitted live identification"
    );
    assert_eq!(snapshot["identification"]["target"], OUTPUT);
    assert_eq!(
        control(&emulator.emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["level"],
        254
    );
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            Some(OUTPUT),
            "rename",
            json!({"expected_revision":1,"name":"Kitchen"}),
        ),
    )
    .await;
    timeout(Duration::from_secs(3), async {
        loop {
            if fixture.handle.snapshot().is_some_and(|s| {
                s.identification
                    .is_some_and(|i| i.phase == crate::service::identification::Stage::Restored)
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("HTTP rename did not restore identified fixture");
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", &cookie)], "")
        .await
        .json();
    assert_eq!(
        snapshot["identification"]["target"],
        "PrivateBus.Outputs.Kitchen"
    );
    assert_eq!(
        snapshot["identification"]["generation"],
        format!("{EPOCH}.2")
    );
    assert_eq!(snapshot["identification"]["actual_level"], 91);
    assert_eq!(
        control(&emulator.emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["level"],
        91
    );
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}
async fn state(fixture: &Fixture, cookie: &str, target: &str) -> Value {
    let snapshot = fixture
        .request("GET", "/api/snapshot", &[("Cookie", cookie)], "")
        .await
        .json();
    snapshot["states"]
        .as_array()
        .expect("HTTP snapshot omitted current observation projections")
        .iter()
        .find(|s| s["target"] == target)
        .expect("snapshot omitted the selected target observation")
        .clone()
}
async fn control(emulator: &crate::emulator::session::Emulator, action: Control) -> Value {
    emulator
        .control(action, Duration::from_secs(1))
        .await
        .unwrap()
}
async fn event(stream: &mut BufReader<TcpStream>, name: &str) -> Value {
    timeout(Duration::from_secs(1), async {
        loop {
            let chunk = chunk(stream)
                .await
                .expect("live stream ended before activity");
            if chunk.contains(&format!("event: {name}\n")) {
                return serde_json::from_str(
                    chunk
                        .lines()
                        .find_map(|l| l.strip_prefix("data: "))
                        .unwrap(),
                )
                .unwrap();
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("live SSE did not relay {name} activity"))
}

#[tokio::test]
async fn web_observed_output_jobs_change_real_lights_and_project_actual_values() {
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":77}]}]})).unwrap()).await.unwrap();
    let fixture = Fixture::configured(|s| {
        s.with_uart(
            emulator.path.clone(),
            crate::atx::transactions::RepeatPolicy::Unverified,
        )
    })
    .await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    let initial = state(&fixture, &cookie, OUTPUT).await;
    assert_eq!(initial["values"]["Available"], false);
    let applied = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            Some(OUTPUT),
            "apply_output",
            json!({"values":{"PowerState":true,"Brightness":0.4}}),
        ),
    )
    .await;
    let physical = control(&emulator.emulator, Control::Snapshot).await;
    let actual = physical["buses"][0]["outputs"][0]["level"]
        .as_f64()
        .unwrap()
        / 254.0;
    assert_eq!(applied["target"], OUTPUT);
    assert_eq!(applied["generation"], format!("{EPOCH}.1"));
    assert_eq!(
        applied["values"]["Brightness"],
        json!(actual),
        "HTTP apply echoed intent instead of actual level"
    );
    assert_ne!(applied["values"]["Brightness"], json!(0.4));
    assert_eq!(
        state(&fixture, &cookie, OUTPUT).await["values"],
        applied["values"]
    );
    let read = run(
        &fixture,
        &cookie,
        &csrf,
        command(1, Some(OUTPUT), "read_output", json!({})),
    )
    .await;
    assert_eq!(read["values"], applied["values"]);
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            Some(OUTPUT),
            "apply_output",
            json!({"values":{"PowerState":false}}),
        ),
    )
    .await;
    assert_eq!(
        control(&emulator.emulator, Control::Snapshot).await["buses"][0]["outputs"][0]["level"],
        0
    );
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn web_observed_native_input_actions_and_latest_state_reach_authenticated_sse() {
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"inputs":[{"id":"panel","short_address":0,"instances":[1]}]}]})).unwrap()).await.unwrap();
    let fixture = Fixture::configured(|s| {
        s.with_uart(
            emulator.path.clone(),
            crate::atx::transactions::RepeatPolicy::Unverified,
        )
    })
    .await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            None,
            "discover_address",
            json!({"bus":0,"space":"inputs","address":0,"expected_revision":1}),
        ),
    )
    .await;
    let projection = fixture
        .handle
        .snapshot()
        .unwrap()
        .model
        .projections()
        .into_iter()
        .find(|p| {
            matches!(
                p.location,
                crate::inventory::Location::InputInstance { instance: 0, .. }
            )
        })
        .unwrap();
    let (_, mut stream) = open(
        fixture.server.address,
        "GET",
        "/api/events",
        &[("Cookie", &cookie)],
        "",
    )
    .await;
    assert!(chunk(&mut stream)
        .await
        .unwrap()
        .contains("event: snapshot\n"));
    control(
        &emulator.emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x008001,
        },
    )
    .await;
    let pressed = event(&mut stream, "input").await;
    assert_eq!(pressed["action"], "pressed");
    assert_eq!(pressed["target"], projection.target);
    assert_eq!(
        pressed["generation"],
        format!("{EPOCH}.{}", projection.binding_revision)
    );
    assert_eq!(
        state(&fixture, &cookie, &projection.target).await["values"]["Status"],
        true
    );
    control(
        &emulator.emulator,
        Control::InputEvent {
            bus: 0,
            frame: 0x008000,
        },
    )
    .await;
    assert_eq!(event(&mut stream, "input").await["action"], "released");
    assert_eq!(
        state(&fixture, &cookie, &projection.target).await["values"]["Status"],
        false
    );
    drop(stream);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

async fn inputs() -> (Fixture, pty::PtyEmulator, String) {
    let emulator = pty::spawn(
        serde_json::from_value(json!({"version":1,"buses":[{"number":0,
        "inputs":[{"id":"panel","short_address":0,"instances":[1,2,2],"values":[
            {"instance":0,"resolution":1,"bytes":[255]},
            {"instance":1,"resolution":1,"bytes":[0]},
            {"instance":2,"resolution":255,"bytes":vec![255;32]}
        ]}]}]}))
        .unwrap(),
    )
    .await
    .unwrap();
    let fixture = Fixture::configured(|s| {
        s.with_uart(
            emulator.path.clone(),
            crate::atx::transactions::RepeatPolicy::Unverified,
        )
    })
    .await;
    hardware_ready(&fixture).await;
    let (cookie, csrf) = fixture.login().await;
    run(
        &fixture,
        &cookie,
        &csrf,
        command(
            1,
            None,
            "discover_address",
            json!({"bus":0,"space":"inputs","address":0,"expected_revision":1}),
        ),
    )
    .await;
    let target = fixture
        .handle
        .snapshot()
        .unwrap()
        .model
        .projections()
        .into_iter()
        .find(|p| {
            matches!(
                p.location,
                crate::inventory::Location::InputInstance { instance: 0, .. }
            )
        })
        .unwrap()
        .target;
    (fixture, emulator, target)
}
type Pull =
    Pin<Box<dyn futures_util::Stream<Item = Result<axum::body::Bytes, axum::Error>> + Send>>;
fn pull_stream(fixture: &Fixture, cookie: &str) -> Pull {
    let permit = fixture
        .server
        .state
        .streams
        .clone()
        .try_acquire_owned()
        .unwrap();
    let stream = handlers::event_stream(
        fixture.server.state.clone(),
        cookie.split_once('=').unwrap().1.to_owned(),
        permit,
    );
    Box::pin(
        axum::response::Sse::new(stream)
            .into_response()
            .into_body()
            .into_data_stream(),
    )
}
async fn pull(stream: &mut Pull) -> String {
    let bytes = timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("pull stream did not progress")
        .expect("pull stream ended")
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
async fn observe_native(
    emulator: &crate::emulator::session::Emulator,
    receive: &mut tokio::sync::broadcast::Receiver<crate::service::input::Notice>,
    frame: u32,
) {
    control(emulator, Control::InputEvent { bus: 0, frame }).await;
    let notice = timeout(Duration::from_secs(1), receive.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notice.frame.raw(), frame);
}

#[tokio::test]
async fn web_observed_input_queries_preserve_exact_resolution_and_do_not_create_actions() {
    let (fixture, emulator, target) = inputs().await;
    let (cookie, csrf) = fixture.login().await;
    let mut stream = pull_stream(&fixture, &cookie);
    pull(&mut stream).await;
    pull(&mut stream).await;
    let button = run(
        &fixture,
        &cookie,
        &csrf,
        command(2, Some(&target), "read_input", json!({})),
    )
    .await;
    assert_eq!(button["values"]["Status"], true);
    assert_eq!(button["values"]["ValueSource"], "query");
    assert_eq!(button["values"]["InputValueRaw"], "01");
    let projections = fixture.handle.snapshot().unwrap().model.projections();
    let contact = projections
        .iter()
        .find(|p| {
            matches!(
                p.location,
                crate::inventory::Location::InputInstance { instance: 1, .. }
            )
        })
        .unwrap();
    let read = run(
        &fixture,
        &cookie,
        &csrf,
        command(2, Some(&contact.target), "read_input", json!({})),
    )
    .await;
    assert_eq!(read["values"]["InputState"], false);
    let wide = projections
        .iter()
        .find(|p| {
            matches!(
                p.location,
                crate::inventory::Location::InputInstance { instance: 2, .. }
            )
        })
        .unwrap();
    let read = run(
        &fixture,
        &cookie,
        &csrf,
        command(2, Some(&wide.target), "read_input", json!({})),
    )
    .await;
    assert_eq!(read["values"]["ValueResolution"], 255);
    assert_eq!(
        read["values"]["InputValueRaw"],
        format!("7F{}", "FF".repeat(31)),
        "HTTP input read truncated full-resolution value"
    );
    assert_eq!(
        state(&fixture, &cookie, &wide.target).await["values"],
        read["values"]
    );
    for _ in 0..2 {
        let update = pull(&mut stream).await;
        assert!(
            !update.contains("event: input\n"),
            "input query fabricated a native action"
        );
    }
    assert!(
        timeout(Duration::from_millis(50), stream.next())
            .await
            .is_err(),
        "query produced extra action events"
    );
    drop(stream);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn web_observed_selected_colour_families_use_atomic_intents_and_actual_readback() {
    for (family, values) in [
        (
            "temperature",
            json!({"ColorTemperature":2700,"Brightness":0.6,"PowerState":true}),
        ),
        ("xy", json!({"ColorX":0.4})),
        (
            "rgbwaf",
            json!({"ColorRed":0.2,"ColorGreen":0.4,"ColorBlue":0.6}),
        ),
    ] {
        let mut scenario: Value =
            serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json"))
                .unwrap();
        scenario["buses"][0]["outputs"][0]["short_address"] = json!(0);
        scenario["buses"][0]["outputs"][0]["colour"]["auto_activation"] = json!(true);
        let emulator = pty::spawn(serde_json::from_value(scenario).unwrap())
            .await
            .unwrap();
        let fixture = Fixture::configured(|s| {
            s.with_uart(
                emulator.path.clone(),
                crate::atx::transactions::RepeatPolicy::Unverified,
            )
        })
        .await;
        hardware_ready(&fixture).await;
        let (cookie, csrf) = fixture.login().await;
        run(
            &fixture,
            &cookie,
            &csrf,
            command(
                1,
                Some(OUTPUT),
                "set_color_representation",
                json!({"expected_revision":1,"color_representation":family}),
            ),
        )
        .await;
        run(
            &fixture,
            &cookie,
            &csrf,
            command(2, Some(OUTPUT), "read_output", json!({})),
        )
        .await;
        let before = control(&emulator.emulator, Control::Snapshot).await;
        for invalid in [
            json!({"PowerState":false,"Bogus":1}),
            json!({"PowerState":false,"Brightness":"bad"}),
            json!({"ColorTemperature":null}),
            if family == "temperature" {
                json!({"ColorX":0.2})
            } else {
                json!({"ColorTemperature":3000})
            },
        ] {
            assert_eq!(
                submit(
                    &fixture,
                    &cookie,
                    &csrf,
                    &command(2, Some(OUTPUT), "apply_output", json!({"values":invalid}))
                )
                .await
                .status,
                400,
                "invalid HTTP output map was partially accepted"
            );
        }
        assert_eq!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(
                    2,
                    Some(OUTPUT),
                    "apply_output",
                    json!({"values":{"PowerState":false},"extra":true})
                )
            )
            .await
            .status,
            400,
            "HTTP apply accepted unknown parameter fields"
        );
        assert_eq!(
            submit(
                &fixture,
                &cookie,
                &csrf,
                &command(2, Some(OUTPUT), "read_output", json!({"extra":true}))
            )
            .await
            .status,
            400
        );
        assert_eq!(
            control(&emulator.emulator, Control::Snapshot).await["buses"],
            before["buses"],
            "invalid HTTP apply changed hardware"
        );
        let result = run(
            &fixture,
            &cookie,
            &csrf,
            command(2, Some(OUTPUT), "apply_output", json!({"values":values})),
        )
        .await;
        let physical = control(&emulator.emulator, Control::Snapshot).await;
        let actual = &physical["buses"][0]["outputs"][0]["colour_state"]["actual"];
        assert_eq!(actual["active"], family);
        assert_eq!(result["values"]["ColorAvailable"], true);
        match family {
            "temperature" => {
                assert_eq!(actual["temperature"]["value"], 370);
                assert_eq!(
                    result["values"]["ColorTemperature"],
                    json!(1_000_000.0 / 370.0),
                    "HTTP colour echoed requested temperature"
                );
            }
            "xy" => {
                assert_eq!(actual["xy"], json!([26214, 9029]));
                assert_eq!(result["values"]["ColorY"], json!(9029.0 / 65536.0));
            }
            _ => assert_eq!(
                actual["rgbwaf"],
                json!([51, 102, 152, 4, 5, 6]),
                "HTTP RGB changed unrequested WAF channels"
            ),
        }
        assert_eq!(
            state(&fixture, &cookie, OUTPUT).await["values"],
            result["values"]
        );
        fixture.close().await;
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn web_observed_stream_rotates_ready_sources_and_reports_native_loss_with_resnapshot() {
    let (fixture, emulator, _) = inputs().await;
    let (cookie, csrf) = fixture.login().await;
    let mut stream = pull_stream(&fixture, &cookie);
    assert!(pull(&mut stream).await.contains("event: snapshot\n"));
    assert!(pull(&mut stream).await.contains("event: jobs\n"));
    let mut notices = fixture.handle.subscribe_inputs();
    let mut kinds = Vec::new();
    for i in 0..3 {
        // Real saves keep both snapshot and job notifications ready each turn.
        run(
            &fixture,
            &cookie,
            &csrf,
            command(
                2 + i,
                None,
                "describe",
                json!({"expected_revision":2+i,"description":format!("fair-{i}")}),
            ),
        )
        .await;
        observe_native(&emulator.emulator, &mut notices, 0x008001).await;
        kinds.push(pull(&mut stream).await);
    }
    assert!(
        kinds.iter().any(|s| s.contains("event: input\n")),
        "ready snapshots and jobs starved native activity"
    );
    for data in kinds.iter().filter(|s| s.contains("event: input\n")) {
        let value: Value =
            serde_json::from_str(data.lines().find_map(|l| l.strip_prefix("data: ")).unwrap())
                .unwrap();
        assert_eq!(
            value["generation"],
            format!("{EPOCH}.2"),
            "web input action replaced its original generation with current controller metadata"
        );
    }
    assert!(
        kinds.iter().any(|s| s.contains("event: snapshot\n")),
        "native activity starved current snapshots"
    );
    assert!(
        kinds.iter().any(|s| s.contains("event: jobs\n")),
        "native activity starved job completion updates"
    );
    // A fresh paused receiver avoids a pending notice masking broadcast Lagged.
    drop(stream);
    for waiting in [false, true] {
        let mut stream = pull_stream(&fixture, &cookie);
        pull(&mut stream).await;
        pull(&mut stream).await;
        if waiting {
            std::future::poll_fn(|cx| {
                assert!(matches!(
                    stream.as_mut().poll_next(cx),
                    std::task::Poll::Pending
                ));
                std::task::Poll::Ready(())
            })
            .await;
        }
        for i in 0..80 {
            observe_native(&emulator.emulator, &mut notices, 0x008000 + (i % 2)).await;
        }
        let next = timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap();
        assert!(
            next.is_some(),
            "slow web input consumer hid native event loss by closing its stream"
        );
        let lost = String::from_utf8(next.unwrap().unwrap().to_vec()).unwrap();
        assert!(
            lost.contains("event: input_events_lost\n"),
            "slow web input consumer hid native event loss: {lost}"
        );
        assert!(lost.contains("\"dropped\":16"));
        assert!(
            pull(&mut stream).await.contains("event: snapshot\n"),
            "native event loss was not followed by a current snapshot"
        );
        drop(stream);
    }
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn web_observed_stream_drops_retired_receive_events_and_rechecks_session_for_pending_input() {
    let (fixture, emulator, target) = inputs().await;
    let (cookie, csrf) = fixture.login().await;
    let mut stream = pull_stream(&fixture, &cookie);
    pull(&mut stream).await;
    pull(&mut stream).await;
    let mut notices = fixture.handle.subscribe_inputs();
    observe_native(&emulator.emulator, &mut notices, 0x008001).await;
    for (revision, enabled) in [(2, false), (3, true)] {
        run(
            &fixture,
            &cookie,
            &csrf,
            command(
                revision,
                None,
                "configure_bus",
                json!({"expected_revision":revision,"bus":0,"name":"PrivateBus","enabled":enabled}),
            ),
        )
        .await;
    }
    for _ in 0..2 {
        let event = timeout(Duration::from_millis(300), pull(&mut stream))
            .await
            .expect("retired input notice stranded ready snapshot or job updates");
        assert!(
            !event.contains("event: input\n"),
            "web stream replayed an event from a retired receive lifetime"
        );
    }
    let value = state(&fixture, &cookie, &target).await;
    assert_eq!(value["values"]["Available"], false);
    assert_eq!(
        value["values"]["Status"],
        Value::Null,
        "web snapshot retained an old input state after receive cutover"
    );
    observe_native(&emulator.emulator, &mut notices, 0x008000).await;
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
    assert!(
        timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .is_none(),
        "revoked stream emitted pending native activity"
    );
    drop(stream);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[test]
fn web_observed_dense_valid_inventory_fits_the_measured_snapshot_budget() {
    let buses:Vec<_>=(0..4).map(|bus|json!({"bus":bus,"name":format!("Bus{bus}"),"enabled":true,
        "outputs":(0..64).map(|a|json!({"short_address":a,"name":format!("Light{a}"),"color_representation":"rgbwaf"})).collect::<Vec<_>>(),"inputs":[]})).collect();
    let mut inventory = json!({"schema_version":2,"revision":1,"inventory_epoch":EPOCH,"name":"Controller","buses":buses});
    let mut last = None;
    for index in 0..256 {
        let bus = index % 4;
        let address = index / 4;
        inventory["buses"][bus]["inputs"].as_array_mut().unwrap().push(json!({"short_address":address,"name":format!("Panel{address}"),"instances":(0..32).map(|n|json!({"number":n,"name":format!("Input{n}"),"instance_type":1+n%4})).collect::<Vec<_>>()}));
        match crate::inventory::Model::new(serde_json::from_value(inventory.clone()).unwrap()) {
            Ok(model) => last = Some(model),
            Err(_) => break,
        }
    }
    let model = last.expect("dense initial model is invalid");
    let inventory_bytes = model.state_bytes().unwrap().len();
    assert!(
        inventory_bytes > crate::inventory::MAX_STATE_BYTES * 9 / 10,
        "density probe did not approach the real inventory limit"
    );
    let expected = model
        .projections()
        .iter()
        .filter(|p| {
            matches!(
                p.location,
                crate::inventory::Location::Output { .. }
                    | crate::inventory::Location::InputInstance { .. }
            )
        })
        .count();
    let snapshot = crate::service::Snapshot {
        identification: None,
        model: Arc::new(model),
        phase: Phase::Ready,
        hardware: crate::service::Hardware::NotStarted,
        outputs: Default::default(),
        inputs: Default::default(),
        input_listeners: Default::default(),
        version: "test",
        build_utc: "test",
    };
    let encoded = handlers::encode_snapshot(&snapshot)
        .expect("dense valid inventory and shared observations exceed HTTP snapshot budget");
    let value: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        value["states"].as_array().unwrap().len(),
        expected,
        "dense snapshot omitted fixture projections"
    );
    assert!(encoded.len() <= handlers::SNAPSHOT_BYTES);
    println!(
        "dense snapshot: inventory={inventory_bytes} bytes, HTTP={} bytes, states={expected}",
        encoded.len()
    );
}

#[tokio::test]
async fn web_restoration_pending_press_stays_revoked_after_fresh_query_and_event() {
    let (fixture, emulator, button) = inputs().await;
    let (cookie, csrf) = fixture.login().await;
    let original = fixture.handle.snapshot().unwrap().model;
    let module = original
        .projections()
        .into_iter()
        .find(|p| {
            matches!(
                p.location,
                crate::inventory::Location::Input {
                    bus: 0,
                    short_address: 0
                }
            )
        })
        .unwrap()
        .target;
    let mut stream = pull_stream(&fixture, &cookie);
    assert!(pull(&mut stream).await.contains("event: snapshot\n"));
    let mut notices = fixture.handle.subscribe_inputs();
    observe_native(&emulator.emulator, &mut notices, 0x008001).await;
    // Prefetch holds the old press while the rotation emits its initial jobs row.
    assert!(pull(&mut stream).await.contains("event: jobs\n"));
    control(
        &emulator.emulator,
        Control::SetAddress {
            id: "panel".into(),
            short_address: None,
        },
    )
    .await;
    let result = run(
        &fixture,
        &cookie,
        &csrf,
        command(
            2,
            Some(&module),
            "restore_unaddressed_address",
            json!({"expected_revision":2}),
        ),
    )
    .await;
    assert_eq!(
        result["outcome"], "restored",
        "HTTP restoration did not complete: {result}"
    );
    assert_eq!(
        result["revision"], 2,
        "unique restoration changed saved identity metadata"
    );
    fixture
        .handle
        .read_input(
            &button,
            mqtt_bridge_kit::RequestContext::new(format!("{EPOCH}.2")).unwrap(),
            tokio::time::Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    observe_native(&emulator.emulator, &mut notices, 0x008000).await;
    let event = timeout(Duration::from_secs(2), async {
        loop {
            let data = pull(&mut stream).await;
            if data.contains("event: input\n") {
                break data;
            }
        }
    })
    .await
    .unwrap();
    let value: Value = serde_json::from_str(
        event
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value["action"], "released",
        "SSE replayed a pre-repair queued press after fresh activity"
    );
    assert_eq!(
        original.state_bytes().unwrap(),
        fixture
            .handle
            .snapshot()
            .unwrap()
            .model
            .state_bytes()
            .unwrap()
    );
    drop(stream);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn web_observed_job_notifications_include_running_then_completed_results() {
    let hold = SaveHold::new();
    let fixture = Fixture::configured(|s| s.with_config_test_hooks(hold.hooks.clone())).await;
    let (cookie, csrf) = fixture.login().await;
    let mut stream = pull_stream(&fixture, &cookie);
    pull(&mut stream).await;
    pull(&mut stream).await;
    let submitted = submit(
        &fixture,
        &cookie,
        &csrf,
        &command(
            1,
            None,
            "describe",
            json!({"expected_revision":1,"description":"watched job"}),
        ),
    )
    .await;
    assert_eq!(submitted.status, 202);
    hold.entered().await;
    async fn job_update(stream: &mut Pull) -> String {
        loop {
            let data = pull(stream).await;
            if data.contains("event: jobs\n") {
                return data;
            }
        }
    }
    let running = timeout(Duration::from_millis(500), job_update(&mut stream))
        .await
        .expect("job admission did not notify the live stream");
    assert!(running.contains("\"status\":\"running\""));
    hold.release.store(true, Ordering::SeqCst);
    finished(&fixture, &cookie, submitted.header("location").unwrap()).await;
    let complete = timeout(Duration::from_millis(500), job_update(&mut stream))
        .await
        .expect("job completion did not notify the live stream");
    assert!(complete.contains("\"status\":\"succeeded\""));
    drop(stream);
    fixture.close().await;
}

#[tokio::test]
async fn web_observed_busy_stream_sources_do_not_renew_idle_session() {
    let (fixture, emulator, _) = inputs().await;
    *fixture.server.state.auth.lock().unwrap() = Auth::new(
        digest("secret"),
        Policy {
            idle: Duration::from_millis(150),
            absolute: Duration::from_secs(1),
            ..Policy::default()
        },
    );
    let (cookie, _) = fixture.login().await;
    let mut stream = pull_stream(&fixture, &cookie);
    pull(&mut stream).await;
    pull(&mut stream).await;
    let mut receive = fixture.handle.subscribe_inputs();
    let limit = Instant::now() + Duration::from_millis(500);
    let mut expired = false;
    while Instant::now() < limit {
        observe_native(&emulator.emulator, &mut receive, 0x008001).await;
        fixture
            .server
            .state
            .jobs
            .changed
            .send_modify(|n| *n = n.wrapping_add(1));
        if timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .is_none()
        {
            expired = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        expired,
        "busy native/snapshot/job streams renewed an idle session"
    );
    drop(stream);
    fixture.close().await;
    emulator.emulator.close().await.unwrap();
}
