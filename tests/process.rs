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
    /// The read end of a stdout pipe nobody reads, kept open until the child has exited.
    _stdout_pipe: Option<std::io::PipeReader>,
}

/// Makes `path` a FIFO: opening it waits until the other end is opened too, as a file on a
/// filesystem that stopped answering waits.
fn mkfifo(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let made = Command::new("mkfifo").arg(path).status().expect("mkfifo");
    assert!(made.success(), "mkfifo {path:?} failed");
}

/// How a test starts the bridge.
struct Launch<'a> {
    /// LOG_DESTINATION.
    destination: &'a str,
    /// The bridge runs in a directory it cannot write, so a file destination (`logs/` under it)
    /// cannot start.
    read_only_cwd: bool,
    /// The configuration file is a FIFO nobody writes: start-up waits in reading it.
    stalled_config: bool,
    /// The file destination is on (LOG_DESTINATION `cf`), and its log file, `logs/dali.<date>.log`
    /// for yesterday, today and tomorrow (UTC), is a FIFO nobody reads: tracing-init's start waits
    /// in opening it until it gives it up, 5 s in.
    stalled_log_file: bool,
    /// Lights in the configuration (a larger file).
    channels: usize,
    /// The bridge may write files of at most 512 bytes (`ulimit -f 1`): a larger write is cut
    /// short, the kernel ending the process (SIGXFSZ). Its stdout and stderr go nowhere.
    file_size_limited: bool,
    /// Its stdout is a pipe already full that nobody reads, as a supervisor's that stopped
    /// draining.
    stdout_full: bool,
}

impl Default for Launch<'_> {
    fn default() -> Self {
        Launch {
            destination: "c",
            read_only_cwd: false,
            stalled_config: false,
            stalled_log_file: false,
            channels: 0,
            file_size_limited: false,
            stdout_full: false,
        }
    }
}

/// The UTC date `days` from today, `YYYY-MM-DD` (the civil-from-days algorithm, Howard Hinnant's).
fn utc_date(days: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("after 1970");
    let z = now.as_secs() as i64 / 86_400 + days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// A pipe filled until a write to it waits: nobody reads it. The filler thread stays blocked until
/// the read end is dropped.
fn full_pipe() -> (std::io::PipeReader, std::io::PipeWriter) {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let mut filler = writer.try_clone().expect("the pipe's write end");
    let written = Arc::new(AtomicUsize::new(0));
    let count = written.clone();
    std::thread::spawn(move || {
        let chunk = [b'x'; 4096];
        while let Ok(n) = filler.write(&chunk) {
            count.fetch_add(n, Ordering::SeqCst);
        }
    });
    // Full once the filler has made no progress for a while: it is blocked in a write.
    let mut last = usize::MAX;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let now = written.load(Ordering::SeqCst);
        if now == last && now > 0 {
            break;
        }
        assert!(Instant::now() < deadline, "the pipe never filled");
        last = now;
    }
    (reader, writer)
}

impl Bridge {
    fn start(broker: &FakeBroker, tag: &str) -> Bridge {
        Bridge::launch(broker, tag, Launch::default())
    }

    /// `destination`: LOG_DESTINATION. `read_only_cwd`: the bridge runs in a directory it cannot
    /// write, so a file destination (`logs/` under it) cannot start.
    fn start_with(
        broker: &FakeBroker,
        tag: &str,
        destination: &str,
        read_only_cwd: bool,
    ) -> Bridge {
        Bridge::launch(broker, tag, Launch { destination, read_only_cwd, ..Launch::default() })
    }

    /// A bridge whose configuration file is a FIFO nobody writes: its start-up waits in reading it.
    fn start_stalled_reading_its_config(broker: &FakeBroker, tag: &str) -> Bridge {
        Bridge::launch(broker, tag, Launch { stalled_config: true, ..Launch::default() })
    }

