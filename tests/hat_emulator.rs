//! Standalone emulator acceptance. All endpoints and fixtures are private/local.
use mqtt_dali::atx::{
    serial::Serial,
    transactions::{BusReply, RepeatPolicy, Response, Transactions},
    wire::{Command, DaliFrame},
};
use serde_json::Value;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command as Process};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dali-hat-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::write(path.join("physical.json"),br#"{"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[{"id":"physical-A","short_address":3,"level":100}]}]}"#).unwrap();
        Self(path)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("control.sock")
    }
    fn command(&self) -> Process {
        let mut command = Process::new(env!("CARGO_BIN_EXE_dali_hat_emulator"));
        command
            .current_dir(&self.0)
            .arg("--scenario")
            .arg(self.0.join("physical.json"))
            .arg("--control-socket")
            .arg(self.socket())
            .env("LOG_DESTINATION", "")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .kill_on_drop(true);
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
async fn ready(child: &mut Child) -> Value {
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut line = String::new();
            assert!(
                reader.read_line(&mut line).await.unwrap() > 0,
                "emulator exited before ready"
            );
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if value["ready"] == true {
                    return value;
                }
            }
        }
    })
    .await
    .unwrap()
}
async fn request(fixture: &Fixture, request: Value) -> Value {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream = UnixStream::connect(fixture.socket()).await.unwrap();
        let mut bytes = serde_json::to_vec(&request).unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    })
    .await
    .unwrap()
}
async fn terminate(child: &mut Child) {
    assert_eq!(
        unsafe { libc::kill(child.id().unwrap() as i32, libc::SIGTERM) },
        0
    );
    assert!(tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .expect("emulator ignored SIGTERM")
        .unwrap()
        .success());
}

