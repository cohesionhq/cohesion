//! Runs one received request as a command and encodes server frames.

use cohesionkit::log::Module;
use cohesionkit::protocol::{Frame, Outcome, Reply, Request, ServerError};
use cohesionkit::{Caller, ClientRequest, Transport, log_debug, log_warn};

use super::transport::BadFrame;

pub fn encode(frame: &Frame) -> String {
    serde_json::to_string(frame).expect("server frames always serialize")
}

/// Runs `request` (whose command's wire `type` is `kind`) as `asking` says:
/// against its context, from its client, which `caller` describes; counted as daemon work until it replies. Once the daemon is
/// shutting down, every command is refused, and once the client is
/// revoked, every one it sent: one its session read before ending, or one
/// sent over a connection opened before.
pub async fn handle_request(
    request: Request,
    kind: &str,
    asking: &ClientRequest<'_>,
    caller: &Caller,
) -> Reply {
    let module = module_for(caller.transport);
    if asking.context.presence.revoked(asking.client.id) {
        log_debug!(module, "{caller}: {kind} refused; the client was revoked");
        return Reply {
            id: request.id,
            outcome: Outcome::Error(ServerError::unauthorized("This client was revoked.")),
        };
    }
    let outcome = asking
        .context
        .work
        .run_new(
            format!("{caller}: {kind}"),
            request.command.dispatch(asking),
        )
        .await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(shutting_down) => {
            log_debug!(module, "{caller}: {kind} refused; shutting down");
            return Reply {
                id: request.id,
                outcome: Outcome::Error(shutting_down.into()),
            };
        }
    };
    match &outcome {
        Outcome::Ok(_) => log_debug!(module, "{caller}: {kind} ok"),
        Outcome::Error(error) => log_debug!(
            module,
            "{caller}: {kind} failed ({:?}): {}",
            error.code,
            error.message
        ),
    }
    Reply {
        id: request.id,
        outcome,
    }
}

/// The bad-request reply to a frame that could not be read, with a warning.
pub fn bad_frame((id, message): BadFrame, caller: &Caller) -> Reply {
    log_warn!(
        module_for(caller.transport),
        "{caller}: bad frame: {message}"
    );
    Reply {
        id,
        outcome: Outcome::Error(ServerError::bad_request(message)),
    }
}

/// The log module for sessions and requests arriving over `transport`.
pub(super) fn module_for(transport: Transport) -> Module {
    match transport {
        Transport::Http => Module::Http,
        Transport::WebSocket => Module::WebSocket,
        Transport::UnixSocket => Module::UnixSocket,
        Transport::Internal => Module::Server,
    }
}
