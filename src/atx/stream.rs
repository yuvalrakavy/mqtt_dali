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
