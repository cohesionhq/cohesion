//! Tells each Unix socket connection which client is on the other end, the
//! socket's counterpart of [`PeerAcceptor`](super::peer_acceptor). The socket
//! has no TLS handshake, so the client sends one `LocalIdentity` line and is
//! answered with one `LocalHandshakeReply` line; only then does the
//! connection become a session, which never sees either.

use std::time::Duration;

use cohesionkit::db::IdentifyLocalClientCommand;
use cohesionkit::log::Module;
use cohesionkit::protocol::ServerError;
use cohesionkit::{
    Caller, CallerClient, Client, CohesionContext, LocalHandshakeReply, LocalIdentity, Transport,
    log_info, log_warn,
};
use tokio::net::UnixStream;

use super::unix_socket::daemon_uid;
use super::unix_socket_exchange::{HANDSHAKE_LINE_LIMIT, SocketExchange};

/// How long a new connection has to identify itself.
const DEADLINE: Duration = Duration::from_secs(10);

/// The connection, who it is with and that client, once its identity is
/// accepted; `None` when it was refused, malformed, late or hung up.
pub async fn accept(
    stream: UnixStream,
    context: &CohesionContext,
) -> Option<(SocketExchange, Caller, Client)> {
    let credentials = stream.peer_cred().ok();
    let address = credentials.map(|credentials| format!("uid {}", credentials.uid()));
    let where_from = address.as_deref().unwrap_or("an unknown peer");
    // The socket's mode already keeps other users out; this holds if it ever doesn't.
    if credentials.is_none_or(|credentials| credentials.uid() != daemon_uid()) {
        log_warn!(
            Module::UnixSocket,
            "{where_from}: refused: not the daemon's user"
        );
        return None;
    }
    let mut exchange = SocketExchange::new(stream);
    let line = match tokio::time::timeout(DEADLINE, exchange.read_line(HANDSHAKE_LINE_LIMIT)).await
    {
        Ok(Ok(Some(line))) => line,
        _ => {
            log_info!(
                Module::UnixSocket,
                "{where_from}: closed before identifying itself"
            );
            return None;
        }
    };
    let identified = match serde_json::from_str::<LocalIdentity>(&line) {
        Ok(identity) => identify(context, identity).await,
        Err(e) => Err(format!(
            "The socket's first line must identify the client: {e}"
        )),
    };
    let reply = match &identified {
        Ok(_) => LocalHandshakeReply::Accepted,
        Err(reason) => LocalHandshakeReply::Refused {
            reason: reason.clone(),
        },
    };
    let encoded = serde_json::to_string(&reply).expect("handshake replies always serialize");
    let answered = exchange.write_line(encoded).await;
    let client = match identified {
        Ok(client) => client,
        Err(reason) => {
            log_warn!(Module::UnixSocket, "{where_from}: refused: {reason}");
            return None;
        }
    };
    answered.ok()?;
    context.security.seen(&client, None);
    let caller = Caller {
        transport: Transport::UnixSocket,
        client: Some(CallerClient {
            id: client.id,
            name: client.client_name.clone(),
        }),
        address,
    };
    Some((exchange, caller, client))
}

/// The client `identity` names, created on first sight; daemon work, so
/// refused once the daemon is shutting down.
async fn identify(context: &CohesionContext, identity: LocalIdentity) -> Result<Client, String> {
    let label = format!("local handshake from {}", identity.name);
    context
        .work
        .run_new(
            label,
            context
                .database
                .send(IdentifyLocalClientCommand { identity }),
        )
        .await
        .map_err(|shutting_down| shutting_down.to_string())?
        .map_err(|e| ServerError::from(e).message)
}

#[cfg(test)]
mod tests;
