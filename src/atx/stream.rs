//! Explicit nonblocking drain contract for a UART receive fence.
use std::{
    future::{poll_fn, Future},
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drain {
    Data,
    Empty,
}

/// Implementations must distinguish genuine would-block from retry/yield. IO
/// polls must not block or leave background reads/writes running after Pending.
/// Serial and in-memory duplex streams meet this contract; async files do not.
pub trait DrainRead: AsyncRead {
    fn poll_drain(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Drain>>;
}

impl DrainRead for tokio::net::TcpStream {
    fn poll_drain(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Drain>> {
        use std::os::fd::AsRawFd;
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(Drain::Data));
        }
        let output = buffer.initialize_unfilled();
        // A readiness cache may say WouldBlock before checking the kernel.
        // Only this nonblocking syscall establishes the receive fence's Empty.
        // The initialized writable slice and socket stay live through recv.
        let count = unsafe {
            libc::recv(
                self.as_raw_fd(),
                output.as_mut_ptr().cast(),
                output.len(),
                libc::MSG_DONTWAIT,
            )
        };
        if count >= 0 {
            buffer.advance(count as usize);
            // recv=0 is EOF, represented as Data with no bytes, like AsyncRead.
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

impl DrainRead for tokio::io::DuplexStream {
    fn poll_drain(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Drain>> {
        // With cooperative yielding disabled, Tokio's duplex read returns
        // Pending only for an empty buffer (tokio 1.41 io/util/mem.rs).
        let mut read =
            tokio::task::unconstrained(poll_fn(|context| self.as_mut().poll_read(context, buffer)));
        match Pin::new(&mut read).poll(context) {
            Poll::Ready(result) => Poll::Ready(result.map(|_| Drain::Data)),
            Poll::Pending => Poll::Ready(Ok(Drain::Empty)),
        }
    }
}
