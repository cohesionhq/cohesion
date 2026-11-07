use cohesion_core::LocalIdentity;
use cohesion_core::model::{
    ActorDefinition, ActorMessageContent, ActorSessionMessageContent, ActorToolKind,
    ActorUserContent,
};
use cohesion_core::protocol::ClientCapabilities;
use cohesion_db::{
    AddActorSessionMessageCommand, Container, CreateActorSessionCommand, DatabaseContext,
    IdentifyLocalClientCommand, ListActorsCommand, Secrets, SyncCapabilitiesCommand,
};

use super::exchange;

/// A database with one actor offered and one session of its, working, whose
/// actor has spoken and run a tool since the user's prompt.
async fn working_session() -> (DatabaseContext, i64) {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let client = db
        .send(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "MacBook".to_string(),
                name: "MacBook".to_string(),
            },
        })
        .await
        .unwrap();
    db.send(SyncCapabilitiesCommand {
        client_id: client.id,
        offers: ClientCapabilities {
            actors: vec![ActorDefinition {
                identifier: "cohesion".to_string(),
                product: "Claude Code".to_string(),
                scope: "/src/cohesion".to_string(),
                modes: Vec::new(),
                models: Vec::new(),
            }],
        },
    })
    .await
    .unwrap();
    let actor_id = db.send(ListActorsCommand).await.unwrap()[0].id;
    let (session, _) = db
        .send(CreateActorSessionCommand {
            actor_id,
            prompt: "Fix the build".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .await
        .unwrap();
    for content in [
        ActorMessageContent::Text {
            text: "Looking.".to_string(),
        },
        ActorMessageContent::Thinking {
            text: "Maybe the linker.".to_string(),
            ended_at: None,
        },
        ActorMessageContent::ToolUse {
            tool: "Edit".to_string(),
            category: ActorToolKind::Edit,
            summary: "build.rs".to_string(),
            ended_at: None,
        },
    ] {
        say(&db, session.id, ActorSessionMessageContent::Actor(content)).await;
    }
    (db, session.id)
}

async fn say(db: &DatabaseContext, session_id: i64, content: ActorSessionMessageContent) {
    db.send(AddActorSessionMessageCommand {
        session_id,
        content,
        attachments: Vec::new(),
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn an_exchange_keeps_the_prompt_and_only_what_the_actor_said_since_last_told() {
    let (db, session_id) = working_session().await;
    let first = exchange(&db, session_id).await.unwrap();
    assert!(first.prompt.is_some());
    assert!(first.said.contains("Looking."));
    assert!(!first.said.contains("build.rs"), "tool uses are left out");
    assert!(!first.said.contains("linker"), "thinking is left out");

    say(
        &db,
        session_id,
        ActorSessionMessageContent::User(ActorUserContent::Text {
            text: "Also the docs".to_string(),
        }),
    )
    .await;
    say(
        &db,
        session_id,
        ActorSessionMessageContent::Actor(ActorMessageContent::Text {
            text: "Docs too.".to_string(),
        }),
    )
    .await;
    let later = exchange(&db, session_id).await.unwrap();
    assert!(later.said.contains("Docs too.") && !later.said.contains("Looking."));
}
