//! Deterministic test transport with an independently held receive drain.
use super::stream::{Drain, DrainRead};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::oneshot;

pub(crate) struct HeldDrain {
    pub stream: tokio::io::DuplexStream,
    pub held: Arc<AtomicBool>,
    pub entered: Option<oneshot::Sender<()>>,
}
impl AsyncRead for HeldDrain {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, bytes)
    }
}
impl DrainRead for HeldDrain {
    fn poll_drain(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<Drain>> {
        if self.held.load(Ordering::Acquire) {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        Pin::new(&mut self.stream).poll_drain(cx, bytes)
    }
}
impl AsyncWrite for HeldDrain {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
