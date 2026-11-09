//! The client envelope in `cohesion-rs` against the daemon's own frames:
//! clients decode what the daemon sends with those types, and send their
//! requests with them, so each must match the frames here on the wire.

use cohesion_rs::protocol as client;

use super::*;
use crate::protocol::{ErrorCode, EvictionReason, WebBundle};

/// `message` as the daemon sends it, read back as a client reads it.
fn as_client_reads(message: ServerMessage) -> client::Frame {
    let json = serde_json::to_string(&Frame { seq: 9, message }).unwrap();
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn reply(outcome: Outcome) -> client::Outcome {
    let frame = as_client_reads(ServerMessage::Reply(Reply {
        id: Some(4),
        outcome,
    }));
    assert_eq!(frame.seq, 9);
    let client::ServerMessage::Reply(reply) = frame.message else {
        panic!("not a reply");
    };
    assert_eq!(reply.id, Some(4));
    reply.outcome
}

#[test]
fn a_client_reads_the_daemons_hello() {
    let frame = as_client_reads(ServerMessage::Hello(Hello {
        protocol: PROTOCOL_VERSION,
        server: "host".into(),
        client_name: "laptop".into(),
        web_bundle_hash: Some("abc".into()),
        vocabulary: Box::new(Vocabulary::new()),
    }));
    let client::ServerMessage::Hello(hello) = frame.message else {
        panic!("not a hello");
    };
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    assert_eq!(hello.server, "host");
    assert_eq!(hello.client_name, "laptop");
    assert_eq!(hello.web_bundle_hash.as_deref(), Some("abc"));
}

#[test]
fn a_client_reads_the_web_bundle() {
    let bundle = WebBundle::new([("index.html", b"deflated".as_slice())]);
    let hash = bundle.hash.clone();
    let client::Outcome::Ok(client::ReplyData::WebBundle(read)) =
        reply(Outcome::Ok(ReplyData::WebBundle(bundle)))
    else {
        panic!("not a web bundle");
    };
    assert_eq!(read.hash, hash);
    assert_eq!(read.files.len(), 1);
    assert_eq!(read.files[0].path, "index.html");
    assert_eq!(read.files[0].deflated().unwrap(), b"deflated");
}

#[test]
fn a_client_reads_any_other_reply_as_json() {
    let client::Outcome::Ok(client::ReplyData::Other { kind, data }) =
        reply(Outcome::Ok(ReplyData::Tasks(Vec::new())))
    else {
        panic!("not Other");
    };
    assert_eq!(kind, "tasks");
    assert_eq!(data, serde_json::json!([]));
}

#[test]
fn a_client_reads_an_error() {
    let client::Outcome::Error(error) = reply(Outcome::Error(ServerError::validation(
        Some("name".into()),
        "Too long.",
    ))) else {
        panic!("not an error");
    };
    assert_eq!(
        serde_json::Value::from(error.code),
        serde_json::to_value(ErrorCode::ValidationFailed).unwrap()
    );
    assert_eq!(error.message, "Too long.");
    assert_eq!(error.field.as_deref(), Some("name"));
}

#[test]
fn a_client_reads_every_other_message_as_json() {
    for (message, kind) in [
        (ServerMessage::Reset, "reset"),
        (
            ServerMessage::Invalidate(InvalidationBatch { keys: Vec::new() }),
            "invalidate",
        ),
        (
            ServerMessage::Evicted {
                reason: EvictionReason::Revoked,
            },
            "evicted",
        ),
    ] {
        let client::ServerMessage::Other(value) = as_client_reads(message).message else {
            panic!("{kind} not Other");
        };
        assert_eq!(value["type"], kind);
    }
}

#[test]
fn the_daemon_reads_a_clients_web_bundle_request() {
    let json = serde_json::to_string(&client::Request {
        id: Some(2),
        command: client::WebBundleRequest.into(),
    })
    .unwrap();
    let request: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(request.id, Some(2));
    assert!(matches!(request.command, Command::WebBundle));
}
