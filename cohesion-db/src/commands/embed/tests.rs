use cohesion_core::LocalIdentity;
use cohesion_core::model::{
    ActorDefinition, ActorMessageContent, ActorSessionMessageContent, ActorUserContent, Embed,
    EmbedItem, EmbedKind, FeedFullPagePolicy, FeedItemCapture, IntelligenceAgentColor,
};
use cohesion_core::protocol::ClientCapabilities;

use super::*;
use crate::test_support::context;
use crate::{
    AddActorSessionMessageCommand, CommandError, CreateActorSessionCommand, CreateDocumentCommand,
    CreateFeedCommand, CreateIntelligenceAgentCommand, CreateTaskCommand, CreateWorkspaceCommand,
    DatabaseContext, DeleteDocumentCommand, DeleteTaskCommand, DeleteWorkspaceCommand,
    EnsureJournalCommand, IdentifyLocalClientCommand, ListActorsCommand, ListArticlesCommand,
    SyncCapabilitiesCommand,
};

fn resolve(db: &DatabaseContext, link: &str) -> Result<Embed, CommandError> {
    db.send_blocking(ResolveLinkCommand {
        link: link.to_string(),
    })
}

fn workspace(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: "Work".to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn task(db: &DatabaseContext, ws: i64) -> Task {
    db.send_blocking(CreateTaskCommand {
        workspace_id: ws,
        title: "Fix login".to_string(),
        folder_id: None,
        priority: None,
        size: None,
        assignee: None,
        description: Some("The **form** loses its state.\n\nMore later.".to_string()),
        start_date: None,
        due_date: None,
        tag_ids: Vec::new(),
    })
    .unwrap()
}

#[test]
fn a_task_link_resolves_with_its_workspace_or_without() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    for link in [
        format!("cohesion://task/{ws}/{}", task.id),
        format!("cohesion://task/{}", task.id),
    ] {
        let embed = resolve(&db, &link).unwrap();
        assert_eq!(embed.link, format!("cohesion://task/{ws}/{}", task.id));
        assert_eq!(embed.kind, EmbedKind::Task);
        assert_eq!(embed.workspace_id, Some(ws));
        assert_eq!(embed.title, "Fix login");
        assert_eq!(embed.excerpt, "The form loses its state.");
        assert!(matches!(embed.item, EmbedItem::Task { task: t } if t.id == task.id));
    }
}

#[test]
fn a_task_in_another_workspace_deleted_or_never_made_is_not_found() {
    let db = context();
    let ws = workspace(&db);
    let other = workspace(&db);
    let task = task(&db, ws);
    let not_found = |link: String| {
        assert!(
            matches!(resolve(&db, &link), Err(CommandError::NotFound(_))),
            "{link}"
        )
    };
    not_found(format!("cohesion://task/{other}/{}", task.id));
    not_found(format!("cohesion://task/{ws}/{}", task.id + 1000));
    db.send_blocking(DeleteTaskCommand {
        workspace_id: ws,
        task_id: task.id,
    })
    .unwrap();
    not_found(format!("cohesion://task/{ws}/{}", task.id));
}

#[test]
fn a_document_in_a_deleted_workspace_is_not_found() {
    let db = context();
    let ws = workspace(&db);
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: ws,
            title: "Launch plan".to_string(),
            contents: "# Launch plan\n\nShip it.".to_string(),
            folder_id: None,
        })
        .unwrap();
    let link = format!("cohesion://document/{ws}/{}", document.id);
    let embed = resolve(&db, &link).unwrap();
    assert_eq!(embed.kind, EmbedKind::Document);
    assert_eq!(embed.excerpt, "Ship it.");
    db.send_blocking(DeleteWorkspaceCommand { id: ws }).unwrap();
    assert!(matches!(
        resolve(&db, &link),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn a_journal_entry_is_a_document_of_its_own_kind() {
    let db = context();
    let ws = workspace(&db);
    let entry = db
        .send_blocking(EnsureJournalCommand {
            workspace_id: ws,
            date: "2026-11-09".to_string(),
        })
        .unwrap();
    let embed = resolve(&db, &format!("cohesion://document/{ws}/{}", entry.id)).unwrap();
    assert_eq!(embed.kind, EmbedKind::JournalEntry);
}

#[test]
fn a_deleted_document_is_not_found() {
    let db = context();
    let ws = workspace(&db);
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: ws,
            title: "Gone".to_string(),
            contents: String::new(),
            folder_id: None,
        })
        .unwrap();
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: document.id,
    })
    .unwrap();
    assert!(matches!(
        resolve(&db, &format!("cohesion://document/{}", document.id)),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn an_agent_resolves_to_its_name_and_role() {
    let db = context();
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Teal,
            emoji: None,
            personality: None,
            verbosity: None,
            role: Some("Reads the news.".to_string()),
            directive: None,
            team_id: None,
        })
        .unwrap();
    let embed = resolve(&db, &format!("cohesion://agent/{}", agent.id)).unwrap();
    assert_eq!(embed.kind, EmbedKind::Agent);
    assert_eq!(embed.title, "Scout");
    assert_eq!(embed.excerpt, "Reads the news.");
    assert_eq!(embed.workspace_id, None);
}

