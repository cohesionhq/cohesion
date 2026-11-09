//! Decoding tests for the client envelope: a message, reply or error code
//! this crate doesn't know must still decode, so a daemon that adds one
//! doesn't break the clients built on it.

use super::*;

fn frame(json: &str) -> Frame {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

#[test]
fn an_unknown_message_decodes_as_its_json() {
    let decoded = frame(r#"{"seq":3,"type":"somethingNew","detail":1}"#);
    assert_eq!(decoded.seq, 3);
    let ServerMessage::Other(value) = decoded.message else {
        panic!("not Other");
    };
    assert_eq!(value["type"], "somethingNew");
    assert_eq!(value["detail"], 1);
}

#[test]
fn an_unknown_reply_decodes_as_its_kind_and_data() {
    let decoded = frame(r#"{"seq":1,"type":"reply","id":4,"ok":{"type":"tasks","data":[]}}"#);
    let ServerMessage::Reply(reply) = decoded.message else {
        panic!("not a reply");
    };
    assert_eq!(reply.id, Some(4));
    let Outcome::Ok(ReplyData::Other { kind, data }) = reply.outcome else {
        panic!("not Other");
    };
    assert_eq!(kind, "tasks");
    assert_eq!(data, Value::Array(Vec::new()));
}

#[test]
fn an_unknown_error_code_decodes() {
    let decoded = frame(
        r#"{"seq":1,"type":"reply","id":null,"error":{"code":"somethingNew","message":"No."}}"#,
    );
    let ServerMessage::Reply(reply) = decoded.message else {
        panic!("not a reply");
    };
    let Outcome::Error(error) = reply.outcome else {
        panic!("not an error");
    };
    assert_eq!(error.code, "somethingNew");
    assert_eq!(error.message, "No.");
    assert_eq!(error.field, None);
}

#[test]
fn a_frame_needs_its_seq_and_type() {
    assert!(serde_json::from_str::<Frame>(r#"{"type":"reset"}"#).is_err());
    assert!(serde_json::from_str::<Frame>(r#"{"seq":1}"#).is_err());
}
