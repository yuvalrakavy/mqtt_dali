//! One owned ATX byte stream, selected without changing protocol processing.
use super::{
    serial::Serial,
    stream::{Drain, DrainRead},
};
use std::{
    future::Future,
    io,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

/// Startup configuration only; construction performs no IO or name resolution.
#[derive(Debug, Clone)]
pub enum Endpoint {
    Uart(PathBuf),
    /// Raw ATX, with a fresh HAT parser/output lifetime for each connection.
    Tcp(SocketAddr),
}
impl Endpoint {
    pub fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp(_))
    }
    /// TCP uses an async two-second connect bound. UART retains its own bounded
    /// blocking opener; a timed-out UART worker must not be retried by its owner.
    pub async fn open(&self) -> io::Result<Connection> {
        match self {
            Self::Uart(path) => Serial::open(path).await.map(Connection::Uart),
            Self::Tcp(address) => {
                if address.port() == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "TCP HAT requires a nonzero port",
                    ));
                }
                let stream = connect(TcpStream::connect(address)).await?;
                stream.set_nodelay(true)?;
                Ok(Connection::Tcp(stream))
            }
        }
    }
}
async fn connect(connecting: impl Future<Output = io::Result<TcpStream>>) -> io::Result<TcpStream> {
    // WAIT: atx-tcp-connect
    tokio::time::timeout(Duration::from_secs(2), connecting)
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "TCP HAT connect exceeded its deadline",
            )
        })?
}

/// The transport belongs to the existing monitor/transaction owner. No helper
/// task, framing, read buffer or command retry is introduced here.
pub enum Connection {
    Uart(Serial),
    Tcp(TcpStream),
}
impl From<Serial> for Connection {
    fn from(value: Serial) -> Self {
        Self::Uart(value)
    }
}
impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Uart(s) => Pin::new(s).poll_read(cx, buffer),
            Self::Tcp(s) => Pin::new(s).poll_read(cx, buffer),
        }
    }
}
impl DrainRead for Connection {
    fn poll_drain(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Drain>> {
        match self.get_mut() {
            Self::Uart(s) => Pin::new(s).poll_drain(cx, buffer),
            Self::Tcp(s) => Pin::new(s).poll_drain(cx, buffer),
        }
    }
}
impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Uart(s) => Pin::new(s).poll_write(cx, bytes),
            Self::Tcp(s) => Pin::new(s).poll_write(cx, bytes),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Uart(s) => Pin::new(s).poll_flush(cx),
            Self::Tcp(s) => Pin::new(s).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Uart(s) => Pin::new(s).poll_shutdown(cx),
            Self::Tcp(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests;
