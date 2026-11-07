//! An [`Exchange`] over a Unix socket stream.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::transport::{
    Exchange, ExchangeError, ExchangeReader, ExchangeResult, ExchangeWriter, Incoming,
    ReceivableMessage,
};

/// The longest identity line a new connection may send.
pub const HANDSHAKE_LINE_LIMIT: usize = 4 * 1024;

/// Frames are newline-delimited JSON; serialized frames never contain a raw newline.
pub struct SocketExchange {
    stream: UnixStream,
    /// Bytes read past the last line, carried from the handshake into the
    /// session's reader.
    buffer: Vec<u8>,
}
impl SocketExchange {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
        }
    }

    /// The next line of at most `limit` bytes, for the handshake before the
    /// session; `None` once the peer has hung up.
    pub async fn read_line(&mut self, limit: usize) -> ExchangeResult<Option<String>> {
        next_line(&mut self.stream, &mut self.buffer, limit).await
    }

    /// Writes `line`, for the handshake before the session.
    pub async fn write_line(&mut self, line: String) -> ExchangeResult<()> {
        write_line(&mut self.stream, line).await
    }
}
impl Exchange for SocketExchange {
    type Reader = SocketReader;
    type Writer = OwnedWriteHalf;
    fn split(self) -> (SocketReader, OwnedWriteHalf) {
        let (read, write) = self.stream.into_split();
        let reader = SocketReader {
            read,
            buffer: self.buffer,
        };
        (reader, write)
    }
}

pub struct SocketReader {
    read: OwnedReadHalf,
    buffer: Vec<u8>,
}
impl ExchangeReader for SocketReader {
    async fn recv(&mut self) -> ExchangeResult<Option<Incoming>> {
        Ok(
            next_line(&mut self.read, &mut self.buffer, super::serve::BODY_LIMIT)
                .await?
                .map(|text| Incoming::Frame(ReceivableMessage(text))),
        )
    }
}

impl ExchangeWriter for OwnedWriteHalf {
    async fn send(&mut self, frame: String) -> ExchangeResult<()> {
        write_line(self, frame).await
    }
}

/// The next non-blank line from `read`, `buffer` holding what was read past
/// the last one. A line longer than `limit` bytes is an error.
async fn next_line<R: AsyncRead + Unpin>(
    read: &mut R,
    buffer: &mut Vec<u8>,
    limit: usize,
) -> ExchangeResult<Option<String>> {
    loop {
        if let Some(end) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=end).collect();
            let line = &line[..end];
            if line.len() > limit {
                return Err(too_long());
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            return match String::from_utf8(line.to_vec()) {
                Ok(text) => Ok(Some(text)),
                Err(e) => Err(ExchangeError::ConnectionError(e.to_string())),
            };
        }
        if buffer.len() > limit {
            return Err(too_long());
        }
        let mut chunk = [0u8; 4096];
        match read.read(&mut chunk).await {
            Ok(0) => return Ok(None),
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(ExchangeError::ConnectionError(e.to_string())),
        }
    }
}

fn too_long() -> ExchangeError {
    ExchangeError::ConnectionError("a line is longer than allowed".to_string())
}

async fn write_line<W: AsyncWriteExt + Unpin>(
    write: &mut W,
    mut line: String,
) -> ExchangeResult<()> {
    line.push('\n');
    write
        .write_all(line.as_bytes())
        .await
        .map_err(|e| ExchangeError::ConnectionError(e.to_string()))
}