#[tokio::test]
async fn standalone_process_exposes_a_real_pty_and_private_mutable_physical_model() {
    let fixture = Fixture::new();
    let mut child = fixture.command().spawn().unwrap();
    let ready = ready(&mut child).await;
    assert_eq!(
        fs::metadata(fixture.socket()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let (link, mut events) = Transactions::start(
        Serial::open(ready["pty"].as_str().unwrap()).await.unwrap(),
        RepeatPolicy::OneTerminal,
    );
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    let query = |address: u16| Command::Send {
        bus: 0,
        frame: DaliFrame::Bits16(((address * 2 + 1) << 8) | 0xa0),
        repeat: false,
    };
    assert_eq!(
        link.execute(query(3), Duration::from_secs(1))
            .await
            .unwrap(),
        Response::Dali(vec![BusReply::Value(100)])
    );
    assert_eq!(
        request(
            &fixture,
            serde_json::json!({"op":"set_address","id":"physical-A","short_address":7})
        )
        .await["result"]["ok"],
        true
    );
    assert_eq!(
        link.execute(query(7), Duration::from_secs(1))
            .await
            .unwrap(),
        Response::Dali(vec![BusReply::Value(100)])
    );
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    assert_eq!(
        snapshot["result"]["buses"][0]["outputs"][0]["short_address"],
        7
    );
    assert!(request(&fixture, serde_json::json!({"op":"invented"})).await["error"].is_string());
    link.close().await.unwrap();
    terminate(&mut child).await;
    assert!(
        !fixture.0.join("dali.v2.json").exists(),
        "emulator coupled its fixture model to bridge inventory"
    );
}

async fn literal_frame(link: &Transactions<Serial>, frame: u16) -> BusReply {
    let response = link
        .execute(
            Command::Send {
                bus: 0,
                frame: DaliFrame::Bits16(frame),
                repeat: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    let Response::Dali(mut replies) = response else {
        panic!("literal frame had a non-DALI response");
    };
    assert_eq!(replies.len(), 1);
    replies.pop().unwrap()
}

#[tokio::test]
async fn gear_settings_typed_plans_roundtrip_through_standalone_hat() {
    use mqtt_dali::dali::gear::{settings::*, Destination, Query, ShortAddress};
    let fixture = Fixture::new();
    let mut child = fixture.command().spawn().unwrap();
    let ready = ready(&mut child).await;
    let (link, mut events) = Transactions::start(
        Serial::open(ready["pty"].as_str().unwrap()).await.unwrap(),
        RepeatPolicy::OneTerminal,
    );
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    let address = ShortAddress::new(3).unwrap();
    for (setting, query, expected) in [
        (
            Setting::Maximum(LimitLevel::new(80).unwrap()),
            Query::MaximumLevel,
            80,
        ),
        (
            Setting::Minimum(LimitLevel::new(5).unwrap()),
            Query::MinimumLevel,
            5,
        ),
        (
            Setting::PowerOn(PowerOnLevel::LastActive),
            Query::PowerOnLevel,
            255,
        ),
        (
            Setting::SystemFailure(SystemFailureLevel::Unchanged),
            Query::SystemFailureLevel,
            255,
        ),
        (
            Setting::FadeTime(FadeTime::new(15).unwrap()),
            Query::FadeTimeRate,
            0xf7,
        ),
        (
            Setting::FadeRate(FadeRate::new(1).unwrap()),
            Query::FadeTimeRate,
            0xf1,
        ),
    ] {
        for step in setting.plan(Destination::Short(address)) {
            assert_eq!(
                link.execute(
                    Command::Send {
                        bus: 0,
                        frame: DaliFrame::Bits16(step.frame),
                        repeat: step.repeat
                    },
                    Duration::from_secs(1)
                )
                .await
                .unwrap(),
                Response::Dali(vec![BusReply::NoResponse])
            );
        }
        let raw = literal_frame(&link, address.query(query)).await;
        assert_eq!(
            raw,
            BusReply::Value(expected),
            "typed settings plan failed actual standalone readback"
        );
    }
    assert_eq!(
        literal_frame(&link, 0x07a0).await,
        BusReply::Value(80),
        "maximum setting did not clamp the lit fixture through PTY"
    );
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    let gear = &snapshot["result"]["buses"][0]["outputs"][0];
    assert_eq!(gear["fade"], 0xf1);
    assert_eq!(gear["power_on_level"], 255);
    assert_eq!(gear["failure_level"], 255);
    link.close().await.unwrap();
    terminate(&mut child).await;
}

#[tokio::test]
async fn standalone_dt8_pty_activates_native_colour_with_input_traffic() {
    use mqtt_dali::atx::{
        monitor::Event,
        wire::{Kind, Packet},
    };
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("physical.json"),
        include_str!("../examples/emulator/colour-i4.json"),
    )
    .unwrap();
    let mut child = fixture.command().spawn().unwrap();
    let ready = ready(&mut child).await;
    let (link, mut events) = Transactions::start(
        Serial::open(ready["pty"].as_str().unwrap()).await.unwrap(),
        RepeatPolicy::OneTerminal,
    );
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();

    assert_eq!(literal_frame(&link, 0xc108).await, BusReply::NoResponse);
    assert_eq!(
        request(
            &fixture,
            serde_json::json!({"op":"input_event","bus":0,"frame":32769})
        )
        .await["result"]["ok"],
        true
    );
    assert_eq!(
        literal_frame(&link, 0x07f9).await,
        BusReply::Value(0xc3),
        "native input traffic cancelled the DT8 command through PTY"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if matches!(
                events.recv().await.unwrap(),
                Event::Packet {
                    packet: Ok(Packet {
                        bus: 0,
                        kind: Kind::Dali(DaliFrame::Bits24(0x008001))
                    }),
                    ..
                }
            ) {
                break;
            }
        }
    })
    .await
    .expect("native input event was lost during colour work");
    for frame in [
        0xa3c2, 0xc301, 0xc108, 0x07e7, 0xc108, 0x07e2, 0xa302, 0xc108,
    ] {
        assert_eq!(literal_frame(&link, frame).await, BusReply::NoResponse);
    }
    assert_eq!(literal_frame(&link, 0x07fa).await, BusReply::Value(1));
    assert_eq!(
        literal_frame(&link, 0x0798).await,
        BusReply::Value(144),
        "standalone Tc readback did not reflect configured clamp"
    );

    for frame in [
        0xa3fe, 0xc300, 0xc57f, 0xc108, 0x07eb, 0xa3ff, 0xc3ff, 0xc5ff, 0xc108, 0x07ec, 0xc108,
        0x07e2,
    ] {
        assert_eq!(literal_frame(&link, frame).await, BusReply::NoResponse);
    }
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    let output = &snapshot["result"]["buses"][0]["outputs"][0];
    assert_eq!(
        output["colour_state"]["actual"]["rgbwaf"],
        serde_json::json!([254, 0, 127, 4, 5, 6]),
        "standalone RGB cleared unrequested WAF values"
    );
    assert_eq!(output["colour_state"]["actual"]["active"], "rgbwaf");
    assert!(output["colour_state"]["temporary"]["kind"].is_null());
    assert_eq!(
        output["level"], 100,
        "standalone colour altered master brightness"
    );
    link.close().await.unwrap();
    terminate(&mut child).await;
}

#[tokio::test]
async fn dt7_typed_plans_roundtrip_through_standalone_hat_with_both_repeat_profiles() {
    use mqtt_dali::dali::gear::{settings, switching, Destination, ShortAddress};
    for (profile, policy) in [
        ("one_terminal", RepeatPolicy::OneTerminal),
        ("two_terminals", RepeatPolicy::TwoTerminals),
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("physical.json"),serde_json::to_vec(&serde_json::json!({"version":1,"clock":"frozen","repeat":profile,"buses":[{"number":0,"outputs":[
            {"id":"relay","short_address":3,"device_type":7,"switching":{"adjustable_thresholds":true}},
            {"id":"dimmer","short_address":4,"level":90}]}]})).unwrap()).unwrap();
        let mut child = fixture.command().spawn().unwrap();
        let ready = ready(&mut child).await;
        let (link, mut events) = Transactions::start(
            Serial::open(ready["pty"].as_str().unwrap()).await.unwrap(),
            policy,
        );
        tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        link.initialize(Duration::from_secs(1)).await.unwrap();
        let address = ShortAddress::new(3).unwrap();
        let minimum = settings::Setting::Minimum(settings::LimitLevel::new(1).unwrap())
            .plan(Destination::Short(address));
        for step in minimum {
            link.execute(
                Command::Send {
                    bus: 0,
                    frame: DaliFrame::Bits16(step.frame),
                    repeat: step.repeat,
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        }
        for (setting, query, expected) in [
            (
                switching::Setting::UpOn(switching::UpThreshold::new(80).unwrap()),
                switching::Query::UpOn,
                80,
            ),
            (
                switching::Setting::UpOff(switching::UpThreshold::new(255).unwrap()),
                switching::Query::UpOff,
                255,
            ),
            (
                switching::Setting::DownOn(switching::Threshold::Disabled),
                switching::Query::DownOn,
                255,
            ),
            (
                switching::Setting::DownOff(switching::Threshold::from_raw(30)),
                switching::Query::DownOff,
                30,
            ),
        ] {
            for step in setting.plan(Destination::Short(address)) {
                let response = link
                    .execute(
                        Command::Send {
                            bus: 0,
                            frame: DaliFrame::Bits16(step.frame),
                            repeat: step.repeat,
                        },
                        Duration::from_secs(1),
                    )
                    .await
                    .unwrap();
                assert!(
                    matches!(response,Response::Dali(values) if values.iter().all(|v|*v==BusReply::NoResponse))
                );
            }
            let [enable, frame] = query.plan(address);
            literal_frame(&link, enable).await;
            assert_eq!(
                literal_frame(&link, frame).await,
                BusReply::Value(expected),
                "DT7 typed setter/query failed standalone HAT readback"
            );
        }
        for (level, actual) in [(60, 0), (100, 254), (50, 254), (20, 0)] {
            literal_frame(&link, 0x0600 | level).await;
            assert_eq!(
                literal_frame(&link, 0x07a0).await,
                BusReply::Value(actual),
                "DT7 standalone actual was not the relay contact state"
            );
        }
        assert_eq!(literal_frame(&link, 0x07a2).await, BusReply::Value(1));
        assert_eq!(literal_frame(&link, 0x079a).await, BusReply::Value(254));
        assert_eq!(literal_frame(&link, 0x09a0).await, BusReply::Value(90));
        let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
        assert_eq!(
            snapshot["result"]["buses"][0]["outputs"][0]["switching_state"]["virtual_level"],
            20
        );
        assert_eq!(snapshot["result"]["buses"][0]["outputs"][0]["level"], 0);
        link.close().await.unwrap();
        terminate(&mut child).await;
    }
}

#[tokio::test]
async fn existing_control_path_is_never_replaced() {
    let fixture = Fixture::new();
    fs::write(fixture.socket(), b"keep this file").unwrap();
    let mut child = fixture.command().stdout(Stdio::null()).spawn().unwrap();
    assert!(!tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    assert_eq!(fs::read(fixture.socket()).unwrap(), b"keep this file");
}

async fn scene_steps(
    link: &Transactions<Serial>,
    steps: Vec<mqtt_dali::dali::scenes::Step>,
) -> Vec<BusReply> {
    use mqtt_dali::dali::scenes::Step;
    let mut values = Vec::new();
    for step in steps {
        let (frame, repeat, query) = match step {
            Step::Send(frame) => (frame, false, false),
            Step::Repeat(frame) => (frame, true, false),
            Step::Query(frame) => (frame, false, true),
        };
        let response = link
            .execute(
                Command::Send {
                    bus: 0,
                    frame: DaliFrame::Bits16(frame),
                    repeat,
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        let Response::Dali(replies) = response else {
            panic!("scene command had a non-DALI response")
        };
        assert_eq!(
            replies.len(),
            1,
            "scene repeat policy produced extra terminals"
        );
        if query {
            values.extend(replies);
        } else {
            assert_eq!(replies, [BusReply::NoResponse]);
        }
    }
    values
}

#[tokio::test]
async fn scene_operations_canonical_process_reads_and_recalls_standalone_hat() {
    use mqtt_test_broker::{FakeBroker, SendOptions};
    use serde_json::json;
    let fixture = Fixture::new();
    let mut slots = vec![255; 16];
    slots[0] = 42;
    fs::write(fixture.0.join("physical.json"),serde_json::to_vec(&json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"level":100,"scenes":slots}]}]})).unwrap()).unwrap();
    fs::write(fixture.0.join("dali.json"),br#"{"name":"Controller","buses":[{"bus":0,"description":"Bus1","status":"Active","channels":[{"short_address":0,"description":"Light1"}]}]}"#).unwrap();
    fs::write(
        fixture.0.join("logging.toml"),
        "[logging]\ndestination=\"\"\n",
    )
    .unwrap();
    let mut emulator = fixture.command().spawn().unwrap();
    let hat = ready(&mut emulator).await;
    let broker = FakeBroker::start().await;
    let mut command = Process::new(env!("CARGO_BIN_EXE_mqtt_dali"));
    command
        .current_dir(&fixture.0)
        .args([
            "--instance",
            "Controller",
            "--broker",
            &broker.address(),
            "--uart",
            hat["pty"].as_str().unwrap(),
            "--log-config",
        ])
        .arg(fixture.0.join("logging.toml"))
        .env("LOG_DESTINATION", "")
        .env_remove("LOG_CONFIG")
        .env_remove("MQTT_BRIDGE_TEST_LOGGING_GATE")
        .env_remove("MQTT_DALI_V2_TEST_CONFIG_GATE")
        .env_remove("MQTT_DALI_V2_TEST_SAVE_GATE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut daemon = command.spawn().unwrap();
    assert!(
        broker
            .wait_until(Duration::from_secs(5), |b| !b
                .received_on("DALI/State/Controller/__inventory")
                .is_empty())
            .await
    );
    let first: Value = serde_json::from_slice(
        &broker
            .received_on("DALI/State/Controller/__inventory")
            .last()
            .unwrap()
            .payload,
    )
    .unwrap();
    let epoch = first["Inventory"]["inventory_epoch"].as_str().unwrap();
    let root = format!("{epoch}.1");
    // Startup inventory is available before UART initialization. Retry a read,
    // never a recall, until actual stored evidence proves the PTY is ready.
    tokio::time::timeout(Duration::from_secs(8), async {
        for attempt in 0.. {
            let topic = format!("test/standalone-scene/read/{attempt}");
            assert!(broker.publish(
                "DALI/Command/Controller",
                json!({"command":"read_scene","target":"Bus1.Outputs.Light1","slot":0}).to_string(),
                &SendOptions::default()
                    .response_topic(&topic)
                    .user_property("target-generation", &root)
            ));
            assert!(
                broker
                    .wait_until(Duration::from_secs(2), |b| !b
                        .received_on(&topic)
                        .is_empty())
                    .await
            );
            let reply: Value =
                serde_json::from_slice(&broker.received_on(&topic)[0].payload).unwrap();
            if reply.get("error").is_none() {
                assert_eq!(
                    reply["value"]["stored"]["level"],
                    json!({"kind":"value","raw":42}),
                    "canonical process did not read standalone stored scene"
                );
                break;
            }
            assert!(
                reply["error"]
                    .as_str()
                    .unwrap()
                    .contains("UART or bus is not ready"),
                "unexpected startup scene read error: {reply}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let physical = request(&fixture, json!({"op":"snapshot"})).await;
    assert_eq!(physical["result"]["buses"][0]["outputs"][0]["level"], 100);
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":"create_scene","bus":0,"slot":0,"name":"Evening","expected_revision":1})
            .to_string(),
        &SendOptions::default()
            .response_topic("test/standalone-scene/create")
            .user_property("target-generation", &root)
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/standalone-scene/create")
                .is_empty())
            .await
    );
    let created: Value =
        serde_json::from_slice(&broker.received_on("test/standalone-scene/create")[0].payload)
            .unwrap();
    assert_eq!(created["value"]["revision"], 2);
    assert!(broker.publish(
        "DALI/Command/Controller",
        json!({"command":"recall_scene","target":"Bus1.Scenes.Evening","expected_revision":2})
            .to_string(),
        &SendOptions::default()
            .response_topic("test/standalone-scene/recall")
            .user_property("target-generation", &format!("{epoch}.2"))
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/standalone-scene/recall")
                .is_empty())
            .await
    );
    let recalled: Value =
        serde_json::from_slice(&broker.received_on("test/standalone-scene/recall")[0].payload)
            .unwrap();
    assert_eq!(
        recalled["value"]["outcome"], "sent",
        "canonical process did not recall standalone scene: {recalled}"
    );
    let physical = request(&fixture, json!({"op":"snapshot"})).await;
    assert_eq!(physical["result"]["buses"][0]["outputs"][0]["level"], 42);
    terminate(&mut daemon).await;
    terminate(&mut emulator).await;
}

#[tokio::test]
async fn standalone_native_scene_codec_programs_and_queries_physical_colour_over_pty() {
    use mqtt_dali::dali::{
        colour,
        gear::{Destination, ShortAddress},
        scenes,
    };
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("physical.json"),
        include_str!("../examples/emulator/colour-i4.json"),
    )
    .unwrap();
    let mut child = fixture.command().spawn().unwrap();
    let ready = ready(&mut child).await;
    let (link, mut events) = Transactions::start(
        Serial::open(ready["pty"].as_str().unwrap()).await.unwrap(),
        RepeatPolicy::OneTerminal,
    );
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    link.initialize(Duration::from_secs(1)).await.unwrap();
    let address = ShortAddress::new(3).unwrap();
    let slot = scenes::Slot::new(15).unwrap();
    for frame in scenes::Colour::Temperature(Some(colour::Temperature::new(350).unwrap()))
        .staged_frames(address)
    {
        assert_eq!(literal_frame(&link, frame).await, BusReply::NoResponse);
    }
    scene_steps(&link, slot.store(address, scenes::Level::Mask)).await;
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    let output = &snapshot["result"]["buses"][0]["outputs"][0];
    assert_eq!(output["level"], 100);
    assert_eq!(
        output["colour_state"]["actual"]["temperature"]["value"], 250,
        "programming a native scene activated physical colour"
    );
    assert_eq!(
        scene_steps(&link, scenes::ReportValue::Type.plan(address, slot)).await,
        [BusReply::Value(255), BusReply::Value(0x20)],
        "native scene did not store colour-only participation"
    );
    assert_eq!(
        scene_steps(&link, scenes::ReportValue::Temperature.plan(address, slot)).await,
        [
            BusReply::Value(255),
            BusReply::Value(1),
            BusReply::Value(94)
        ]
    );
    literal_frame(&link, slot.recall(Destination::Short(address))).await;
    // The fixture's manual mode needs explicit ACTIVATE after native recall.
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    assert_eq!(
        snapshot["result"]["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]
            ["value"],
        250
    );
    literal_frame(&link, 0xc108).await;
    literal_frame(&link, 0x07e2).await;
    let snapshot = request(&fixture, serde_json::json!({"op":"snapshot"})).await;
    let output = &snapshot["result"]["buses"][0]["outputs"][0];
    assert_eq!(
        output["level"], 100,
        "colour-only recall changed physical brightness"
    );
    assert_eq!(
        output["colour_state"]["actual"]["temperature"]["value"],
        350
    );
    scene_steps(&link, vec![slot.remove(address)]).await;
    assert_eq!(
        scene_steps(&link, scenes::ReportValue::Type.plan(address, slot)).await,
        [BusReply::Value(255), BusReply::Value(255)],
        "native Remove kept physical scene colour"
    );
    link.close().await.unwrap();
    terminate(&mut child).await;
}

fn full_pipe() -> (fs::File, fs::File) {
    use std::io::Write;
    let mut fd = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fd.as_mut_ptr()) }, 0);
    let reader = unsafe { fs::File::from_raw_fd(fd[0]) };
    let mut writer = unsafe { fs::File::from_raw_fd(fd[1]) };
    for fd in [reader.as_raw_fd(), writer.as_raw_fd()] {
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }
    let flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    while writer.write(b"x").is_ok() {}
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags) },
        0
    );
    (reader, writer)
}
#[tokio::test]
async fn full_output_and_unterminated_controls_cannot_hold_process_shutdown() {
    let fixture = Fixture::new();
    let (_unread, writer) = full_pipe();
    let mut child = fixture
        .command()
        .stdout(Stdio::from(writer.try_clone().unwrap()))
        .stderr(Stdio::from(writer))
        .spawn()
        .unwrap();
    let mut client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(stream) = UnixStream::connect(fixture.socket()).await {
                break stream;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    client.write_all(b"{\"op\":").await.unwrap();
    // The startup JSON is still held in a full pipe; both raw output workers and
    // an unfinished control client must be abandoned within the stop bound.
    terminate(&mut child).await;
}

#[tokio::test]
async fn private_controls_reject_oversized_requests_and_retire_idle_connections() {
    let fixture = Fixture::new();
    let mut child = fixture.command().spawn().unwrap();
    ready(&mut child).await;
    let mut stream = UnixStream::connect(fixture.socket()).await.unwrap();
    stream.write_all(&vec![b'x'; 16385]).await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert!(serde_json::from_str::<Value>(&line).unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("16384"));
    let stream = UnixStream::connect(fixture.socket()).await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line))
            .await
            .expect("unterminated control was never closed")
            .unwrap(),
        0
    );
    assert!(
        request(&fixture, serde_json::json!({"op":"snapshot"})).await["result"]["buses"].is_array()
    );
    terminate(&mut child).await;
}
