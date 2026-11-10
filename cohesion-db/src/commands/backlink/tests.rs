use cohesion_core::model::{Document, Embed, Task, VersionChange, VersionTarget};

use super::*;
use crate::test_support::context;
use crate::{
    AppendToJournalCommand, CommandError, CreateDocumentCommand, CreateTaskCommand,
    CreateWorkspaceCommand, DatabaseContext, DeleteDocumentCommand, DeleteWorkspaceCommand,
    EditDocumentCommand, ListVersionsCommand, RenameDocumentCommand, RestoreVersionCommand,
};

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
        description: None,
        start_date: None,
        due_date: None,
        tag_ids: Vec::new(),
    })
    .unwrap()
}

fn document(db: &DatabaseContext, ws: i64, title: &str, contents: &str) -> Document {
    db.send_blocking(CreateDocumentCommand {
        workspace_id: ws,
        title: title.to_string(),
        contents: contents.to_string(),
        folder_id: None,
    })
    .unwrap()
}

fn edit(db: &DatabaseContext, document: &Document, contents: &str) {
    db.send_blocking(EditDocumentCommand {
        workspace_id: document.workspace_id,
        document_id: document.id,
        contents: contents.to_string(),
    })
    .unwrap();
}

fn backlinks(db: &DatabaseContext, link: &str) -> Vec<Embed> {
    db.send_blocking(BacklinksOfCommand {
        link: link.to_string(),
    })
    .unwrap()
}

fn titles(db: &DatabaseContext, link: &str) -> Vec<String> {
    backlinks(db, link)
        .into_iter()
        .map(|embed| embed.title)
        .collect()
}

/// The keys every batch published since `batches` subscribed carried.
fn published(
    batches: &mut tokio::sync::broadcast::Receiver<
        std::sync::Arc<cohesion_core::invalidation::InvalidationBatch>,
    >,
) -> HashSet<Invalidation> {
    let mut keys = HashSet::new();
    while let Ok(batch) = batches.try_recv() {
        keys.extend(batch.keys.iter().cloned());
    }
    keys
}

fn key(kind: LinkKind, id: i64) -> Invalidation {
    Invalidation::Backlinks(LinkTarget { kind, id })
}

#[test]
fn a_documents_links_are_its_targets_backlinks_whichever_way_they_are_written() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    let plan = document(&db, ws, "Plan", "Nothing yet.");
    let notes = document(
        &db,
        ws,
        "Notes",
        &format!(
            "[Fix login](cohesion://task/{ws}/{task})\n\nSee [the plan](cohesion://document/{plan}) and \
             [again](cohesion://task/{task}).",
            task = task.id,
            plan = plan.id
        ),
    );

    let linking = backlinks(&db, &format!("cohesion://task/{}", task.id));
    assert_eq!(linking.len(), 1);
    assert_eq!(linking[0].title, "Notes");
    assert_eq!(
        linking[0].link,
        format!("cohesion://document/{ws}/{}", notes.id)
    );
    assert_eq!(
        titles(&db, &format!("cohesion://document/{ws}/{}", plan.id)),
        ["Notes"]
    );
    assert!(titles(&db, &format!("cohesion://document/{ws}/{}", notes.id)).is_empty());
}

#[test]
fn editing_a_document_adds_and_removes_its_backlinks() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    let plan = document(&db, ws, "Plan", "");
    let notes = document(&db, ws, "Notes", "");
    let task_link = format!("cohesion://task/{ws}/{}", task.id);
    let plan_link = format!("cohesion://document/{ws}/{}", plan.id);
    assert!(titles(&db, &task_link).is_empty());

    let mut batches = db.subscribe();
    edit(&db, &notes, &format!("[Fix login]({task_link})"));
    assert_eq!(titles(&db, &task_link), ["Notes"]);
    assert!(published(&mut batches).contains(&key(LinkKind::Task, task.id)));

    edit(&db, &notes, &format!("Only [the plan]({plan_link}) now."));
    assert!(titles(&db, &task_link).is_empty());
    assert_eq!(titles(&db, &plan_link), ["Notes"]);
    let keys = published(&mut batches);
    assert!(keys.contains(&key(LinkKind::Task, task.id)));
    assert!(keys.contains(&key(LinkKind::Document, plan.id)));

    // A rename changes what the plan's backlinks show, so it says so too.
    db.send_blocking(RenameDocumentCommand {
        workspace_id: ws,
        document_id: notes.id,
        title: "Meeting notes".to_string(),
    })
    .unwrap();
    assert_eq!(titles(&db, &plan_link), ["Meeting notes"]);
    assert!(published(&mut batches).contains(&key(LinkKind::Document, plan.id)));
}

#[test]
fn a_document_linking_to_itself_is_not_its_own_backlink() {
    let db = context();
    let ws = workspace(&db);
    let notes = document(&db, ws, "Notes", "");
    let link = format!("cohesion://document/{ws}/{}", notes.id);
    edit(&db, &notes, &format!("[Here]({link})"));
    assert!(titles(&db, &link).is_empty());
}

#[test]
fn backlinks_are_listed_by_title_and_a_journal_entry_links_too() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    let link = format!("cohesion://task/{ws}/{}", task.id);
    document(&db, ws, "zebra", &format!("[x]({link})"));
    document(&db, ws, "Apple", &format!("[x]({link})"));
    db.send_blocking(AppendToJournalCommand {
        workspace_id: ws,
        date: "2026-11-09".to_string(),
        content: format!("Worked on [Fix login]({link})"),
    })
    .unwrap();
    assert_eq!(titles(&db, &link), ["2026-11-09", "Apple", "zebra"]);
}

#[test]
fn a_deleted_document_or_workspace_links_to_nothing() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    let link = format!("cohesion://task/{ws}/{}", task.id);
    let notes = document(&db, ws, "Notes", &format!("[Fix login]({link})"));
    let mut batches = db.subscribe();
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: notes.id,
    })
    .unwrap();
    assert!(titles(&db, &link).is_empty());
    assert!(published(&mut batches).contains(&key(LinkKind::Task, task.id)));

    let other = workspace(&db);
    document(&db, other, "Elsewhere", &format!("[Fix login]({link})"));
    assert_eq!(titles(&db, &link), ["Elsewhere"]);
    db.send_blocking(DeleteWorkspaceCommand { id: other })
        .unwrap();
    assert!(titles(&db, &link).is_empty());
}

#[test]
fn an_item_never_made_has_no_backlinks_and_text_that_isnt_a_link_fails() {
    let db = context();
    assert!(backlinks(&db, "cohesion://task/1/999").is_empty());
    let error = db
        .send_blocking(BacklinksOfCommand {
            link: "https://example.com".to_string(),
        })
        .err()
        .unwrap();
    assert!(matches!(error, CommandError::Validation(ref v) if v.field.as_deref() == Some("link")));
}

#[test]
fn a_document_brought_back_by_rewind_links_again() {
    let db = context();
    let ws = workspace(&db);
    let task = task(&db, ws);
    let link = format!("cohesion://task/{ws}/{}", task.id);
    let notes = document(&db, ws, "Notes", &format!("[Fix login]({link})"));
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: notes.id,
    })
    .unwrap();
    assert!(titles(&db, &link).is_empty());

    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: notes.id,
            },
        })
        .unwrap();
    assert_eq!(versions[0].change, VersionChange::Deleted);
    db.send_blocking(RestoreVersionCommand {
        workspace_id: ws,
        version_id: versions[0].id,
    })
    .unwrap();
    assert_eq!(titles(&db, &link), ["Notes"]);
}
