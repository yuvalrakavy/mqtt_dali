//! Process-level acceptance for the v2 configuration boundary. No broker/HAT.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mqtt-dali-v2-startup-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mqtt_dali_v2"));
        command
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn finish(&mut self, within: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + within;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "v2 config process did not exit in time"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn full_output_pipe() -> (fs::File, fs::File) {
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd};

    let mut descriptors = [-1; 2];
    assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
    // Own both pipe ends. CLOEXEC keeps the unused read end out of the child;
    // Stdio duplicates the writer onto stdout/stderr for each invocation.
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    let mut writer = unsafe { File::from_raw_fd(descriptors[1]) };
    for descriptor in [reader.as_raw_fd(), writer.as_raw_fd()] {
        assert_eq!(
            unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }
    let flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    loop {
        match writer.write(b"x") {
            Ok(1) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            other => panic!("could not fill output pipe: {other:?}"),
        }
    }
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags) },
        0
    );
    (reader, writer)
}

#[test]
fn unread_full_output_pipes_cannot_hold_success_error_or_help_exit() {
    let fixture = Fixture::new();
    let (reader, writer) = full_output_pipe();
    for (arguments, success) in [
        (vec!["--check-config"], true),
        (vec!["--unknown"], false),
        (vec!["--help"], true),
    ] {
        let child = fixture
            .command()
            .args(arguments)
            .stdout(Stdio::from(writer.try_clone().unwrap()))
            .stderr(Stdio::from(writer.try_clone().unwrap()))
            .spawn()
            .unwrap();
        assert_eq!(
            ChildGuard(child).finish(Duration::from_secs(5)).success(),
            success
        );
    }
    drop(reader); // Remains unread until every child exited.
}

#[test]
fn first_install_with_closed_stdin_creates_v2_without_prompting() {
    let fixture = Fixture::new();
    let child = fixture
        .command()
        .args(["--check-config", "--name", "NewController"])
        .spawn()
        .unwrap();
    assert!(ChildGuard(child).finish(Duration::from_secs(5)).success());
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(config["name"], "NewController");
    assert_eq!(config["buses"], serde_json::json!([]));
    assert!(!fixture.0.join("dali.json").exists());
}

#[test]
fn migration_check_needs_no_mqtt_argument_and_preserves_legacy_bytes() {
    let fixture = Fixture::new();
    let bytes = br#"{"name":"ExistingController","buses":[]}"#;
    fs::write(fixture.0.join("dali.json"), bytes).unwrap();
    let child = fixture.command().arg("--check-config").spawn().unwrap();
    assert!(ChildGuard(child).finish(Duration::from_secs(5)).success());
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), bytes);
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(config["name"], "ExistingController");
}

#[test]
fn a_migration_cut_short_during_write_never_publishes_a_partial_v2() {
    use std::os::unix::process::CommandExt;
    let fixture = Fixture::new();
    let original = br#"{"name":"ExistingController","buses":[]}"#;
    fs::write(fixture.0.join("dali.json"), original).unwrap();
    let mut command = fixture.command();
    command.arg("--check-config");
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 64,
                rlim_max: 64,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    assert!(!ChildGuard(command.spawn().unwrap())
        .finish(Duration::from_secs(5))
        .success());
    assert!(
        !fixture.0.join("dali.v2.json").exists(),
        "a cut-short import published a partial document"
    );
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), original);
    let retry = fixture.command().arg("--check-config").spawn().unwrap();
    assert!(ChildGuard(retry).finish(Duration::from_secs(5)).success());
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.0.join("dali.v2.json")).unwrap()).unwrap();
    assert_eq!(current["name"], "ExistingController");
    assert_eq!(fs::read(fixture.0.join("dali.json")).unwrap(), original);
}

#[test]
fn distinct_explicit_paths_use_the_current_file_after_the_first_import() {
    let fixture = Fixture::new();
    let legacy = fixture.0.join("old.json");
    let current = fixture.0.join("current.json");
    fs::write(&legacy, br#"{"name":"ImportedController","buses":[]}"#).unwrap();
    for corrupt_legacy in [false, true] {
        if corrupt_legacy {
            fs::write(&legacy, b"invalid legacy").unwrap();
        }
        let child = fixture
            .command()
            .args(["--check-config", "--legacy-config"])
            .arg(&legacy)
            .arg("--config")
            .arg(&current)
            .spawn()
            .unwrap();
        assert!(ChildGuard(child).finish(Duration::from_secs(5)).success());
    }
    assert!(!fixture.0.join("dali.v2.json").exists());
    let config: serde_json::Value = serde_json::from_slice(&fs::read(current).unwrap()).unwrap();
    assert_eq!(config["name"], "ImportedController");
}

#[test]
fn invalid_current_file_is_an_error_and_is_not_replaced() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("dali.json"), br#"{"name":"Old","buses":[]}"#).unwrap();
    let current = fixture.0.join("dali.v2.json");
    fs::write(&current, b"invalid current").unwrap();
    let child = fixture.command().arg("--check-config").spawn().unwrap();
    assert!(!ChildGuard(child).finish(Duration::from_secs(5)).success());
    assert_eq!(fs::read(current).unwrap(), b"invalid current");
}

#[test]
fn unfinished_v2_runtime_never_falls_through_to_legacy_startup() {
    let fixture = Fixture::new();
    let child = fixture.command().spawn().unwrap();
    assert!(!ChildGuard(child).finish(Duration::from_secs(2)).success());
    assert!(!fixture.0.join("dali.json").exists());
    assert!(!fixture.0.join("dali.v2.json").exists());
}

#[cfg(debug_assertions)]
#[test]
fn stalled_configuration_worker_does_not_hold_process_exit() {
    use std::os::unix::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let gate = fixture.0.join("config-gate");
    assert!(Command::new("mkfifo")
        .arg(&gate)
        .status()
        .unwrap()
        .success());
    let (_reader, writer) = full_output_pipe();
    let child = fixture
        .command()
        .arg("--check-config")
        .env("MQTT_DALI_V2_TEST_CONFIG_GATE", &gate)
        .stdout(Stdio::from(writer.try_clone().unwrap()))
        .stderr(Stdio::from(writer))
        .spawn()
        .unwrap();
    let mut child = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(2);
    let _hold = loop {
        match fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&gate)
        {
            Ok(writer) => break writer,
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                assert!(
                    Instant::now() < deadline,
                    "worker did not enter the real filesystem read"
                );
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "process ended before worker read"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("opening gate writer: {error}"),
        }
    };
    assert!(!child.finish(Duration::from_secs(5)).success());
    assert!(!fixture.0.join("dali.v2.json").exists());
}
