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
