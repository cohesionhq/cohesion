//! Tells each connection's requests which client is on the other end, and
//! notes when and from where each client connects.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use std::{future::Future, io, pin::Pin};

use axum::{Extension, middleware::AddExtension};
use axum_server::{accept::Accept, tls_rustls::RustlsAcceptor};
use cohesionkit::db::{DatabaseContext, LookupClientByFingerprintCommand};
use cohesionkit::log::Module;
use cohesionkit::{
    Caller, CallerClient, Client, Security, Transport, key_fingerprint, log_debug, log_warn,
};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tower_layer::Layer;

use super::first_byte::FirstByte;
use super::proxy_protocol::TrustedProxies;

/// How long a peer has, once its handshake is done, to send anything at all.
const FIRST_BYTE: Duration = Duration::from_secs(10);

/// Who is on the other end of a connection, attached to each of its requests.
#[derive(Clone)]
pub struct Peer {
    /// The key fingerprint of the certificate it presented.
    pub fingerprint: String,
    /// The client trusting that certificate; `None` until it registers, when
    /// it may only call `/register`.
    pub client: Option<Client>,
    /// The address it connected from, as a trusted proxy's header gives it. It
    /// is shown and recorded and admits nothing: a certificate and a setup key do.
    pub ip: IpAddr,
}

/// Certificate fingerprints recently found to belong to no client, so a
/// stranger reconnecting costs the database nothing. Bounded in size and in
/// time; a registration makes its certificate forgotten at once.
#[derive(Clone, Default)]
pub struct UnknownPeers {
    state: Arc<Mutex<UnknownState>>,
}

#[derive(Default)]
struct UnknownState {
    seen: HashMap<String, Instant>,
    /// Counts forgettings, so a lookup that began before one is not remembered.
    epoch: u64,
}

impl UnknownPeers {
    /// How long a fingerprint stays remembered as unknown.
    const TTL: Duration = Duration::from_secs(30);
    /// The most fingerprints remembered at once.
    const CAPACITY: usize = 1024;

    fn state(&self) -> MutexGuard<'_, UnknownState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether `fingerprint` was found unknown within the last [`TTL`](Self::TTL).
    fn contains(&self, fingerprint: &str, now: Instant) -> bool {
        self.state()
            .seen
            .get(fingerprint)
            .is_some_and(|&until| now < until)
    }

    /// The epoch to pass to [`remember`](Self::remember), read before the lookup.
    fn epoch(&self) -> u64 {
        self.state().epoch
    }

    /// Remembers `fingerprint` as unknown, unless it was [`forget`](Self::forget)ed
    /// since `epoch`: a registration finished while the lookup was running.
    fn remember(&self, fingerprint: String, epoch: u64, now: Instant) {
        let mut state = self.state();
        if state.epoch != epoch {
            return;
        }
        state.seen.retain(|_, &mut until| now < until);
        if state.seen.len() >= Self::CAPACITY
            && let Some(oldest) = state
                .seen
                .iter()
                .min_by_key(|(_, until)| **until)
                .map(|(fingerprint, _)| fingerprint.clone())
        {
            state.seen.remove(&oldest);
        }
        state.seen.insert(fingerprint, now + Self::TTL);
    }

    /// Drops `fingerprint`: it just registered, and the next lookup must find it.
    pub fn forget(&self, fingerprint: &str) {
        let mut state = self.state();
        state.epoch += 1;
        state.seen.remove(fingerprint);
    }
}

/// Learns the client's real address, completes the TLS handshake, then looks up the client certificate it
/// presented, noting when a client connects.
/// The stream it hands on must speak within [`FIRST_BYTE`].
#[derive(Clone)]
pub struct PeerAcceptor {
    inner: RustlsAcceptor,
    context: DatabaseContext,
    security: Security,
    unknown: UnknownPeers,
    proxies: TrustedProxies,
}
impl PeerAcceptor {
    pub fn new(
        inner: RustlsAcceptor,
        context: DatabaseContext,
        security: Security,
        unknown: UnknownPeers,
        proxies: TrustedProxies,
    ) -> Self {
        Self {
            inner,
            context,
            security,
            unknown,
            proxies,
        }
    }
}

/// Concrete over `TcpStream` (what `axum_server::bind` hands us) so the
/// remote address can be read before the stream is wrapped in TLS.
impl<S> Accept<TcpStream, S> for PeerAcceptor
where
    S: Send + 'static,
{
    type Stream = FirstByte<TlsStream<TcpStream>>;
    type Service = AddExtension<AddExtension<S, Peer>, Caller>;
    type Future = Pin<Box<dyn Future<Output = io::Result<(Self::Stream, Self::Service)>> + Send>>;

    fn accept(&self, mut stream: TcpStream, service: S) -> Self::Future {
        let acceptor = self.inner.clone();
        let context = self.context.clone();
        let security = self.security.clone();
        let unknown = self.unknown.clone();
        let proxies = self.proxies.clone();
        let peer_addr = stream.peer_addr();
        Box::pin(async move {
            let peer_addr = peer_addr?;
            let peer_addr = proxies
                .client_address(&mut stream, peer_addr)
                .await
                .inspect_err(|err| {
                    log_warn!(Module::Server, "PROXY header from {peer_addr}: {err}");
                })?;
            let ip = peer_addr.ip().to_canonical();
            let address = Some(peer_addr.to_string());
            let (stream, service) = acceptor.accept(stream, service).await.inspect_err(|err| {
                // Debug: anything that finds an open port fails here.
                let from = address.as_deref().unwrap_or("an unknown address");
                log_debug!(Module::Server, "TLS handshake from {from} failed: {err}");
            })?;
            // The verifier makes client certificates mandatory, so one is always here.
            let fingerprint = stream
                .get_ref()
                .1
                .peer_certificates()
                .and_then(|chain| chain.first())
                .and_then(key_fingerprint)
                .ok_or_else(|| io::Error::other("no usable client certificate"))?;
            let client = if unknown.contains(&fingerprint, Instant::now()) {
                None
            } else {
                let epoch = unknown.epoch();
                let found = context
                    .send(LookupClientByFingerprintCommand {
                        fingerprint: fingerprint.clone(),
                    })
                    .await
                    .map_err(|_| io::Error::other("could not look up client"))?;
                if found.is_none() {
                    unknown.remember(fingerprint.clone(), epoch, Instant::now());
                }
                found
            };
            if let Some(client) = &client {
                security.seen(client, Some(ip));
            }
            let caller = Caller {
                transport: Transport::Http,
                client: client.as_ref().map(|client| CallerClient {
                    id: client.id,
                    name: client.client_name.clone(),
                }),
                address,
            };
            let peer = Peer {
                fingerprint,
                client,
                ip,
            };
            let service = Extension(caller).layer(Extension(peer).layer(service));
            Ok((FirstByte::new(stream, FIRST_BYTE), service))
        })
    }
}

#[cfg(test)]
mod tests;
