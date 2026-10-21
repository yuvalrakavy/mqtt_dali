use crate::{
    config::{self, ConfigPaths, Inventory, MAX_CONFIG_BYTES},
    inventory::Model,
};
use std::{
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Reservation,
    Mapping,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    StartupBeforeLoad,
    StartupSync,
    BeforeRequest,
    BeforeFinalize,
    BeforeRename(Stage),
    AfterRename(Stage),
    DirectorySync(Stage),
    RecoverySync,
}
#[derive(Clone, Default)]
pub(crate) struct Hooks {
    #[cfg(test)]
    pub callback: Option<Arc<dyn Fn(Step) -> Result<(), String> + Send + Sync>>,
}
impl Hooks {
    pub fn check(&self, step: Step) -> Result<(), String> {
        #[cfg(test)]
        if let Some(callback) = &self.callback {
            return callback(step);
        }
        let _ = step;
        #[cfg(debug_assertions)]
        {
            let variable = match step {
                Step::StartupBeforeLoad => Some("MQTT_DALI_V2_TEST_CONFIG_GATE"),
                Step::AfterRename(Stage::Mapping) => Some("MQTT_DALI_V2_TEST_SAVE_GATE"),
                _ => None,
            };
            if let Some(path) = variable.and_then(std::env::var_os) {
                let _ = fs::read(path);
            }
        }
        Ok(())
    }
}
pub(super) fn lock_path(paths: &ConfigPaths) -> Result<PathBuf, String> {
    let mut name = paths
        .current
        .file_name()
        .ok_or("configuration must name a file")?
        .to_os_string();
    name.push(".lock");
    Ok(parent(&paths.current).join(name))
}
pub(super) fn acquire(paths: &ConfigPaths) -> Result<File, String> {
    let path = lock_path(paths)?;
    // Reject aliases before create(true): even creating an empty legacy file
    // would violate the read-only v1 migration boundary.
    config::check_distinct_paths(paths).map_err(|e| e.to_string())?;
    config::check_distinct_paths(&ConfigPaths {
        legacy: paths.legacy.clone(),
        current: path.clone(),
    })
    .map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(
            "configuration lock must be a private, singly linked, owned regular file".into(),
        );
    }
    // Nonblocking lifetime lock; never unlink its inode, even on close.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(format!(
            "configuration writer ownership unavailable: {}",
            io::Error::last_os_error()
        ));
    }
    let at_path = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if at_path.dev() != metadata.dev() || at_path.ino() != metadata.ino() {
        return Err("configuration lock path changed during acquisition".into());
    }
    Ok(file)
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
pub(super) fn bytes(model: &Model) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec_pretty(model.inventory()).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"));
    }
    Ok(bytes)
}
pub(super) struct Failure {
    pub reason: String,
    pub uncertain: bool,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn create_temporary(path: PathBuf) -> io::Result<(File, Temporary)> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC)
        .open(&path)?;
    let temporary = Temporary(path);
    Ok((file, temporary))
}
pub(super) fn replace(
    path: &Path,
    expected: &Inventory,
    bytes: &[u8],
    stage: Stage,
    hooks: &Hooks,
) -> Result<(), Failure> {
    let before = |reason| Failure {
        reason,
        uncertain: false,
    };
    let after = |reason| Failure {
        reason,
        uncertain: true,
    };
    let current = config::load_current(path).map_err(|e| after(e.to_string()))?;
    if current.as_ref() != Some(expected) {
        return Err(after(
            "canonical configuration changed outside its owner".into(),
        ));
    }
    let temporary_path = parent(path).join(format!(
        ".dali.{}.{:032x}.tmp",
        std::process::id(),
        rand::random::<u128>()
    ));
    let (mut file, temporary) =
        create_temporary(temporary_path).map_err(|e| before(e.to_string()))?;
    file.set_permissions(Permissions::from_mode(0o600))
        .and_then(|_| file.write_all(bytes))
        .and_then(|_| file.sync_all())
        .map_err(|e| before(e.to_string()))?;
    hooks.check(Step::BeforeRename(stage)).map_err(before)?;
    // An errored rename may have taken effect. Reload rather than assuming old.
    fs::rename(&temporary.0, path).map_err(|e| after(e.to_string()))?;
    hooks.check(Step::AfterRename(stage)).map_err(after)?;
    hooks.check(Step::DirectorySync(stage)).map_err(after)?;
    File::open(parent(path))
        .and_then(|directory| directory.sync_all())
        .map_err(|e| after(e.to_string()))
}
pub(super) fn recover(
    path: &Path,
    allowed: &[Arc<Model>],
    hooks: &Hooks,
) -> Result<Arc<Model>, String> {
    let current = config::load_current(path)
        .map_err(|e| e.to_string())?
        .ok_or("configuration disappeared during recovery")?;
    let known = allowed
        .iter()
        .find(|model| model.inventory() == &current)
        .ok_or("configuration recovery found an unexpected mapping")?
        .clone();
    hooks.check(Step::RecoverySync)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("recovery configuration is not regular".into());
    }
    file.sync_all()
        .and_then(|_| File::open(parent(path))?.sync_all())
        .map_err(|e| e.to_string())?;
    // Catch ordinary edits racing the first read. The directory remains trusted.
    if config::load_current(path)
        .map_err(|e| e.to_string())?
        .as_ref()
        != Some(&current)
    {
        return Err("configuration changed during recovery".into());
    }
    Ok(known)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failing_to_create_a_temporary_file_preserves_the_existing_path() {
        let path = std::env::temp_dir().join(format!(
            "dali-existing-temp-{:032x}",
            rand::random::<u128>()
        ));
        fs::write(&path, b"owned by another operation").unwrap();
        let result = create_temporary(path.clone());
        let preserved = fs::read(&path).ok();
        let _ = fs::remove_file(&path);
        assert!(result.is_err());
        assert_eq!(
            preserved.as_deref(),
            Some(b"owned by another operation".as_slice()),
            "failed create_new removed a file it did not own"
        );
    }
}
