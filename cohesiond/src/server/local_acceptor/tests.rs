use cohesionkit::db::{Container, DeleteClientCommand, Secrets};
use cohesionkit::{CohesionContext, LocalHandshakeReply, LocalIdentity};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::accept;
use crate::server::transport::{Exchange, ExchangeReader, Incoming};

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(container, Secrets::new(), test_server())
}

fn identity_line(key: &str, name: &str) -> String {
    let identity = LocalIdentity {
        key: key.to_string(),
        name: name.to_string(),
    };
    format!("{}\n", serde_json::to_string(&identity).unwrap())
}

/// Connects a client that writes `sent` at once, accepts the daemon's end,
/// and returns what was accepted with the client's end, the reply read.
async fn handshake(
    context: &CohesionContext,
    sent: String,
) -> (
    Option<(
        super::SocketExchange,
        cohesionkit::Caller,
        cohesionkit::Client,
    )>,
    LocalHandshakeReply,
) {
    let (daemon, mut client) = UnixStream::pair().unwrap();
    client.write_all(sent.as_bytes()).await.unwrap();
    let accepted = accept(daemon, context).await;
    let mut lines = BufReader::new(client).lines();
    let reply = lines.next_line().await.unwrap().unwrap();
    (accepted, serde_json::from_str(&reply).unwrap())
}

#[tokio::test]
async fn a_frame_sent_with_the_identity_reaches_the_session() {
    let context = context();
    // The first frame arrives in the same read as the identity.
    let sent = identity_line("k", "Studio") + "{\"type\":\"listWorkspaces\",\"id\":1}\n";
    let (accepted, reply) = handshake(&context, sent).await;
    assert_eq!(reply, LocalHandshakeReply::Accepted);
    let (exchange, caller, client) = accepted.unwrap();
    assert_eq!(client.client_name, "Studio");
    assert_eq!(caller.client.unwrap().id, client.id);
    let (mut reader, _writer) = exchange.split();
    let Ok(Some(Incoming::Frame(frame))) = reader.recv().await else {
        panic!("expected the frame sent with the identity");
    };
    assert_eq!(frame.0, "{\"type\":\"listWorkspaces\",\"id\":1}");
}

#[tokio::test]
async fn a_revoked_key_or_a_malformed_line_gets_no_session() {
    let context = context();
    let (accepted, _) = handshake(&context, identity_line("k", "Studio")).await;
    let (_, _, client) = accepted.unwrap();
    context
        .database
        .send(DeleteClientCommand { id: client.id })
        .await
        .unwrap();

    let (accepted, reply) = handshake(&context, identity_line("k", "Studio")).await;
    assert!(accepted.is_none());
    assert!(matches!(reply, LocalHandshakeReply::Refused { .. }));

    let (accepted, reply) =
        handshake(&context, "{\"type\":\"listWorkspaces\"}\n".to_string()).await;
    assert!(accepted.is_none());
    assert!(matches!(reply, LocalHandshakeReply::Refused { .. }));
}

#[tokio::test]
async fn an_endless_first_line_gets_no_session() {
    let context = context();
    let (daemon, mut client) = UnixStream::pair().unwrap();
    client.write_all(&vec![b'x'; 64 * 1024]).await.unwrap();
    assert!(accept(daemon, &context).await.is_none());
}

fn test_server() -> cohesionkit::ServerAddress {
    cohesionkit::ServerAddress {
        host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
        port: 3000,
        fingerprint: String::new(),
    }
}