#[test]
fn an_actor_session_shows_what_its_actor_last_said() {
    let db = context();
    let client_id = db
        .send_blocking(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "MacBook".to_string(),
                name: "MacBook".to_string(),
            },
        })
        .unwrap()
        .id;
    db.send_blocking(SyncCapabilitiesCommand {
        client_id,
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
    .unwrap();
    let actor = db.send_blocking(ListActorsCommand).unwrap().remove(0);
    let session = db
        .send_blocking(CreateActorSessionCommand {
            actor_id: actor.id,
            prompt: "Fix the build".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .unwrap()
        .0;
    let link = format!("cohesion://actor-session/{}", session.id);
    assert_eq!(resolve(&db, &link).unwrap().excerpt, "");
    for content in [
        ActorSessionMessageContent::Actor(ActorMessageContent::Text {
            text: "Found it: a *stale* cache.".to_string(),
        }),
        ActorSessionMessageContent::User(ActorUserContent::Text {
            text: "Thanks".to_string(),
        }),
    ] {
        db.send_blocking(AddActorSessionMessageCommand {
            session_id: session.id,
            content,
            attachments: Vec::new(),
        })
        .unwrap();
    }
    let embed = resolve(&db, &link).unwrap();
    assert_eq!(embed.kind, EmbedKind::ActorSession);
    assert_eq!(embed.excerpt, "Found it: a stale cache.");
    assert!(matches!(
        embed.item,
        EmbedItem::ActorSession { actor_title: Some(title), .. } if title == actor.title
    ));
}

#[test]
fn an_article_resolves_with_its_feed() {
    let db = context();
    db.send_blocking(CreateFeedCommand {
        url: "https://example.com/feed.xml".to_string(),
        title: "Example Weekly".to_string(),
        full_page_policy: FeedFullPagePolicy::WhenExcerpt,
        items: vec![FeedItemCapture {
            url: "https://example.com/post".to_string(),
            title: Some("A post".to_string()),
            author: None,
            published: None,
            fetched: Ok(cohesion_core::model::ArticleCapture {
                final_url: "https://example.com/post".to_string(),
                page_title: None,
                author: None,
                published: None,
                text: "What the post says.".to_string(),
                truncated: false,
            }),
        }],
    })
    .unwrap();
    let article = db
        .send_blocking(ListArticlesCommand {
            filter: cohesion_core::model::ArticleFilter::Feeds,
        })
        .unwrap()
        .remove(0);
    let embed = resolve(&db, &format!("cohesion://article/{}", article.id)).unwrap();
    assert_eq!(embed.kind, EmbedKind::Article);
    assert_eq!(embed.title, "A post");
    assert_eq!(embed.excerpt, "What the post says.");
    assert!(matches!(
        embed.item,
        EmbedItem::Article { feed_title, .. } if feed_title == "Example Weekly"
    ));
    assert!(matches!(
        resolve(&db, &format!("cohesion://article/{}", article.id + 1000)),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn a_missing_resource_agent_or_session_is_not_found() {
    let db = context();
    let ws = workspace(&db);
    for link in [
        format!("cohesion://resource/{ws}/1"),
        "cohesion://agent/999".to_string(),
        "cohesion://actor-session/999".to_string(),
    ] {
        assert!(
            matches!(resolve(&db, &link), Err(CommandError::NotFound(_))),
            "{link}"
        );
    }
}

#[test]
fn text_that_is_not_an_item_link_fails_on_link() {
    let db = context();
    for text in ["https://example.com", "cohesion://contact/3", ""] {
        match resolve(&db, text) {
            Err(CommandError::Validation(error)) => {
                assert_eq!(error.field.as_deref(), Some("link"))
            }
            _ => panic!("{text} resolved"),
        }
    }
}
