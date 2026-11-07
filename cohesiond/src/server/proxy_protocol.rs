//! Learns a client's real address from a PROXY protocol v2 header, for a daemon
//! behind a pass-through router that holds no keys and so can't say it any other
//! way. The router writes the header ahead of the client's TLS bytes. The header
//! is unauthenticated, so only a peer named by `--proxy-protocol` is asked for
//! one; it is read to its exact length, leaving the TLS bytes for rustls.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::timeout;

/// What opens every header.
const SIGNATURE: [u8; 12] = *b"\r\n\r\n\0\r\nQUIT\n";
/// The signature, then version and command, family and protocol, and the length
/// of what follows.
const PREFIX: usize = 16;
/// Version 2, command PROXY. A trusted router only ever relays a client, so
/// the LOCAL command, which names no client, is malformed here.
const VERSION_2_PROXY: u8 = 0x21;
/// TCP over IPv4, and the size of its source and destination addresses and ports.
const TCP4: u8 = 0x11;
const TCP4_ADDRESSES: usize = 12;
/// TCP over IPv6, and the same.
const TCP6: u8 = 0x21;
const TCP6_ADDRESSES: usize = 36;
/// The most that may follow the prefix: the largest address block the protocol
/// defines. Anything beyond the addresses is an extension, skipped unread.
const MAX_ADDRESSES: usize = 216;
/// How long a trusted peer has to deliver the whole header. The router writes it
/// in one go with the ClientHello, so only a broken peer comes close.
const HEADER_WITHIN: Duration = Duration::from_secs(5);

/// The peers whose PROXY header is believed, by the address the daemon sees them
/// at. With none, nothing is read and the daemon sees only real peers.
#[derive(Clone)]
pub struct TrustedProxies(Arc<[IpAddr]>);

impl TrustedProxies {
    pub fn new(peers: impl IntoIterator<Item = IpAddr>) -> Self {
        Self(peers.into_iter().map(|ip| ip.to_canonical()).collect())
    }

    /// The address to show for the connection from `peer`. A trusted peer must
    /// send a well-formed header in time, or the error ends the connection. Any
    /// other peer is itself the client: nothing is read from it, so a header it
    /// sends goes to rustls, which refuses it as it would any other bytes.
    pub async fn client_address<S: AsyncRead + Unpin>(
        &self,
        stream: &mut S,
        peer: SocketAddr,
    ) -> io::Result<SocketAddr> {
        if self.0.contains(&peer.ip().to_canonical()) {
            read_source(stream).await
        } else {
            Ok(peer)
        }
    }

    /// The trusted peers, or `None` when there are none.
    pub fn described(&self) -> Option<String> {
        (!self.0.is_empty()).then(|| {
            self.0
                .iter()
                .map(IpAddr::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
    }
}

/// Reads one header and returns the source it names, consuming nothing past it.
async fn read_source<S: AsyncRead + Unpin>(stream: &mut S) -> io::Result<SocketAddr> {
    timeout(HEADER_WITHIN, read_header(stream))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "the header did not arrive in time"))?
}

async fn read_header<S: AsyncRead + Unpin>(stream: &mut S) -> io::Result<SocketAddr> {
    let mut prefix = [0; PREFIX];
    stream.read_exact(&mut prefix).await?;
    if prefix[..SIGNATURE.len()] != SIGNATURE {
        return Err(malformed("no signature"));
    }
    if prefix[12] != VERSION_2_PROXY {
        return Err(malformed("not version 2 with command PROXY"));
    }
    let needed = match prefix[13] {
        TCP4 => TCP4_ADDRESSES,
        TCP6 => TCP6_ADDRESSES,
        _ => return Err(malformed("neither TCP over IPv4 nor TCP over IPv6")),
    };
    let length = usize::from(u16::from_be_bytes([prefix[14], prefix[15]]));
    if length < needed {
        return Err(malformed("too short for its addresses"));
    }
    if length > MAX_ADDRESSES {
        return Err(malformed("longer than the limit"));
    }
    let mut addresses = [0; MAX_ADDRESSES];
    stream.read_exact(&mut addresses[..length]).await?;
    Ok(if prefix[13] == TCP4 {
        source_v4(&addresses)
    } else {
        source_v6(&addresses)
    })
}

/// Addresses come first in the block: source, then destination, then their ports.
fn source_v4(block: &[u8; MAX_ADDRESSES]) -> SocketAddr {
    let ip = Ipv4Addr::new(block[0], block[1], block[2], block[3]);
    SocketAddr::new(ip.into(), u16::from_be_bytes([block[8], block[9]]))
}

/// A router on a dual-stack listener can name an IPv4 client in a TCP6 header, as
/// a mapped address; it is shown as the IPv4 address it is.
fn source_v6(block: &[u8; MAX_ADDRESSES]) -> SocketAddr {
    let mut octets = [0; 16];
    octets.copy_from_slice(&block[..16]);
    let ip = IpAddr::from(Ipv6Addr::from(octets)).to_canonical();
    SocketAddr::new(ip, u16::from_be_bytes([block[32], block[33]]))
}

fn malformed(why: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("malformed header: {why}"),
    )
}

#[cfg(test)]
mod tests;
