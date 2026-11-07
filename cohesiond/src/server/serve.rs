//! Starts the HTTPS server and the local socket, and runs until one exits.
//! [`start`] returns the error that stopped it, so the daemon can exit
//! non-zero when it could not serve.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{any, post};
use axum::{Extension, Router, middleware};
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use cohesionkit::log::Module;
use cohesionkit::{Certificate, CohesionContext, log_error, log_info};
use hyper_util::rt::TokioTimer;

#[cfg(unix)]
use super::local_acceptor;
use super::peer_acceptor::{PeerAcceptor, UnknownPeers};
use super::proxy_protocol::TrustedProxies;
#[cfg(unix)]
use super::session::Session;
#[cfg(unix)]
use super::unix_socket::SocketServer;
use super::{http_routes, tls_config, ws_routes};

/// The largest request body, or message on `/connect`: room for a message
/// with several images attached, base64-encoded.
pub(super) const BODY_LIMIT: usize = 64 * 1024 * 1024;

/// The largest `/register` body, which a stranger may send: a key and a name.
const REGISTER_BODY_LIMIT: usize = 4 * 1024;

/// How long an HTTP/1.1 peer has to send a request's headers, between
/// requests on a kept-alive connection included.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How often an idle HTTP/2 peer is pinged, and how long it has to answer
/// before the connection is dropped.
const H2_KEEPALIVE: Duration = Duration::from_secs(30);
const H2_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn start(
    context: CohesionContext,
    certificate: Certificate,
    port: u16,
    socket_path: PathBuf,
    proxy_protocol: Vec<IpAddr>,
) -> std::io::Result<()> {
    let proxies = TrustedProxies::new(proxy_protocol);
    if let Some(peers) = proxies.described() {
        log_info!(Module::Server, "believing PROXY headers from {peers}");
    }
    let unknown_peers = UnknownPeers::default();
    let registered_only = Router::new()
        .route("/command", post(http_routes::handle_http))
        .route("/connect", any(ws_routes::handle_ws))
        .route_layer(middleware::from_fn(http_routes::require_registered))
        .layer(DefaultBodyLimit::max(BODY_LIMIT));
    let app = Router::new()
        .merge(registered_only)
        .route(
            "/register",
            post(http_routes::handle_register).layer(DefaultBodyLimit::max(REGISTER_BODY_LIMIT)),
        )
        .layer(Extension(unknown_peers.clone()))
        .with_state(context.clone());
    let tls = RustlsConfig::from_config(Arc::new(tls_config::server_config(certificate)));
    let acceptor = PeerAcceptor::new(
        RustlsAcceptor::new(tls),
        context.database.clone(),
        context.security.clone(),
        unknown_peers,
        proxies,
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let mut server = axum_server::bind(addr).acceptor(acceptor);
    // hyper has no clock of its own: without one its timeouts are silently
    // off, and a peer that sends nothing after its first byte holds the
    // connection forever (the acceptor bounds the wait for the first byte).
    let http = server.http_builder();
    http.http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT);
    // Lets `/connect` upgrade over HTTP/2 (RFC 8441) instead of forcing HTTP/1.1.
    http.http2()
        .timer(TokioTimer::new())
        .enable_connect_protocol()
        .keep_alive_interval(H2_KEEPALIVE)
        .keep_alive_timeout(H2_KEEPALIVE_TIMEOUT);
    log_info!(Module::Server, "listening on https://{addr}");
    tokio::select! {
        res = server.serve(app.into_make_service()) => match res {
            Err(err) => {
                log_error!(Module::Server, "cohesiond exited: {err}");
                Err(err)
            }
            _ => {
                log_info!(Module::Server, "The server stopped.");
                Ok(())
            }
        },
        Err(err) = serve_socket(socket_path, context) => {
            log_error!(Module::Server, "cohesiond socket exited: {err}");
            Err(err)
        }
    }
}

#[cfg(unix)]
async fn serve_socket(path: PathBuf, context: CohesionContext) -> std::io::Result<()> {
    let server = SocketServer::new(&path)?;
    log_info!(Module::Server, "listening on {}", path.display());
    server
        .run(|stream| {
            let context = context.clone();
            async move {
                if let Some((exchange, caller, client)) =
                    local_acceptor::accept(stream, &context).await
                {
                    Session::new(exchange, &context, caller, client)
                        .attach()
                        .await;
                }
            }
        })
        .await
}

/// Local sockets are Unix-only; elsewhere this never completes so HTTP runs alone.
#[cfg(not(unix))]
async fn serve_socket(_path: PathBuf, _context: CohesionContext) -> std::io::Result<()> {
    std::future::pending().await
}
