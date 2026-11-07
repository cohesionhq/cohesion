//! A stream whose peer must send its first byte within a deadline. After
//! the TLS handshake, hyper tells HTTP/1.1 from HTTP/2 by reading the first
//! bytes, and that wait has no timer: a stranger with any certificate that
//! then stays silent would hold the connection, and a file descriptor,
//! forever. Once the first byte is in, hyper's own timeouts take over.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Sleep, sleep};

pub struct FirstByte<S> {
    inner: S,
    /// Armed until the peer's first byte arrives.
    deadline: Option<Pin<Box<Sleep>>>,
}

impl<S> FirstByte<S> {
    /// `inner`, whose first read must deliver a byte within `within` or fail
    /// as timed out.
    pub fn new(inner: S, within: Duration) -> Self {
        Self {
            inner,
            deadline: Some(Box::pin(sleep(within))),
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for FirstByte<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Some(deadline) = this.deadline.as_mut()
            && deadline.as_mut().poll(cx).is_ready()
        {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the client sent nothing",
            )));
        }
        let filled = buf.filled().len();
        let polled = Pin::new(&mut this.inner).poll_read(cx, buf);
        if matches!(polled, Poll::Ready(Ok(()))) && buf.filled().len() > filled {
            this.deadline = None;
        }
        polled
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for FirstByte<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests;
