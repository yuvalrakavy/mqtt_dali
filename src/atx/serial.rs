//! Nonblocking Unix UART/PTY adapter; configuration runs off runtime workers.

use super::stream::{Drain, DrainRead};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct Serial {
    descriptor: AsyncFd<OwnedFd>,
    #[cfg(test)]
    write_limit: Option<WriteLimit>,
}

#[cfg(test)]
struct WriteLimit {
    remaining: usize,
    blocked: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Serial {
    /// Open/configure on this runtime's blocking pool. A late result drops its
    /// owned fd, and actual runtime teardown includes the worker. No reader is
    /// started here. An owner must not retry a timed-out open and accumulate
    /// outstanding workers; the managed service treats that timeout as terminal.
    pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        Self::open_using(move || open_configured(path), Duration::from_secs(2)).await
    }
    async fn open_using(
        open: impl FnOnce() -> io::Result<OwnedFd> + Send + 'static,
        within: Duration,
    ) -> io::Result<Self> {
        let deadline = tokio::time::Instant::now() + within;
        let worker = tokio::task::spawn_blocking(open);
        // WAIT: atx-serial-open
        let result = tokio::time::timeout_at(deadline, worker)
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "serial open/configure exceeded its deadline",
                )
            })?;
        let descriptor = match result {
            Ok(result) => result?,
            Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
            Err(_) => return Err(io::Error::other("serial open worker was canceled")),
        };
        if tokio::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "serial open/configure exceeded its deadline",
            ));
        }
        Ok(Self {
            descriptor: AsyncFd::new(descriptor)?,
            #[cfg(test)]
            write_limit: None,
        })
    }

    #[cfg(test)]
    pub(crate) async fn open_with_hook(
        path: std::path::PathBuf,
        hook: OpenHook,
    ) -> io::Result<Self> {
        Self::open_using(
            move || {
                hook()?;
                open_configured(path)
            },
            Duration::from_secs(2),
        )
        .await
    }

    /// Adopt a TTY/PTY owned by the caller. This synchronous initialization must
    /// happen on a bounded startup worker for real hardware; emulator tests may
    /// call it on an already open, local PTY.
    pub fn from_owned_fd(descriptor: OwnedFd) -> io::Result<Self> {
        configure(&descriptor)?;
        Ok(Self {
            descriptor: AsyncFd::new(descriptor)?,
            #[cfg(test)]
            write_limit: None,
        })
    }

    /// Deterministic PTY backpressure after an actual accepted prefix. Unit
    /// builds only: no environment knob or production transport behavior.
    #[cfg(test)]
    pub(crate) fn limit_writes(
        &mut self,
        prefix: usize,
        blocked: tokio::sync::oneshot::Sender<()>,
    ) {
        self.write_limit = Some(WriteLimit {
            remaining: prefix,
            blocked: Some(blocked),
        });
    }
}

#[cfg(test)]
pub(crate) type OpenHook = std::sync::Arc<dyn Fn() -> io::Result<()> + Send + Sync>;

fn open_configured(path: PathBuf) -> io::Result<OwnedFd> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(path)?;
    let descriptor: OwnedFd = file.into();
    configure(&descriptor)?;
    Ok(descriptor)
}

fn syscall(result: libc::c_int) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn configure(descriptor: &OwnedFd) -> io::Result<()> {
    let fd = descriptor.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    syscall(flags)?;
    syscall(unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) })?;
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    syscall(flags)?;
    syscall(unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) })?;
    let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
    syscall(unsafe { libc::tcgetattr(fd, settings.as_mut_ptr()) })?;
    let mut settings = unsafe { settings.assume_init() };
    unsafe { libc::cfmakeraw(&mut settings) };
    settings.c_cflag &= !(libc::CSIZE | libc::PARENB | libc::CSTOPB | libc::CRTSCTS);
    settings.c_cflag |= libc::CS8 | libc::CREAD | libc::CLOCAL;
    settings.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
    settings.c_cc[libc::VMIN] = 1;
    settings.c_cc[libc::VTIME] = 0;
    syscall(unsafe { libc::cfsetispeed(&mut settings, libc::B19200) })?;
    syscall(unsafe { libc::cfsetospeed(&mut settings, libc::B19200) })?;
    // TCSANOW preserves received traffic; do not tcflush or use TCSAFLUSH.
    syscall(unsafe { libc::tcsetattr(fd, libc::TCSANOW, &settings) })
}

impl AsyncRead for Serial {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            let mut ready = ready!(self.descriptor.poll_read_ready(context))?;
            let result = ready.try_io(|descriptor| {
                let output = buffer.initialize_unfilled();
                // The slice is initialized, writable and live through this syscall.
                let count = unsafe {
                    libc::read(
                        descriptor.get_ref().as_raw_fd(),
                        output.as_mut_ptr().cast(),
                        output.len(),
                    )
                };
                if count < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(count as usize)
                }
            });
            match result {
                Ok(Ok(count)) => {
                    buffer.advance(count);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {
                    // Keep readiness and retry on the next poll, giving timers
                    // and cancellation a chance even under repeated signals.
                    context.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Ok(Err(error)) => return Poll::Ready(Err(error)),
                Err(_) => continue,
            }
        }
    }
}

impl DrainRead for Serial {
    fn poll_drain(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Drain>> {
        let output = buffer.initialize_unfilled();
        let count = unsafe {
            libc::read(
                self.descriptor.get_ref().as_raw_fd(),
                output.as_mut_ptr().cast(),
                output.len(),
            )
        };
        if count >= 0 {
            buffer.advance(count as usize);
            return Poll::Ready(Ok(Drain::Data));
        }
        let error = io::Error::last_os_error();
        match error.kind() {
            io::ErrorKind::WouldBlock => Poll::Ready(Ok(Drain::Empty)),
            io::ErrorKind::Interrupted => {
                context.waker().wake_by_ref();
                Poll::Pending
            }
            _ => Poll::Ready(Err(error)),
        }
    }
}

impl AsyncWrite for Serial {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        #[cfg(test)]
        let bytes = match this.write_limit.as_mut() {
            Some(limit) if limit.remaining == 0 => {
                if let Some(blocked) = limit.blocked.take() {
                    let _ = blocked.send(());
                }
                return Poll::Pending;
            }
            Some(limit) => &bytes[..bytes.len().min(limit.remaining)],
            None => bytes,
        };
        loop {
            let mut ready = ready!(this.descriptor.poll_write_ready(context))?;
            let result = ready.try_io(|descriptor| {
                let count = unsafe {
                    libc::write(
                        descriptor.get_ref().as_raw_fd(),
                        bytes.as_ptr().cast(),
                        bytes.len(),
                    )
                };
                if count < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(count as usize)
                }
            });
            drop(ready);
            match result {
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => {
                    context.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Ok(result) => {
                    #[cfg(test)]
                    if let (Ok(count), Some(limit)) = (&result, this.write_limit.as_mut()) {
                        limit.remaining = limit.remaining.saturating_sub(*count);
                    }
                    return Poll::Ready(result);
                }
                Err(_) => continue,
            }
        }
    }

    // UART acceptance is the kernel write. tcdrain could block indefinitely and
    // neither flush nor shutdown claims physical transmission/acknowledgement.
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests;
