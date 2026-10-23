//! Accepted IO retains a permit until actual drop, including Axum-spawned tasks.
use std::{
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};

pub(super) const CONNECTIONS: u32 = 64;
pub(super) struct Listener {
    pub listener: TcpListener,
    pub slots: Arc<Semaphore>,
    pub force: CancellationToken,
}
pub(super) struct Io {
    stream: TcpStream,
    canceled: Pin<Box<WaitForCancellationFutureOwned>>,
    _permit: OwnedSemaphorePermit,
}
impl Io {
    fn check(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if self.canceled.as_mut().poll(cx).is_ready() {
            Err(io::ErrorKind::ConnectionAborted.into())
        } else {
            Ok(())
        }
    }
}
impl AsyncRead for Io {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.check(cx)?;
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
impl AsyncWrite for Io {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.check(cx)?;
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check(cx)?;
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check(cx)?;
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
impl axum::serve::Listener for Listener {
    type Io = Io;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Io, SocketAddr) {
        loop {
            // WAIT: emulator-web-serve
            let (stream, address) =
                <TcpListener as axum::serve::Listener>::accept(&mut self.listener).await;
            let Ok(permit) = self.slots.clone().try_acquire_owned() else {
                drop(stream);
                continue;
            };
            return (
                Io {
                    stream,
                    canceled: Box::pin(self.force.clone().cancelled_owned()),
                    _permit: permit,
                },
                address,
            );
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}
