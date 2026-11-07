//! `cohesion open`: serves the web UI on loopback and proxies its API calls to cohesiond
//! over this client's own connection, so the browser needs no identity of its own.
//! `guard` decides who may call the API, `launch_token` keeps the secret it checks,
//! `launch_link` hands it to the browser without putting it on a command line,
//! `bridge` relays WebSocket sessions, and `static_assets` serves the build cohesiond
//! hands out.

mod bridge;
mod browser;
mod guard;
mod launch_link;
pub mod launch_token;
pub mod static_assets;

use std::{io::ErrorKind, net::Ipv4Addr, sync::Arc, time::Instant};

use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade},
    handler::Handler,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{any, get},
};
use serde::Serialize;
use tokio::net::TcpListener;

use self::{guard::Guard, launch_link::LaunchLink, static_assets::Assets};
use crate::{cli_error::CliError, connection::Connection};

/// Where the UI is served unless `--port` says otherwise, so its address stays put.
const DEFAULT_PORT: u16 = 7343;

/// The largest message the UI may send: room for a message with several images attached,
/// base64-encoded. A browser sends a message as one frame, so a frame may be as large.
const MESSAGE_LIMIT: usize = 64 * 1024 * 1024;

/// What `/client` tells the UI about the CLI it runs behind.
#[derive(Serialize, Clone)]
pub struct ClientInfo {
    pub workspace: Option<i64>,
    pub target: String,
}

/// How `cohesion open` was asked to serve.
pub struct Options {
    /// `DEFAULT_PORT`, or a free one if that's taken, when absent.
    pub port: Option<u16>,
    /// The token the UI must present.
    pub token: String,
    pub mode: Mode,
    /// Stop once stdin closes, as well as on Ctrl-C.
    pub exit_with_parent: bool,
}

/// Where the UI comes from.
pub enum Mode {
    /// Serve `assets`, cohesiond's build of the UI, reached through a launch link
    /// that hands the browser the token; open that link in a browser if `browser`.
    Launch { browser: bool, assets: Assets },
    /// Serve only the API, for the Vite dev server, which serves the UI and
    /// adds the token itself.
    Dev,
}

#[derive(Clone)]
struct ProxyState {
    connection: Connection,
    client: ClientInfo,
    launch: Arc<LaunchLink>,
}

/// Serves until Ctrl-C, after printing the URL (and opening it, if asked) for `hostname`'s
/// cohesiond.
pub async fn serve(
    connection: Connection,
    client: ClientInfo,
    hostname: &str,
    options: Options,
) -> Result<(), CliError> {
    let listener = match options.port {
        Some(port) => TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await,
        None => match TcpListener::bind((Ipv4Addr::LOCALHOST, DEFAULT_PORT)).await {
            Err(err) if err.kind() == ErrorKind::AddrInUse => {
                eprintln!(
                    "Port {DEFAULT_PORT} is in use, so Cohesion is serving on a free port. Bookmarks to {DEFAULT_PORT} won't reach it."
                );
                TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await
            }
            bound => bound,
        },
    }
    .map_err(CliError::CannotServe)?;
    let addr = listener.local_addr().map_err(CliError::CannotServe)?;

    let launch = LaunchLink::new(options.token.clone());
    let (guard, url) = match options.mode {
        Mode::Launch { .. } => {
            let url = format!("http://{addr}/launch/{}", launch.mint(Instant::now()));
            (Guard::launch(addr, options.token), url)
        }
        Mode::Dev => (Guard::dev(addr, options.token), format!("http://{addr}/")),
    };
    println!("Serving Cohesion UI for {hostname} at {url}");
    if let Mode::Launch { browser: true, .. } = options.mode
        && let Err(err) = browser::open(&url)
    {
        eprintln!("Couldn't open a browser ({err}). Open the URL above.");
    }

    let router = router(guard, options.mode).with_state(ProxyState {
        connection,
        client,
        launch,
    });
    tokio::select! {
        served = axum::serve(listener, router) => served.map_err(CliError::CannotServe),
        _ = tokio::signal::ctrl_c() => Ok(()),
        _ = stdin_closed(), if options.exit_with_parent => Ok(()),
    }
}

/// Returns once stdin reaches end of file. The parent holds the pipe's write end, and
/// the kernel closes it however the parent exits.
async fn stdin_closed() {
    let _ = tokio::io::copy(&mut tokio::io::stdin(), &mut tokio::io::sink()).await;
}

fn router(guard: Guard, mode: Mode) -> Router<ProxyState> {
    let api = guard.protect(
        Router::new()
            .route("/client", get(client))
            .route("/connect", any(connect)),
    );
    match mode {
        Mode::Launch { assets, .. } => {
            let launch = guard
                .protect_host(Router::new().route("/launch/{nonce}", get(launch_link::redeem)));
            api.merge(launch)
                .fallback_service(static_assets::serve.with_state(Arc::new(assets)))
        }
        // Vite serves the UI itself.
        Mode::Dev => api.fallback(|| async { StatusCode::NOT_FOUND }),
    }
}

async fn client(State(state): State<ProxyState>) -> Json<ClientInfo> {
    Json(state.client)
}

async fn connect(State(state): State<ProxyState>, upgrade: WebSocketUpgrade) -> Response {
    match state.connection.connect_upstream().await {
        Ok(upstream) => upgrade
            .max_message_size(MESSAGE_LIMIT)
            .max_frame_size(MESSAGE_LIMIT)
            .on_upgrade(|socket| bridge::relay(socket, upstream)),
        Err(err) => (StatusCode::BAD_GATEWAY, err.to_string()).into_response(),
    }
}