    fn launch(broker: &FakeBroker, tag: &str, launch: Launch) -> Bridge {
        let dir = std::env::temp_dir().join(format!("mqtt-dali-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let config = dir.join("dali.json");
        if launch.stalled_config {
            mkfifo(&config);
        } else {
            let channels: Vec<String> = (0..launch.channels)
                .map(|a| format!(r#"{{"description":"Light number {a} of the test bus","short_address":{a}}}"#))
                .collect();
            let json = format!(
                r#"{{"name":"{NAME}","buses":[{{"description":"Bus-1","status":"Active","bus":0,"channels":[{}],"groups":[]}}]}}"#,
                channels.join(",")
            );
            std::fs::write(&config, json).expect("a config file");
        }
        let logging = dir.join("logging.toml");
        std::fs::write(&logging, "[logging]\ndestination = \"c\"\n").expect("a logging config");
        let mut destination = launch.destination;
        if launch.stalled_log_file {
            destination = "cf";
            std::fs::create_dir_all(dir.join("logs")).expect("a logs directory");
            for day in [-1, 0, 1] {
                mkfifo(&dir.join("logs").join(format!("dali.{}.log", utc_date(day))));
            }
        }
        let cwd = if launch.read_only_cwd {
            use std::os::unix::fs::PermissionsExt;
            let cwd = dir.join("read-only");
            std::fs::create_dir_all(&cwd).expect("a read-only directory");
            std::fs::set_permissions(&cwd, std::fs::Permissions::from_mode(0o555))
                .expect("read-only");
            cwd
        } else {
            dir.clone()
        };
        let bridge = env!("CARGO_BIN_EXE_mqtt_dali");
        let args = [
            "--emulation",
            "--config",
            config.to_str().expect("a UTF-8 path"),
            &broker.address(),
        ];
        let mut command = if launch.file_size_limited {
            let mut sh = Command::new("/bin/sh");
            sh.args(["-c", "ulimit -f 1 && exec \"$0\" \"$@\"", bridge]).args(args);
            sh
        } else {
            let mut command = Command::new(bridge);
            command.args(args);
            command
        };
        let mut stdout_pipe = None;
        let (stdout, stderr) = if launch.file_size_limited {
            (Stdio::null(), Stdio::null())
        } else if launch.stdout_full {
            let (reader, writer) = full_pipe();
            stdout_pipe = Some(reader);
            let stderr = std::fs::File::create(dir.join("stderr.log")).expect("stderr file");
            (Stdio::from(writer), Stdio::from(stderr))
        } else {
            let stdout = std::fs::File::create(dir.join("stdout.log")).expect("stdout file");
            let stderr = std::fs::File::create(dir.join("stderr.log")).expect("stderr file");
            (Stdio::from(stdout), Stdio::from(stderr))
        };
        let child = command
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
        Bridge { child, dir, _stdout_pipe: stdout_pipe }
    }

    fn config_path(&self) -> PathBuf {
        self.dir.join("dali.json")
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

    /// Makes the bridge's configuration file, and the temporary a save writes before renaming it
    /// over the file, FIFOs nobody reads: its next save waits in opening whichever it writes.
    fn stall_config_writes(&self) {
        mkfifo(&self.dir.join("dali.json"));
        mkfifo(&self.dir.join(".dali.json.tmp"));
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Only now, with the child gone, is the full stdout pipe's read end dropped.
        self._stdout_pipe = None;
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

/// The logging start is raced with a stop too (fleet class B1). tracing-init's start is
/// synchronous: it gives a destination that will not start 5 s, and reads its logging config with
/// no bound of its own. Here the file destination's log file is a FIFO nobody reads, so its open
/// waits until tracing-init gives it up, 5 s in. SIGTERM, 1 s in, must end the bridge at once —
/// within the runtime's 1 s for the stuck start, not when tracing-init gives up 4 s later.
///
/// Nothing is logged before logging is up, so no line proves the start has begun; the stop
/// handlers are installed first thing, milliseconds in. A late SIGTERM could only be a false RED;
/// and an unraced start can only exit later than this test allows, never sooner.
#[tokio::test(flavor = "multi_thread")]
async fn sigterm_while_a_log_destination_is_starting_exits_at_once() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::launch(&broker, "stuck-log-file", Launch { stalled_log_file: true, ..Launch::default() });
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        bridge.child.try_wait().expect("the child's status").is_none(),
        "the bridge ended before the SIGTERM; stderr:\n{}",
        bridge.output("stderr.log")
    );
    let sent = Instant::now();
    bridge.terminate();
    let status = bridge.wait(Duration::from_secs(10)).await;
    let took = sent.elapsed();
    let status = status.unwrap_or_else(|| panic!("the bridge did not exit within 10 s of SIGTERM during its logging start"));
    assert!(status.success(), "the bridge did not exit cleanly ({})", describe(status));
    assert!(
        took <= Duration::from_millis(2500),
        "the bridge exited {took:?} after SIGTERM: it waited for its logging start (tracing-init gives a stuck destination 5 s) instead of racing it with the stop"
    );
}

/// Nothing on the start-up or stop path writes to stdout or stderr directly (fleet class B2): it
/// goes through tracing, whose sinks drop what they cannot write. With stdout a full pipe nobody
/// reads (a supervisor's that stopped draining; kept unread until the child has exited), the
/// bridge still starts, and stops within its bound. A direct print there waits for ever.
#[tokio::test(flavor = "multi_thread")]
async fn a_full_stdout_pipe_never_holds_the_start_or_the_stop() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::launch(&broker, "full-stdout", Launch { stdout_full: true, ..Launch::default() });
    let subscribed = broker
        .wait_for_subscription(&format!("DALI/Controllers/{NAME}/Command"), Duration::from_secs(20))
        .await;
    assert!(
        subscribed,
        "the bridge never subscribed with its stdout full: a direct print on the start-up path waits on the pipe; stderr:\n{}",
        bridge.output("stderr.log")
    );
    bridge.terminate();
    // The stop's bound: the session's 5 s, the logging guard's bounded flush (its console writer
    // is stuck on the pipe), and the runtime's 2 s.
    let status = bridge.wait(Duration::from_secs(15)).await;
    let status = status.unwrap_or_else(|| {
        panic!("the bridge did not exit within 15 s of SIGTERM with its stdout full: a direct print on the stop path waits on the pipe")
    });
    assert!(status.success(), "the bridge did not exit cleanly ({})", describe(status));
}

/// A configuration write cut short never leaves dali.json unreadable (finding Q6): a save writes a
/// temporary file and renames it over the old one. Here the file-size limit cuts the write (the
/// kernel ends the bridge, SIGXFSZ), as the bounded exit abandons a write; written in place, the
/// file was left truncated, and the bridge then refused to start.
#[tokio::test(flavor = "multi_thread")]
async fn a_config_write_cut_short_leaves_the_last_good_config() {
    let broker = FakeBroker::start().await;
    let mut bridge = Bridge::launch(&broker, "cut-write", Launch { channels: 40, file_size_limited: true, ..Launch::default() });
    let before = std::fs::read_to_string(bridge.config_path()).expect("the config file");
    assert!(before.len() > 1024, "the config is too small for the file-size limit to cut its save");
    let command = format!("DALI/Controllers/{NAME}/Command");
    let status_topic = format!("DALI/Status/{NAME}");
    let subscribed = broker.wait_for_subscription(&command, Duration::from_secs(20)).await;
    assert!(subscribed, "the bridge never subscribed");
    assert!(broker.send(&command, r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
    // The save is cut short: the bridge ends (SIGXFSZ), or reports the failed write.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ended = bridge.child.try_wait().expect("the child's status").is_some();
        let reported = broker
            .received_on(&status_topic)
            .iter()
            .any(|r| String::from_utf8_lossy(&r.payload).contains("saving the configuration"));
        if ended || reported || Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let after = std::fs::read_to_string(bridge.config_path()).unwrap_or_default();
    assert!(
        serde_json::from_str::<serde_json::Value>(&after).is_ok(),
        "dali.json was left unreadable by a write cut short ({} bytes; {} before): the save wrote it in place",
        after.len(),
        before.len()
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
