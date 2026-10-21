//! Private, one-request-per-connection JSON controls. No TCP listener.
use super::session::{Control, Emulator};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::task::JoinSet;

struct Owner {
    path: PathBuf,
    dev: u64,
    inode: u64,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let (path, dev, inode) = (self.path.clone(), self.dev, self.inode);
        // Cleanup is best effort and never joins a filesystem operation.
        let _ = std::thread::Builder::new()
            .name("dali-emulator-cleanup".into())
            .spawn(move || {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_socket() && meta.dev() == dev && meta.ino() == inode {
                        let _ = std::fs::remove_file(path);
                    }
                }
            });
    }
}
pub struct Endpoint {
    listener: UnixListener,
    _owner: Owner,
}
impl Endpoint {
    pub async fn bind(path: PathBuf) -> Result<Self, String> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("dali-emulator-control-bind".into())
            .spawn(move || {
                let _ = send.send(bind_private(path));
            })
            .map_err(|e| e.to_string())?;
        drop(worker);
        // WAIT: emulator-control-bind
        let (listener, owner) = tokio::time::timeout(Duration::from_secs(2), receive)
            .await
            .map_err(|_| "control socket bind deadline")?
            .map_err(|_| "control bind worker disappeared")?
            .map_err(|e| e.to_string())?;
        Ok(Self {
            listener: UnixListener::from_std(listener).map_err(|e| e.to_string())?,
            _owner: owner,
        })
    }
    pub async fn run(
        self,
        emulator: Emulator,
        mut stop: watch::Receiver<bool>,
    ) -> Result<(), String> {
        let mut clients = JoinSet::new();
        loop {
            if *stop.borrow() {
                return Ok(());
            }
            // WAIT: emulator-control-serve
            tokio::select! {
                _ = stop.changed() => return Ok(()),
                _ = clients.join_next(), if !clients.is_empty() => {},
                connection = self.listener.accept() => {
                    let (stream,_) = connection.map_err(|e| e.to_string())?;
                    // A seventeenth connection is closed immediately; no task is created.
                    if clients.len() < 16 {
                        let emulator = emulator.clone();
                        clients.spawn(async move {
                            // WAIT: emulator-control-client
                            let _ = tokio::time::timeout(Duration::from_secs(3),handle(stream,emulator)).await;
                        });
                    }
                },
            }
        }
    }
}
fn bind_private(path: PathBuf) -> io::Result<(std::os::unix::net::UnixListener, Owner)> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let meta = std::fs::metadata(parent)?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "control socket directory must belong to this user and have mode 0700",
        ));
    }
    // bind fails if anything exists here; never unlink a pre-existing endpoint.
    let listener = std::os::unix::net::UnixListener::bind(&path)?;
    let meta = std::fs::symlink_metadata(&path)?;
    let owner = Owner {
        path,
        dev: meta.dev(),
        inode: meta.ino(),
    };
    std::fs::set_permissions(&owner.path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok((listener, owner))
}
async fn handle(stream: UnixStream, emulator: Emulator) -> Result<(), String> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    // WAIT: emulator-control-client
    tokio::time::timeout(
        Duration::from_secs(1),
        (&mut reader).take(16385).read_until(b'\n', &mut bytes),
    )
    .await
    .map_err(|_| "control read deadline")?
    .map_err(|e| e.to_string())?;
    let result = if bytes.len() > 16384 || !bytes.ends_with(b"\n") {
        Err("control request requires a newline and at most 16384 bytes".into())
    } else {
        match serde_json::from_slice::<Control>(&bytes) {
            Ok(action) => emulator.control(action, Duration::from_secs(1)).await,
            Err(error) => Err(format!("invalid control: {error}")),
        }
    };
    let value = match result {
        Ok(value) => serde_json::json!({"result":value}),
        Err(error) => serde_json::json!({"error":error}),
    };
    let mut response = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if response.len() > 1024 * 1024 {
        return Err("control response exceeds 1 MiB".into());
    }
    response.push(b'\n');
    // WAIT: emulator-control-client
    tokio::time::timeout(
        Duration::from_secs(1),
        reader.get_mut().write_all(&response),
    )
    .await
    .map_err(|_| "control output deadline")?
    .map_err(|e| e.to_string())
}
