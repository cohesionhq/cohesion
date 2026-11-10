//! Decoding tests for `Request`. A command without arguments is sent with
//! `args` absent by some clients and `null` or `{}` by others; each must
//! read as the same command, and one with arguments must still need them.
//! A client's `toolUse` may leave out its `category`, as clients written
//! before it did.

use super::*;
use crate::model::{ActorMessageContent, ActorToolKind};
use crate::protocol::{ActorSessionReport, ListCatalogModelsRequest, SubagentLaunch};

fn decode(json: &str) -> Result<Request, serde_json::Error> {
    serde_json::from_str(json)
}

#[test]
fn a_command_without_arguments_reads_with_args_absent_null_or_empty() {
    for json in [
        r#"{"type":"listClients"}"#,
        r#"{"type":"listClients","args":null}"#,
        r#"{"type":"listClients","args":{}}"#,
        r#"{"id":7,"type":"listIntelligenceEndpoints","args":{}}"#,
    ] {
        let request = decode(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        assert!(matches!(
            request.command,
            Command::ListClients | Command::ListIntelligenceEndpoints
        ));
    }
    assert_eq!(
        decode(r#"{"id":7,"type":"listClients"}"#).unwrap().id,
        Some(7)
    );
}

#[test]
fn a_command_whose_arguments_are_all_optional_reads_with_empty_args() {
    let request = decode(r#"{"type":"listCatalogModels","args":{}}"#).unwrap();
    assert!(matches!(
        request.command,
        Command::ListCatalogModels(ListCatalogModelsRequest {
            provider_identifier: None,
            query: None,
        })
    ));
}

#[test]
fn a_command_with_arguments_still_needs_them() {
    assert!(decode(r#"{"type":"deleteIntelligenceEndpoint","args":{}}"#).is_err());
    assert!(decode(r#"{"type":"deleteIntelligenceEndpoint"}"#).is_err());
    assert!(decode(r#"{"type":"deleteIntelligenceEndpoint","args":{"id":1}}"#).is_ok());
}

/// The category of the `toolUse` an `actorSessionUpdate` frame carries.
fn tool_category(json: &str) -> ActorToolKind {
    let frame: ClientFrame = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
    match frame {
        ClientFrame::ActorSessionUpdate(ActorSessionUpdate {
            report:
                ActorSessionReport::Message {
                    content: ActorMessageContent::ToolUse { category, .. },
                    ..
                },
            ..
        }) => category,
        _ => panic!("{json}: not a toolUse message"),
    }
}

#[test]
fn a_tool_use_reads_its_category_and_is_other_without_one() {
    assert_eq!(
        tool_category(
            r#"{"type":"actorSessionUpdate","id":1,"session_id":3,"kind":"message","content":{"kind":"toolUse","tool":"Bash","category":"command","summary":"cargo test","ended_at":null}}"#
        ),
        ActorToolKind::Command
    );
    assert_eq!(
        tool_category(
            r#"{"type":"actorSessionUpdate","id":2,"session_id":3,"kind":"message","content":{"kind":"toolUse","tool":"Edit","summary":"src/main.rs"}}"#
        ),
        ActorToolKind::Other
    );
    // A category Cohesion doesn't know is refused, not guessed at.
    let unknown = r#"{"type":"actorSessionUpdate","id":3,"session_id":3,"kind":"message","content":{"kind":"toolUse","tool":"Edit","category":"write","summary":"src/main.rs"}}"#;
    assert!(serde_json::from_str::<ClientFrame>(unknown).is_err());
}

#[test]
fn an_end_is_read_only_as_end() {
    let end = |kind: &str| {
        serde_json::from_str::<ClientFrame>(&format!(
            r#"{{"type":"actorSessionUpdate","id":1,"session_id":3,"kind":"{kind}","message_id":44}}"#
        ))
    };
    assert!(matches!(
        end("end").unwrap(),
        ClientFrame::ActorSessionUpdate(ActorSessionUpdate {
            report: ActorSessionReport::End { message_id: 44 },
            ..
        })
    ));
    assert!(end("toolEnd").is_err());
}

#[test]
fn a_report_names_a_subagent_by_key_and_a_launch_reads_its_fields() {
    let frame: ClientFrame = serde_json::from_str(
        r#"{"type":"actorSessionUpdate","id":5,"session_id":12,"subagent":"a1b2","kind":"launch","title":"Map the db layer","message_id":311,"agent_type":"Explore","background":true}"#,
    )
    .unwrap();
    let ClientFrame::ActorSessionUpdate(ActorSessionUpdate {
        session_id: 12,
        subagent: Some(subagent),
        report: ActorSessionReport::Launch(launch),
        ..
    }) = frame
    else {
        panic!("not a launch");
    };
    assert_eq!(subagent, "a1b2");
    assert_eq!(
        launch,
        SubagentLaunch {
            title: "Map the db layer".to_string(),
            parent: None,
            message_id: Some(311),
            agent_type: Some("Explore".to_string()),
            model: None,
            background: true,
        }
    );
    // Left out, a report is about the session itself.
    let frame: ClientFrame = serde_json::from_str(
        r#"{"type":"actorSessionUpdate","id":6,"session_id":12,"kind":"title","title":"Fix it"}"#,
    )
    .unwrap();
    assert!(matches!(
        frame,
        ClientFrame::ActorSessionUpdate(ActorSessionUpdate { subagent: None, .. })
    ));
}

#[test]
fn an_unknown_command_is_refused_by_name_alone() {
    let error = |json: &str| serde_json::from_str::<ClientFrame>(json).err().unwrap();
    assert_eq!(
        error(r#"{"type":"listActorTasks","id":4}"#).to_string(),
        "unknown command listActorTasks"
    );
    assert_eq!(
        serde_json::from_str::<Request>(r#"{"type":"listActorTasks","id":4,"args":{}}"#)
            .err()
            .unwrap()
            .to_string(),
        "unknown command listActorTasks"
    );
}
