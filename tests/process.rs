//! The bridge as a process, against a fake broker on 127.0.0.1 and the DALI emulator (Store
//! no-hang 3b): SIGTERM takes the bounded shutdown path and main returns, so the logging guard is
//! dropped (finding C-7); and logging is on without any flag (finding C-M6). Every wait here is
//! bounded, and the child is killed when a test ends, however it ends.

use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use mqtt_test_broker::FakeBroker;

const NAME: &str = "Process";

struct Bridge {
    child: Child,
    dir: PathBuf,
}

impl Bridge {
    fn start(broker: &FakeBroker, tag: &str) -> Bridge {
        Bridge::start_with(broker, tag, "c", false)
    }

    /// `destination`: LOG_DESTINATION. `read_only_cwd`: the bridge runs in a directory it cannot
    /// write, so a file destination (`logs/` under it) cannot start.
    fn start_with(
        broker: &FakeBroker,
        tag: &str,
        destination: &str,
        read_only_cwd: bool,
    ) -> Bridge {
        let dir = std::env::temp_dir().join(format!("mqtt-dali-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let config = dir.join("dali.json");
        let json = format!(
            r#"{{"name":"{NAME}","buses":[{{"description":"Bus-1","status":"Active","bus":0,"channels":[],"groups":[]}}]}}"#
        );
        std::fs::write(&config, json).expect("a config file");
        let stdout = std::fs::File::create(dir.join("stdout.log")).expect("stdout file");
        let stderr = std::fs::File::create(dir.join("stderr.log")).expect("stderr file");
        let cwd = if read_only_cwd {
            use std::os::unix::fs::PermissionsExt;
            let cwd = dir.join("read-only");
            std::fs::create_dir_all(&cwd).expect("a read-only directory");
            std::fs::set_permissions(&cwd, std::fs::Permissions::from_mode(0o555))
                .expect("read-only");
            cwd
        } else {
            dir.clone()
        };
        let child = Command::new(env!("CARGO_BIN_EXE_mqtt_dali"))
            .args([
                "--emulation",
                "--config",
                config.to_str().expect("a UTF-8 path"),
                &broker.address(),
            ])
            // No logging.toml is found from here, and the destinations are the console and at most
            // a local file: nothing this test starts sends telemetry anywhere.
            .current_dir(&cwd)
            .env("LOG_DESTINATION", destination)
            .env_remove("LOG_CONFIG")
            .env_remove("LOG_LEVEL")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .expect("the bridge");
        Bridge { child, dir }
    }

    fn terminate(&self) {
        let status = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .expect("kill");
        assert!(status.success(), "kill -TERM failed");
    }

    async fn wait(&mut self, within: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(status) = self.child.try_wait().expect("the child's status") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn output(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn describe(status: ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("exit status {code}"),
        (None, Some(signal)) => format!("killed by signal {signal}"),
        _ => format!("{status:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sigterm_publishes_inactive_and_exits_cleanly() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::start(&broker, "sigterm");
    let active = format!("DALI/Active/{NAME}");
    let subscribed = broker
        .wait_for_subscription(
            &format!("DALI/Controllers/{NAME}/Command"),
            Duration::from_secs(20),
        )
        .await;
    assert!(
        subscribed,
        "the bridge never subscribed; stderr:\n{}",
        bridge.output("stderr.log")
    );
    let before = broker.received_on(&active).len();

    bridge.terminate();
    let status = bridge.wait(Duration::from_secs(10)).await;
    let status = status.unwrap_or_else(|| panic!("the bridge did not exit within 10 s of SIGTERM"));
    assert!(
        status.success(),
        "the bridge did not exit cleanly on SIGTERM ({}): no handler took it to the shutdown path",
        describe(status)
    );
    let after = broker.received_on(&active);
    assert!(
        after[before..]
            .iter()
            .any(|r| r.payload.as_ref() == b"false" && r.retain),
        "the bridge exited without publishing a retained Active=false: {:?}",
        &after[before..]
    );
}

/// A destination that cannot start (a file under a directory the bridge cannot write) is skipped:
/// the console still logs, and the bridge runs (finding C-8: `on_destination_error` is pinned to
/// skip in code, and logging never panics the bridge).
#[tokio::test(flavor = "multi_thread")]
async fn a_log_destination_that_cannot_start_is_skipped() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::start_with(&broker, "skip", "cf", true);
    let subscribed = broker
        .wait_for_subscription(
            &format!("DALI/Controllers/{NAME}/Command"),
            Duration::from_secs(20),
        )
        .await;
    assert!(
        subscribed,
        "the bridge never subscribed; stderr:\n{}",
        bridge.output("stderr.log")
    );
    bridge.terminate();
    let _ = bridge.wait(Duration::from_secs(10)).await;
    let stdout = bridge.output("stdout.log");
    let stderr = bridge.output("stderr.log");
    assert!(
        stdout.contains("connected to the MQTT broker"),
        "the console did not log beside a file destination that could not start\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("file destination failed"),
        "the skipped file destination was not reported\nstderr:\n{stderr}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn logging_is_on_without_any_flag() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::start(&broker, "logging");
    let subscribed = broker
        .wait_for_subscription(
            &format!("DALI/Controllers/{NAME}/Command"),
            Duration::from_secs(20),
        )
        .await;
    assert!(
        subscribed,
        "the bridge never subscribed; stderr:\n{}",
        bridge.output("stderr.log")
    );
    bridge.terminate();
    let _ = bridge.wait(Duration::from_secs(10)).await;
    let stdout = bridge.output("stdout.log");
    // An INFO of mqtt_dali's: the default filter (warn,mqtt_dali=info) lets it through.
    assert!(
        stdout.contains("connected to the MQTT broker"),
        "no log line on stdout with LOG_DESTINATION=c and no flag: tracing-init was not initialised\nstdout:\n{stdout}\nstderr:\n{}",
        bridge.output("stderr.log")
    );
}
