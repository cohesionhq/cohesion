//! The network front of the daemon: HTTPS (one-off commands, `/register`, and
//! WebSocket sessions) behind mutual TLS, plus a Unix socket for local
//! clients. Each transport first establishes which client is connecting
//! (`peer_acceptor` from the certificate, `local_acceptor` from the socket's
//! handshake), then feeds frames into a [`Session`](session::Session) or a
//! one-off request handler; a session a client offers on also
//! carries the daemon's calls into it. Behind a pass-through router,
//! `proxy_protocol` gives the client's real address.

mod client_cert_verifier;
mod first_byte;
mod http_routes;
mod peer_acceptor;
mod proxy_protocol;
mod request_handler;
mod serve;
mod session;
mod tls_config;
mod transport;
mod ws_routes;

#[cfg(unix)]
mod local_acceptor;
#[cfg(unix)]
mod unix_socket;
#[cfg(unix)]
mod unix_socket_exchange;

pub use serve::start;
