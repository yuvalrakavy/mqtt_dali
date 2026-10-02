//! The bridge as a process, against a fake broker on 127.0.0.1 and the DALI emulator (Store
//! no-hang 3b): SIGTERM takes the bounded shutdown path and main returns, so the logging guard is
//! dropped (finding C-7) — also during start-up (fleet class F3) and while a file write is stuck
//! (fleet class F1); and logging is on without any flag (finding C-M6). Every wait here is
//! bounded, and the child is killed when a test ends, however it ends. Logging goes to the console
//! only: `LOG_CONFIG` names a console-only file in the test's scratch directory, and nothing this
//! file starts sends telemetry anywhere.

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

/// Makes `path` a FIFO: opening it waits until the other end is opened too, as a file on a
/// filesystem that stopped answering waits.
fn mkfifo(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let made = Command::new("mkfifo").arg(path).status().expect("mkfifo");
    assert!(made.success(), "mkfifo {path:?} failed");
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
        Bridge::launch(broker, tag, destination, read_only_cwd, false)
    }

    /// A bridge whose configuration file is a FIFO nobody writes: its start-up waits in reading it.
    fn start_stalled_reading_its_config(broker: &FakeBroker, tag: &str) -> Bridge {
        Bridge::launch(broker, tag, "c", false, true)
    }

    fn launch(
        broker: &FakeBroker,
        tag: &str,
        destination: &str,
        read_only_cwd: bool,
        stalled_config: bool,
    ) -> Bridge {
        let dir = std::env::temp_dir().join(format!("mqtt-dali-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let config = dir.join("dali.json");
        if stalled_config {
            mkfifo(&config);
        } else {
            let json = format!(
                r#"{{"name":"{NAME}","buses":[{{"description":"Bus-1","status":"Active","bus":0,"channels":[],"groups":[]}}]}}"#
            );
            std::fs::write(&config, json).expect("a config file");
        }
        let logging = dir.join("logging.toml");
        std::fs::write(&logging, "[logging]\ndestination = \"c\"\n").expect("a logging config");
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
            // A console-only logging config, and LOG_DESTINATION over it: the console and at most
            // a local file. Nothing this test starts sends telemetry anywhere.
            .current_dir(&cwd)
            .env("LOG_DESTINATION", destination)
            .env("LOG_CONFIG", &logging)
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

    /// Waits until the bridge's stdout contains `text`, for at most `within`.
    async fn logged(&self, text: &str, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        loop {
            if self.output("stdout.log").contains(text) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Replaces the bridge's configuration file with a FIFO nobody reads: its next save waits in
    /// opening it.
    fn stall_config_writes(&self) {
        mkfifo(&self.dir.join("dali.json"));
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

/// A stop during start-up (fleet class F3): the configuration is a FIFO nobody writes, so start-up
/// waits in reading it, as it would on a filesystem that stopped answering, or for the HAT's version
/// (up to 11 s). SIGTERM must still take the bridge's own path, logged and exiting cleanly in
/// bounded time: its handler is installed before start-up, and start-up is raced with it. Installed
/// late, the signal's default action killed the bridge with nothing logged; installed early but not
/// raced, it would leave the bridge unkillable but by SIGKILL.
#[tokio::test(flavor = "multi_thread")]
async fn sigterm_during_start_up_takes_the_bounded_path() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::start_stalled_reading_its_config(&broker, "startup");
    // Logged once logging is up, just before the configuration is read.
    let loading = bridge.logged("Loading configuration", Duration::from_secs(20)).await;
    assert!(loading, "the bridge never began to load its configuration; stderr:\n{}", bridge.output("stderr.log"));
    // A moment for start-up to be inside its read.
    tokio::time::sleep(Duration::from_millis(300)).await;
    bridge.terminate();
    let status = bridge.wait(Duration::from_secs(10)).await;
    let status = status.unwrap_or_else(|| {
        panic!("the bridge did not exit within 10 s of SIGTERM during start-up\nstdout:\n{}", bridge.output("stdout.log"))
    });
    assert!(
        status.success(),
        "SIGTERM during start-up took the signal's default action ({}): no handler was installed yet\nstdout:\n{}",
        describe(status),
        bridge.output("stdout.log")
    );
    assert!(
        bridge.output("stdout.log").contains("during start-up"),
        "the stop during start-up was not logged\nstdout:\n{}",
        bridge.output("stdout.log")
    );
}

/// The whole process's stop is bounded (fleet class F1): SIGTERM while a configuration write is
/// stuck (the file replaced by a FIFO nobody reads) still ends the process within its bound.
/// Dropping a tokio runtime waits, without limit, for every blocking task still running, so the
/// write left running on the blocking pool must not hold the exit.
#[tokio::test(flavor = "multi_thread")]
async fn sigterm_while_a_config_write_is_stuck_exits_in_bounded_time() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::start(&broker, "stuck-write");
    let command = format!("DALI/Controllers/{NAME}/Command");
    let status_topic = format!("DALI/Status/{NAME}");
    let active = format!("DALI/Active/{NAME}");
    let subscribed = broker.wait_for_subscription(&command, Duration::from_secs(20)).await;
    assert!(subscribed, "the bridge never subscribed; stderr:\n{}", bridge.output("stderr.log"));
    bridge.stall_config_writes();
    assert!(broker.send(&command, r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
    let reported = broker
        .wait_until(Duration::from_secs(15), |b| {
            b.received_on(&status_topic).iter().any(|r| String::from_utf8_lossy(&r.payload).contains("saving the configuration"))
        })
        .await;
    assert!(reported, "the stuck write was never reported; status: {:?}", broker.received_on(&status_topic));
    let before = broker.received_on(&active).len();

    bridge.terminate();
    let status = bridge.wait(Duration::from_secs(10)).await;
    let status = status.unwrap_or_else(|| {
        panic!(
            "the bridge did not exit within 10 s of SIGTERM while a configuration write was stuck: \
             its runtime waited for the write\nstdout:\n{}",
            bridge.output("stdout.log")
        )
    });
    assert!(status.success(), "the bridge did not exit cleanly ({})", describe(status));
    let after = broker.received_on(&active);
    assert!(
        after[before..].iter().any(|r| r.payload.as_ref() == b"false" && r.retain),
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
