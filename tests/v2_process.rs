//! Canonical v2 subprocess acceptance. Only ephemeral loopback endpoints/PTYs.
use mqtt_test_broker::{FakeBroker, SendOptions};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time::{timeout, Instant},
};
const NAME: &str = "Controller";
const LEGACY:&[u8]=br#"{"name":"Controller","buses":[{"bus":0,"description":"Bus1","status":"Active","channels":[{"short_address":0,"description":"Light1"}]}]}"#;
#[path = "support/otel_collector.rs"]
mod otel;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "dali-v2-process-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("dali.json"), LEGACY).unwrap();
        fs::write(directory.join("logging.toml"),"[logging]\ndestination=\"f\"\non_destination_error=\"skip\"\n[logging.gelf]\naddress=\"127.0.0.1:9\"\n[logging.otel]\nendpoint=\"http://127.0.0.1:9\"\ntransport=\"http\"\n").unwrap();
        Self(directory)
    }
    fn command(&self, broker: &FakeBroker) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mqtt_dali"));
        for (name, _) in std::env::vars_os() {
            let key = name.to_string_lossy();
            if key.starts_with("LOG_")
                || key.starts_with("OTEL_")
                || key.starts_with("GELF_")
                || key.to_ascii_lowercase().ends_with("_proxy")
            {
                command.env_remove(name);
            }
        }
        command
            .current_dir(&self.0)
            .args([
                "--instance",
                NAME,
                "--broker",
                &broker.address(),
                "--uart",
                "none",
                "--log-config",
            ])
            .arg(self.0.join("logging.toml"))
            .env_remove("LOG_CONFIG")
            .env_remove("LOG_DESTINATION")
            .env_remove("LOG_LEVEL")
            .env("RUST_LOG", "info")
            .env_remove("MQTT_BRIDGE_TEST_LOGGING_GATE")
            .env_remove("MQTT_DALI_V2_TEST_CONFIG_GATE")
            .env_remove("MQTT_DALI_V2_TEST_SAVE_GATE")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        command
    }
    fn config(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("dali.v2.json")).unwrap()).unwrap()
    }
    fn log(&self) -> String {
        fs::read_dir(self.0.join("logs"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| fs::read_to_string(entry.path()).unwrap_or_default())
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
async fn exit(child: &mut Child) -> ExitStatus {
    timeout(Duration::from_secs(12), child.wait())
        .await
        .expect("v2 daemon exceeded its process exit bound")
        .unwrap()
}
async fn stop(child: &mut Child) -> ExitStatus {
    assert_eq!(
        unsafe { libc::kill(child.id().unwrap() as i32, libc::SIGTERM) },
        0
    );
    exit(child).await
}
async fn inventory(broker: &FakeBroker) -> Value {
    assert!(
        broker
            .wait_until(Duration::from_secs(4), |broker| !broker
                .received_on("DALI/State/Controller/__inventory")
                .is_empty())
            .await,
        "canonical executable did not enter the v2 BridgeKit inventory runtime"
    );
    serde_json::from_slice::<Value>(
        &broker
            .received_on("DALI/State/Controller/__inventory")
            .last()
            .unwrap()
            .payload,
    )
    .unwrap()["Inventory"]
        .clone()
}

#[tokio::test]
async fn offline_mode_name_is_a_literal_daemon_option_value() {
    for inline in [false, true] {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("dali.json"),
            br#"{"name":"--check-config","buses":[]}"#,
        )
        .unwrap();
        let broker = FakeBroker::start().await;
        let mut command = fixture.command(&broker);
        if inline {
            command.arg("--instance=--check-config");
        } else {
            command.args(["--instance", "--check-config"]);
        }
        let mut child = command.spawn().unwrap();
        assert!(
            broker
                .wait_until(Duration::from_secs(4), |broker| !broker
                    .received_on("DALI/State/--check-config/__inventory")
                    .is_empty())
                .await,
            "mode flag inside option value redirected daemon"
        );
        assert!(stop(&mut child).await.success());
    }
}

#[tokio::test]
async fn uart_quiet_option_rejects_invalid_intervals_before_starting_resources() {
    let broker = FakeBroker::start().await;
    for value in ["0", "5001", "invalid", "-1", "0.5"] {
        let fixture = Fixture::new();
        let mut command = fixture.command(&broker);
        command
            .args(["--uart-quiet-ms", value])
            .stderr(Stdio::piped());
        let output = timeout(Duration::from_secs(4), command.output())
            .await
            .expect("invalid quiet interval started resources instead of failing preflight")
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("--uart-quiet-ms must be an integer between 1 and 5000"),
            "invalid interval did not reach the explicit preflight validator"
        );
        assert!(!fixture.0.join("dali.v2.json").exists());
        assert!(!fixture.0.join("logs").exists());
        assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
    }
}

