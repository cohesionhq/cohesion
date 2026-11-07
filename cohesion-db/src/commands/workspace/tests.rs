use crate::test_support::context;
use crate::{
    CommandError, CreateDocumentCommand, CreateTaskCommand, CreateWorkspaceCommand,
    GetWorkspaceIncludingDeletedCommand, RenameWorkspaceCommand, SearchWorkspaceCommand,
    SetWorkspaceEmojiCommand,
};

#[test]
fn search_ranks_title_matches_first_and_stays_in_its_workspace() {
    let db = context();
    let [home, work] = ["home", "work"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
    });
    let document = |workspace_id, title: &str, contents: &str| {
        db.send_blocking(CreateDocumentCommand {
            workspace_id,
            title: title.to_string(),
            contents: contents.to_string(),
        })
        .unwrap()
    };
    let mentions = document(home.id, "Notes", "We talked about the garden fence today.");
    let titled = document(home.id, "Garden", "Tomatoes and beans.");
    document(work.id, "Garden budget", "");
    let task = db
        .send_blocking(CreateTaskCommand {
            workspace_id: home.id,
            title: "Water the garden".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();

    let hits = db
        .send_blocking(SearchWorkspaceCommand {
            workspace_id: home.id,
            query: "garden".to_string(),
        })
        .unwrap();
    let ids: Vec<_> = hits.iter().map(|hit| hit.entity_id).collect();
    assert_eq!(hits.len(), 3);
    assert_eq!(ids.last(), Some(&mentions.id));
    assert!(ids.contains(&titled.id) && ids.contains(&task.id));
}

#[test]
fn emoji_is_trimmed_and_blank_clears_it() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: Some(" 🌱 ".to_string()),
        })
        .unwrap();
    assert_eq!(workspace.emoji.as_deref(), Some("🌱"));
    let set = |emoji: &str| {
        db.send_blocking(SetWorkspaceEmojiCommand {
            id: workspace.id,
            emoji: Some(emoji.to_string()),
        })
    };
    assert!(set("not an emoji").is_err());
    assert_eq!(set(" ").unwrap().emoji, None);
}

#[test]
fn renaming_to_an_empty_title_is_rejected_and_keeps_the_old_one() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    assert!(matches!(
        db.send_blocking(RenameWorkspaceCommand {
            id: workspace.id,
            title: String::new(),
        }),
        Err(CommandError::Validation(_))
    ));
    let unchanged = db
        .send_blocking(GetWorkspaceIncludingDeletedCommand { id: workspace.id })
        .unwrap();
    assert_eq!(unchanged.title, "home");
}

#[test]
fn a_malformed_search_query_is_a_validation_error() {
    let db = context();
    let home = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    for query in ["re-auth", "\"open"] {
        let result = db.send_blocking(SearchWorkspaceCommand {
            workspace_id: home.id,
            query: query.to_string(),
        });
        assert!(
            matches!(result, Err(crate::CommandError::Validation(_))),
            "{query}"
        );
    }
}

#[test]
fn an_unrelated_database_failure_during_a_search_stays_internal() {
    let failure = |detail: &str| {
        crate::database_error::DatabaseError::Sql(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(detail.to_string()),
        ))
    };
    let syntax = crate::searched::<()>(Err(failure("fts5: syntax error near \".\"")));
    assert!(matches!(syntax, Err(crate::CommandError::Validation(_))));
    for detail in ["no such table: search_entries", "database is locked"] {
        let result = crate::searched::<()>(Err(failure(detail)));
        assert!(
            matches!(result, Err(crate::CommandError::Database(_))),
            "{detail}"
        );
    }
}
