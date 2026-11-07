//! The HTTPS routes: one-off commands, client registration, and the gate
//! that keeps unregistered clients out of everything but `/register`.

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, body::Bytes};
use cohesionkit::db::RegisterClientCommand;
use cohesionkit::log::Module;
use cohesionkit::protocol::{Frame, Outcome, RegisterRequest, Reply, ServerError, ServerMessage};
use cohesionkit::{Caller, Client, ClientRequest, CohesionContext, log_info, log_warn};

use super::peer_acceptor::{Peer, UnknownPeers};
use super::request_handler::{bad_frame, encode, handle_request};
use super::transport::ReceivableMessage;

pub async fn handle_http(
    State(context): State<CohesionContext>,
    Extension(caller): Extension<Caller>,
    Extension(client): Extension<Client>,
    command: String,
) -> impl IntoResponse {
    let asking = ClientRequest {
        context: &context,
        client: &client,
    };
    let reply = match ReceivableMessage(command).request() {
        Ok((request, kind)) => handle_request(request, &kind, &asking, &caller).await,
        Err(bad) => bad_frame(bad, &caller),
    };
    one_off(reply)
}

/// Registers the caller's certificate. Whatever goes wrong, the stranger on
/// the other end hears the same 401: it is told nothing about why, and the
/// refusal is recorded. The setup key is checked before any database work,
/// so a wrong one costs nothing.
pub async fn handle_register(
    State(context): State<CohesionContext>,
    Extension(peer): Extension<Peer>,
    Extension(caller): Extension<Caller>,
    Extension(unknown): Extension<UnknownPeers>,
    body: Bytes,
) -> Response {
    let refuse = |why: &str| {
        log_warn!(Module::Http, "{caller}: registration refused: {why}");
        context.security.pairing_refused(peer.ip);
        refused()
    };
    let Ok(request) = serde_json::from_slice::<RegisterRequest>(&body) else {
        return refuse("malformed");
    };
    if !context.secrets.verify_setup_key(&request.setup_key) {
        return refuse("wrong setup key");
    }
    let command = RegisterClientCommand {
        setup_key: request.setup_key,
        fingerprint: peer.fingerprint.clone(),
        client_name: request.client_name,
        address: peer.ip.to_canonical().to_string(),
    };
    let registered = context
        .work
        .run_new(
            format!("{caller}: register"),
            context.database.send(command),
        )
        .await;
    match registered {
        Err(shutting_down) => one_off(Reply {
            id: None,
            outcome: Outcome::Error(shutting_down.into()),
        })
        .into_response(),
        Ok(Ok(client)) => {
            unknown.forget(&peer.fingerprint);
            log_info!(
                Module::Http,
                "{caller}: registered client {} (id {})",
                client.client_name,
                client.id
            );
            one_off(Reply {
                id: None,
                outcome: Outcome::Ok(client.into()),
            })
            .into_response()
        }
        Ok(Err(error)) => refuse(&ServerError::from(error).message),
    }
}

fn refused() -> Response {
    (StatusCode::UNAUTHORIZED, "The setup code wasn’t accepted.").into_response()
}

/// Rejects requests from clients that haven't registered, and hands the
/// routes behind it the registered [`Client`](cohesionkit::Client).
pub async fn require_registered(
    Extension(peer): Extension<Peer>,
    mut request: Request,
    next: Next,
) -> Response {
    match peer.client {
        Some(client) => {
            request.extensions_mut().insert(client);
            next.run(request).await
        }
        None => (
            StatusCode::FORBIDDEN,
            "This client isn’t registered. Run `cohesion setup` with the setup code.",
        )
            .into_response(),
    }
}

fn one_off(reply: Reply) -> impl IntoResponse {
    // One-off: no session, so the only frame is always seq 0.
    let encoded = encode(&Frame {
        seq: 0,
        message: ServerMessage::Reply(reply),
    });
    ([(header::CONTENT_TYPE, "application/json")], encoded)
}