#[tokio::test]
async fn canonical_daemon_imports_v1_and_serves_bridgekit_inventory_without_legacy_topics() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let mut child = fixture.command(&broker).spawn().unwrap();
    let inventory = inventory(&broker).await;
    assert_eq!(inventory["name"], NAME);
    assert_eq!(inventory["buses"][0]["outputs"][0]["name"], "Light1");
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
    assert!(broker.received_on("DALI/Config/Controller").is_empty());
    assert!(
        stop(&mut child).await.success(),
        "clean v2 daemon did not stop cleanly: {}",
        fixture.log()
    );
    assert!(
        fixture.log().contains("Bridge stopped"),
        "shared process shutdown was not logged"
    );
    let version = broker.received_on("DALI/Version/Controller");
    assert!(
        version
            .last()
            .is_some_and(|value| String::from_utf8_lossy(&value.payload)
                .starts_with("mqtt_dali 2.0.0-dev.0")),
        "retained Version did not identify v2 executable/build"
    );
}

async fn rename(broker: &FakeBroker, inventory: &Value) -> String {
    let generation = format!(
        "{}.{}",
        inventory["inventory_epoch"].as_str().unwrap(),
        inventory["revision"]
    );
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(broker.publish("DALI/Command/Controller",json!({"command":"rename","target":"Bus1.Outputs.Light1","name":"Kitchen","expected_revision":1}).to_string(),&SendOptions::default().user_property("target-generation",&generation).response_topic("test/reply").correlation_data("rename")));
    generation
}
#[tokio::test]
async fn canonical_rename_replies_once_and_restart_prefers_v2_while_delete_forces_reimport() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let mut child = fixture.command(&broker).spawn().unwrap();
    let first = inventory(&broker).await;
    let generation = rename(&broker, &first).await;
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/reply")
                .is_empty())
            .await,
        "canonical rename did not reply"
    );
    let replies = broker.received_on("test/reply");
    assert_eq!(replies.len(), 1);
    let reply = &replies[0];
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.payload).unwrap()["value"]["revision"],
        2
    );
    assert_eq!(
        reply.user_property("target-generation"),
        Some(generation.as_str())
    );
    assert_eq!(reply.correlation_data().unwrap().as_ref(), b"rename");
    assert!(stop(&mut child).await.success());
    assert_eq!(
        fixture.config()["buses"][0]["outputs"][0]["name"],
        "Kitchen"
    );
    fs::write(fixture.0.join("dali.json"), b"unread legacy bytes").unwrap();
    let broker = FakeBroker::start().await;
    let mut child = fixture.command(&broker).spawn().unwrap();
    let second = inventory(&broker).await;
    assert_eq!(second["buses"][0]["outputs"][0]["name"], "Kitchen");
    assert_eq!(second["inventory_epoch"], first["inventory_epoch"]);
    assert!(stop(&mut child).await.success());
    assert_eq!(
        fs::read(fixture.0.join("dali.json")).unwrap(),
        b"unread legacy bytes"
    );
    fs::write(fixture.0.join("dali.json"), LEGACY).unwrap();
    fs::remove_file(fixture.0.join("dali.v2.json")).unwrap();
    let broker = FakeBroker::start().await;
    let mut child = fixture.command(&broker).spawn().unwrap();
    let third = inventory(&broker).await;
    assert_eq!(third["buses"][0]["outputs"][0]["name"], "Light1");
    assert_ne!(third["inventory_epoch"], first["inventory_epoch"]);
    assert!(stop(&mut child).await.success());
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
}

