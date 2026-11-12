use crate::test_support::context;
use cohesion_core::model::SearchEntityKind;

use crate::{
    ChangeTaskStatusCommand, CommandError, CreateDocumentCommand, CreateDocumentFolderCommand,
    CreateTaskCommand, CreateTaskFolderCommand, CreateWorkspaceCommand,
    DeleteDocumentFolderCommand, DeleteTaskFolderCommand, DeleteWorkspaceCommand,
    GetWorkspaceIncludingDeletedCommand, ListWorkspacesCommand, RenameDocumentFolderCommand,
    RenameWorkspaceCommand, ReorderWorkspacesCommand, SearchWorkspaceCommand,
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
            folder_id: None,
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
    let status_of = |hits: &[cohesion_core::model::SearchResult], id| {
        hits.iter()
            .find(|hit| hit.entity_id == id)
            .unwrap()
            .status
            .clone()
    };
    assert_eq!(status_of(&hits, task.id), "incomplete");
    assert_eq!(status_of(&hits, titled.id), "none");

    db.send_blocking(ChangeTaskStatusCommand {
        workspace_id: home.id,
        task_id: task.id,
        status: cohesion_core::TaskStatus::Completed,
    })
    .unwrap();
    let hits = db
        .send_blocking(SearchWorkspaceCommand {
            workspace_id: home.id,
            query: "garden".to_string(),
        })
        .unwrap();
    assert_eq!(status_of(&hits, task.id), "completed");
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
fn a_deleted_workspace_is_not_searched() {
    let db = context();
    let home = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    db.send_blocking(CreateTaskCommand {
        workspace_id: home.id,
        title: "milk".to_string(),
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
    let search = || {
        db.send_blocking(SearchWorkspaceCommand {
            workspace_id: home.id,
            query: "milk".to_string(),
        })
    };
    assert_eq!(search().unwrap().len(), 1);
    db.send_blocking(DeleteWorkspaceCommand { id: home.id })
        .unwrap();
    assert!(matches!(search(), Err(CommandError::NotFound(_))));
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

#[test]
fn search_finds_live_folders_by_title() {
    let db = context();
    let ws = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let tasks = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: ws,
            title: "Garden chores".to_string(),
        })
        .unwrap();
    let docs = db
        .send_blocking(CreateDocumentFolderCommand {
            workspace_id: ws,
            title: "Garden plans".to_string(),
            parent_id: None,
        })
        .unwrap();
    let search = |query: &str| -> Vec<(bool, i64)> {
        let mut hits: Vec<_> = db
            .send_blocking(SearchWorkspaceCommand {
                workspace_id: ws,
                query: query.to_string(),
            })
            .unwrap()
            .into_iter()
            .map(|hit| {
                let task_folder = matches!(hit.entity_kind, SearchEntityKind::TaskFolder);
                assert!(task_folder || matches!(hit.entity_kind, SearchEntityKind::DocumentFolder));
                (task_folder, hit.entity_id)
            })
            .collect();
        hits.sort();
        hits
    };
    assert_eq!(search("garden"), [(false, docs.id), (true, tasks.id)]);

    db.send_blocking(RenameDocumentFolderCommand {
        workspace_id: ws,
        folder_id: docs.id,
        title: "Allotment".to_string(),
    })
    .unwrap();
    assert_eq!(search("garden"), [(true, tasks.id)]);
    assert_eq!(search("allotment"), [(false, docs.id)]);

    db.send_blocking(DeleteTaskFolderCommand {
        workspace_id: ws,
        folder_id: tasks.id,
    })
    .unwrap();
    db.send_blocking(DeleteDocumentFolderCommand {
        workspace_id: ws,
        folder_id: docs.id,
    })
    .unwrap();
    assert!(search("garden").is_empty());
    assert!(search("allotment").is_empty());
}

#[test]
fn search_ranks_an_exact_title_first_then_titles_then_contents() {
    let db = context();
    let ws = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let task = |title: &str, description: &str| {
        db.send_blocking(CreateTaskCommand {
            workspace_id: ws,
            title: title.to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: Some(description.to_string()),
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap()
        .id
    };
    // A short description repeating the word scored above the folder named
    // exactly that when contents counted alongside the title.
    let titled = task("Pick default models for tiers", "models models");
    let mentioned = task("Refresh the catalogue", "The models list. models.");
    // Unrelated tasks with long descriptions, so "models" is rare, as in a
    // real workspace.
    let filler = "Sweep the porch and water the garden before the rain comes. ".repeat(15);
    for n in 0..200 {
        task(&format!("Chore {n}"), &filler);
    }
    let folder = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: ws,
            title: "Models".to_string(),
        })
        .unwrap()
        .id;

    // As the palette asks: each word prefix-matched.
    let hits: Vec<_> = db
        .send_blocking(SearchWorkspaceCommand {
            workspace_id: ws,
            query: "\"Models\"*".to_string(),
        })
        .unwrap()
        .into_iter()
        .map(|hit| {
            let task_folder = matches!(hit.entity_kind, SearchEntityKind::TaskFolder);
            (task_folder, hit.entity_id)
        })
        .collect();
    assert_eq!(hits, [(true, folder), (false, titled), (false, mentioned)]);
}

fn workspace_ids(db: &crate::DatabaseContext) -> Vec<i64> {
    db.send_blocking(ListWorkspacesCommand::default())
        .unwrap()
        .into_iter()
        .map(|workspace| workspace.id)
        .collect()
}

#[test]
fn workspaces_keep_the_order_they_are_given_and_new_ones_go_last() {
    let db = context();
    let [a, b, c] = ["a", "b", "c"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
        .id
    });
    assert_eq!(workspace_ids(&db), [a, b, c]);

    let reordered = db
        .send_blocking(ReorderWorkspacesCommand { ids: vec![c, a, b] })
        .unwrap();
    assert_eq!(
        reordered.iter().map(|w| w.id).collect::<Vec<_>>(),
        [c, a, b]
    );
    assert_eq!(workspace_ids(&db), [c, a, b]);

    let d = db
        .send_blocking(CreateWorkspaceCommand {
            title: "d".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    assert_eq!(workspace_ids(&db), [c, a, b, d]);
}

#[test]
fn reordering_with_a_deleted_workspace_changes_nothing() {
    let db = context();
    let [a, b, gone] = ["a", "b", "gone"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
        .id
    });
    db.send_blocking(DeleteWorkspaceCommand { id: gone })
        .unwrap();

    let refused = db.send_blocking(ReorderWorkspacesCommand {
        ids: vec![b, gone, a],
    });
    assert!(matches!(refused, Err(CommandError::NotFound(_))));
    assert_eq!(workspace_ids(&db), [a, b]);
}
