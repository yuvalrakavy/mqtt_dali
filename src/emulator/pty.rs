use super::scenario::Scenario;
use super::session::Emulator;
use crate::atx::serial::Serial;
use std::ffi::CStr;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;
use std::time::Duration;

pub struct PtyEmulator {
    pub path: PathBuf,
    pub emulator: Emulator,
}

pub async fn spawn(scenario: Scenario) -> Result<PtyEmulator, String> {
    scenario.validate()?;
    let runtime = tokio::runtime::Handle::current();
    let (send, receive) = tokio::sync::oneshot::channel();
    let worker = std::thread::Builder::new()
        .name("dali-emulator-pty".into())
        .spawn(move || {
            let _context = runtime.enter();
            let _ = send.send(create());
        })
        .map_err(|e| e.to_string())?;
    drop(worker);
    // WAIT: emulator-pty-open
    let (serial, slave, path) = tokio::time::timeout(Duration::from_secs(2), receive)
        .await
        .map_err(|_| "PTY creation deadline")?
        .map_err(|_| "PTY worker disappeared")?
        .map_err(|e| e.to_string())?;
    Ok(PtyEmulator {
        path,
        emulator: Emulator::start_retained(serial, scenario, Some(slave))?,
    })
}
fn create() -> io::Result<(Serial, OwnedFd, PathBuf)> {
    let (mut master, mut slave) = (-1, -1);
    let mut name = [0 as libc::c_char; 1024];
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            name.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // openpty owns both descriptors on success, including on later errors.
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    if unsafe { libc::fcntl(slave.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let path = PathBuf::from(
        unsafe { CStr::from_ptr(name.as_ptr()) }
            .to_str()
            .map_err(io::Error::other)?,
    );
    Ok((Serial::from_owned_fd(master)?, slave, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retained_slave_is_not_inherited_by_exec_children() {
        let (_master, slave, _path) = create().unwrap();
        assert_ne!(
            unsafe { libc::fcntl(slave.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0,
            "PTY slave was inheritable across exec"
        );
    }
}