#[tokio::test]
async fn malformed_or_mismatched_configuration_never_reaches_the_uart() {
    for malformed in [false, true] {
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        if malformed {
            fs::write(fixture.0.join("dali.v2.json"), b"invalid v2").unwrap();
        } else {
            fs::write(
                fixture.0.join("dali.json"),
                br#"{"name":"OtherController","buses":[]}"#,
            )
            .unwrap();
        }
        let mut pty = Pty::new();
        let mut child = fixture
            .command(&broker)
            .args(["--uart", pty.path.to_str().unwrap()])
            .spawn()
            .unwrap();
        assert!(
            !exit(&mut child).await.success(),
            "invalid identity/config started the daemon"
        );
        let mut byte = [0];
        assert!(
            timeout(
                Duration::from_millis(80),
                tokio::io::AsyncReadExt::read(&mut pty.peer, &mut byte)
            )
            .await
            .is_err(),
            "UART work preceded configuration validation"
        );
        assert!(broker
            .received_on("DALI/State/Controller/__inventory")
            .is_empty());
    }
}
struct Pty {
    peer: mqtt_dali::atx::serial::Serial,
    _slave: std::os::fd::OwnedFd,
    path: PathBuf,
}
impl Pty {
    fn new() -> Self {
        use std::{
            ffi::CStr,
            os::fd::{FromRawFd, OwnedFd},
        };
        let (mut master, mut slave) = (-1, -1);
        let mut name = [0 as libc::c_char; 1024];
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    name.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        Self {
            peer: mqtt_dali::atx::serial::Serial::from_owned_fd(unsafe {
                OwnedFd::from_raw_fd(master)
            })
            .unwrap(),
            _slave: unsafe { OwnedFd::from_raw_fd(slave) },
            path: PathBuf::from(unsafe { CStr::from_ptr(name.as_ptr()) }.to_str().unwrap()),
        }
    }
}
fn fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
}
async fn entered_gate(path: &Path) -> fs::File {
    use std::os::unix::fs::OpenOptionsExt;
    timeout(Duration::from_secs(5), async {
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(path)
            {
                Ok(file) => return file,
                Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                    tokio::time::sleep(Duration::from_millis(5)).await
                }
                Err(error) => panic!("FIFO gate failed: {error}"),
            }
        }
    })
    .await
    .expect("daemon did not begin its held filesystem operation")
}
#[tokio::test]
async fn sigterm_during_config_start_is_bounded_and_reports_incomplete_stop() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let gate = fixture.0.join("config-gate");
    fifo(&gate);
    let mut child = fixture
        .command(&broker)
        .env("MQTT_DALI_V2_TEST_CONFIG_GATE", &gate)
        .spawn()
        .unwrap();
    let hold = entered_gate(&gate).await;
    let before = Instant::now();
    let status = stop(&mut child).await;
    drop(hold);
    assert!(
        !status.success(),
        "blocked managed config owner was reported as a clean stop"
    );
    assert!(before.elapsed() < Duration::from_secs(9));
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
    assert!(!fixture.0.join("dali.v2.json").exists());
}
#[tokio::test]
async fn sigterm_during_saved_mapping_keeps_v1_and_reports_incomplete_stop() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let gate = fixture.0.join("save-gate");
    fifo(&gate);
    let mut child = fixture
        .command(&broker)
        .env("MQTT_DALI_V2_TEST_SAVE_GATE", &gate)
        .spawn()
        .unwrap();
    let initial = inventory(&broker).await;
    rename(&broker, &initial).await;
    let hold = entered_gate(&gate).await;
    assert_eq!(
        fixture.config()["buses"][0]["outputs"][0]["name"],
        "Kitchen"
    );
    let status = stop(&mut child).await;
    drop(hold);
    assert!(
        !status.success(),
        "blocked mapping owner was reported as a clean stop"
    );
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
    assert!(
        !broker
            .received_on("test/reply")
            .iter()
            .any(|message| serde_json::from_slice::<Value>(&message.payload)
                .is_ok_and(|reply| reply["value"]["revision"] == 2)),
        "uncertain save returned a successful commit"
    );
}
#[cfg(feature = "process-test-seams")]
#[tokio::test]
async fn sigterm_during_shared_logging_start_never_opens_config_or_uart() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let gate = fixture.0.join("logging-gate");
    fifo(&gate);
    let mut child = fixture
        .command(&broker)
        .env("MQTT_BRIDGE_TEST_LOGGING_GATE", &gate)
        .spawn()
        .unwrap();
    let hold = entered_gate(&gate).await;
    let before = Instant::now();
    let status = stop(&mut child).await;
    drop(hold);
    assert!(status.success());
    assert!(
        before.elapsed() < Duration::from_secs(3),
        "shared logging start held process termination"
    );
    assert!(!fixture.0.join("dali.v2.json").exists());
    assert_eq!(broker.connections(), 0);
}

#[tokio::test]
async fn canonical_daemon_propagates_command_traceparent_to_reply_and_local_otlp_collector() {
    let collector = otel::Collector::start().await;
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    fs::write(fixture.0.join("logging.toml"),format!("[logging]\ndestination=\"fo\"\non_destination_error=\"skip\"\n[logging.gelf]\naddress=\"127.0.0.1:9\"\n[logging.otel]\nendpoint=\"{}\"\ntransport=\"http\"\n",collector.endpoint)).unwrap();
    let mut child = fixture.command(&broker).spawn().unwrap();
    let inventory = inventory(&broker).await;
    let generation = format!(
        "{}.{}",
        inventory["inventory_epoch"].as_str().unwrap(),
        inventory["revision"]
    );
    let trace = "00-112233445566778899aabbccddeeff00-1234567890abcdef-01";
    assert!(broker.publish(
        "DALI/Command/Controller",
        r#"{"command":"describe","description":"traced","expected_revision":1}"#,
        &SendOptions::default()
            .user_property("target-generation", &generation)
            .user_property("traceparent", trace)
            .response_topic("trace/reply")
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("trace/reply")
                .is_empty())
            .await
    );
    let reply = broker.received_on("trace/reply");
    assert_eq!(
        serde_json::from_slice::<Value>(&reply[0].payload).unwrap()["value"]["revision"],
        2,
        "traced edit did not commit"
    );
    let traceparent = reply[0].user_property("traceparent").unwrap_or_default();
    assert!(
        traceparent.starts_with("00-112233445566778899aabbccddeeff00-"),
        "command reply lost the incoming trace: {traceparent}"
    );
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |broker| broker
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(|state| serde_json::from_slice::<Value>(&state.payload)
                    .is_ok_and(|state| state["Inventory"]["revision"] == 2)))
            .await,
        "traced edit did not publish committed inventory"
    );
    let states = broker.received_on("DALI/State/Controller/__inventory");
    let committed = states
        .iter()
        .find(|state| {
            serde_json::from_slice::<Value>(&state.payload)
                .is_ok_and(|state| state["Inventory"]["revision"] == 2)
        })
        .expect("traced edit did not publish committed inventory");
    assert!(
        committed
            .user_property("traceparent")
            .is_some_and(|parent| parent.starts_with("00-112233445566778899aabbccddeeff00-")),
        "committed Inventory lost the original command trace"
    );
    assert_ne!(
        committed.user_property("target-generation"),
        Some(generation.as_str()),
        "traced Inventory reused retired generation"
    );
    assert!(stop(&mut child).await.success());
    let expected = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        0x00,
    ];
    assert!(
        collector.has_operation(&expected, "describe"),
        "local OTLP collector never received the worker command operation under the incoming trace: {:?}", collector.summary()
    );
    assert!(
        collector.has_operation(&expected, "inventory_change"),
        "metadata actor lost the incoming command trace across its queue"
    );
}

async fn uart_line(peer: &mut mqtt_dali::atx::serial::Serial) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    timeout(Duration::from_secs(3), async {
        let mut line = Vec::new();
        loop {
            let byte = peer.read_u8().await.unwrap();
            line.push(byte);
            if byte == b'\n' {
                return line;
            }
            assert!(line.len() < 32);
        }
    })
    .await
    .expect("daemon UART health query was missing")
}

#[tokio::test]
async fn configured_uart_quiet_recovers_a_killed_process_without_boot() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fixture = Fixture::new();
    let mut pty = Pty::new();
    for restart in [false, true] {
        // Separate ephemeral brokers ensure inventory is from this process.
        let broker = FakeBroker::start().await;
        if restart {
            // An old complete reply buffered while the bridge was stopped.
            pty.peer.write_all(b"V991104\n").await.unwrap();
        }
        let mut command = fixture.command(&broker);
        command.args([
            "--uart",
            pty.path.to_str().unwrap(),
            "--uart-quiet-ms",
            "600",
        ]);
        let (first_frame, feeding) = tokio::sync::oneshot::channel();
        let (stop_feeding, mut stopped) = tokio::sync::watch::channel(false);
        let feeder = async {
            let mut first_frame = Some(first_frame);
            loop {
                pty.peer.write_all(b"L020401\n").await.unwrap();
                if let Some(first_frame) = first_frame.take() {
                    let _ = first_frame.send(());
                }
                tokio::select! {
                    biased;
                    _ = stopped.changed() => break,
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {},
                }
            }
        };
        let launch = async {
            feeding.await.unwrap();
            let child = command.spawn().unwrap();
            inventory(&broker).await;
            stop_feeding.send_replace(true);
            child
        };
        let ((), mut child) = tokio::join!(feeder, launch);
        // Start before sending: the concurrent reader can process input before
        // the write future returns, but never before this measurement starts.
        let last_input = Instant::now();
        pty.peer.write_all(b"L020402\n").await.unwrap();
        assert_eq!(uart_line(&mut pty.peer).await, b"v\n");
        assert!(
            last_input.elapsed() >= Duration::from_millis(600),
            "daemon did not use the configured quiet interval after input"
        );
        if !restart {
            // End during an unanswered full write; no cleanup or Boot is sent.
            child.start_kill().unwrap();
            assert!(!exit(&mut child).await.success());
            continue;
        }
        assert!(
            timeout(Duration::from_millis(100), pty.peer.read_u8())
                .await
                .is_err(),
            "buffered old version completed the new initialization"
        );
        pty.peer.write_all(b"V013204\n").await.unwrap();
        assert_eq!(uart_line(&mut pty.peer).await, b"d\n");
        pty.peer.write_all(b"D24\n").await.unwrap();
        assert!(stop(&mut child).await.success());
    }
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), LEGACY);
}
#[tokio::test]
async fn broker_outage_does_not_hold_uart_startup_and_inventory_arrives_after_reconnect() {
    use tokio::io::AsyncWriteExt;
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    broker.refuse_connections(true);
    let mut pty = Pty::new();
    let mut child = fixture
        .command(&broker)
        .args(["--uart", pty.path.to_str().unwrap(), "--mqtt-retry", "50ms"])
        .spawn()
        .unwrap();
    assert_eq!(uart_line(&mut pty.peer).await, b"v\n");
    pty.peer.write_all(b"V013204\n").await.unwrap();
    assert_eq!(uart_line(&mut pty.peer).await, b"d\n");
    pty.peer.write_all(b"D24\n").await.unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "MQTT outage stopped the independent service"
    );
    assert_eq!(fixture.config()["name"], NAME);
    broker.refuse_connections(false);
    inventory(&broker).await;
    assert!(stop(&mut child).await.success());
}

#[tokio::test]
async fn full_unread_diagnostic_streams_hold_neither_start_nor_stop() {
    use std::{
        io::{ErrorKind, Write},
        os::{fd::OwnedFd, unix::net::UnixStream},
    };
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (mut full, unread) = UnixStream::pair().unwrap();
    full.set_nonblocking(true).unwrap();
    for chunk in [&[b'x'; 4096][..], &b"x"[..]] {
        loop {
            match full.write(chunk) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("stream fill: {error}"),
            }
        }
    }
    full.set_nonblocking(false).unwrap();
    let mut child = fixture
        .command(&broker)
        .stdout(Stdio::from(OwnedFd::from(full.try_clone().unwrap())))
        .stderr(Stdio::from(OwnedFd::from(full)))
        .spawn()
        .unwrap();
    inventory(&broker).await;
    let status = stop(&mut child).await;
    drop(unread);
    assert!(
        status.success(),
        "unread diagnostic streams held the daemon"
    );
}

async fn cpu_time(pid: u32) -> f64 {
    let request = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "time="])
        .output();
    let result = timeout(Duration::from_secs(2), request)
        .await
        .unwrap()
        .unwrap();
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    let value = text.trim();
    let (days, time) = value.split_once('-').map_or((0.0, value), |(days, time)| {
        (days.parse::<f64>().unwrap(), time)
    });
    time.split(':')
        .try_fold(0.0, |total, part| {
            part.parse::<f64>().map(|part| total * 60.0 + part)
        })
        .unwrap()
        + days * 86400.0
}
#[tokio::test]
async fn idle_metadata_daemon_has_no_busy_loop() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let mut child = fixture.command(&broker).spawn().unwrap();
    inventory(&broker).await;
    let pid = child.id().unwrap();
    let first = cpu_time(pid).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let used = cpu_time(pid).await - first;
    assert!(stop(&mut child).await.success());
    assert!(
        used < 0.3,
        "idle daemon consumed {used:.3} CPU seconds during a two-second sample"
    );
}
