use cohesion_core::model::{
    Document, Embed, EmbedKind, PropertyDataType, Task, VersionChange, VersionTarget,
};

use super::*;
use crate::commands::document_folder_property::tests::Fixture;
use crate::test_support::context;
use crate::{
    AppendToJournalCommand, ChangeTaskDescriptionCommand, ChangeTaskTitleCommand, CommandError,
    CreateDocumentCommand, CreateTaskCommand, CreateTaskFolderCommand, CreateWorkspaceCommand,
    DatabaseContext, DeleteDocumentCommand, DeleteTaskCommand, DeleteTaskFolderCommand,
    DeleteWorkspaceCommand, EditDocumentCommand, ListVersionsCommand, MoveTaskToFolderCommand,
    RenameDocumentCommand, RestoreVersionCommand,
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
fn a_document_linking_to_a_contact_is_its_backlink() {
    let db = context();
    let ws = workspace(&db);
    let contact = db
        .send_blocking(crate::CreateContactCommand {
            workspace_id: ws,
            first_name: "Ada".to_string(),
            middle_name: None,
            last_name: "Lovelace".to_string(),
            notes: String::new(),
            methods: Vec::new(),
        })
        .unwrap();
    let link = format!("cohesion://contact/{ws}/{}", contact.id);
    document(&db, ws, "Notes", &format!("Call [Ada]({link}) back."));
    assert_eq!(titles(&db, &link), ["Notes"]);
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

fn describe(db: &DatabaseContext, task: &Task, description: &str) {
    db.send_blocking(ChangeTaskDescriptionCommand {
        workspace_id: task.workspace_id,
        task_id: task.id,
        description: description.to_string(),
    })
    .unwrap();
}

#[test]
fn a_tasks_description_links_as_a_documents_contents_do() {
    let db = context();
    let ws = workspace(&db);
    let plan = document(&db, ws, "Plan", "");
    let other = task(&db, ws);
    let plan_link = format!("cohesion://document/{ws}/{}", plan.id);
    let other_link = format!("cohesion://task/{ws}/{}", other.id);
    let fixing = db
        .send_blocking(CreateTaskCommand {
            workspace_id: ws,
            title: "Write the plan".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: Some(format!("[Plan]({plan_link})")),
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    assert_eq!(titles(&db, &plan_link), ["Write the plan"]);
    let linking = backlinks(&db, &plan_link);
    assert_eq!(
        linking[0].link,
        format!("cohesion://task/{ws}/{}", fixing.id)
    );

    let mut batches = db.subscribe();
    describe(&db, &fixing, &format!("Blocked on [this]({other_link})."));
    assert!(titles(&db, &plan_link).is_empty());
    assert_eq!(titles(&db, &other_link), ["Write the plan"]);
    let keys = published(&mut batches);
    assert!(keys.contains(&key(LinkKind::Document, plan.id)));
    assert!(keys.contains(&key(LinkKind::Task, other.id)));

    // A rename changes what the other task's backlinks show, so it says so too.
    db.send_blocking(ChangeTaskTitleCommand {
        workspace_id: ws,
        task_id: fixing.id,
        title: "Unblock".to_string(),
    })
    .unwrap();
    assert_eq!(titles(&db, &other_link), ["Unblock"]);
    assert!(published(&mut batches).contains(&key(LinkKind::Task, other.id)));

    // A task linking to itself is not its own backlink.
    let own = format!("cohesion://task/{ws}/{}", fixing.id);
    describe(&db, &fixing, &format!("[Me]({own})"));
    assert!(titles(&db, &own).is_empty());
}

#[test]
fn a_deleted_task_links_to_nothing_until_rewind_brings_it_back() {
    let db = context();
    let ws = workspace(&db);
    let plan = document(&db, ws, "Plan", "");
    let link = format!("cohesion://document/{ws}/{}", plan.id);
    let fixing = task(&db, ws);
    describe(&db, &fixing, &format!("[Plan]({link})"));
    let mut batches = db.subscribe();
    db.send_blocking(DeleteTaskCommand {
        workspace_id: ws,
        task_id: fixing.id,
    })
    .unwrap();
    assert!(titles(&db, &link).is_empty());
    assert!(published(&mut batches).contains(&key(LinkKind::Document, plan.id)));

    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: fixing.id },
        })
        .unwrap();
    assert_eq!(versions[0].change, VersionChange::Deleted);
    db.send_blocking(RestoreVersionCommand {
        workspace_id: ws,
        version_id: versions[0].id,
    })
    .unwrap();
    assert_eq!(titles(&db, &link), ["Fix login"]);
}

#[test]
fn deleting_a_task_folder_takes_its_tasks_links() {
    let db = context();
    let ws = workspace(&db);
    let plan = document(&db, ws, "Plan", "");
    let link = format!("cohesion://document/{ws}/{}", plan.id);
    let folder = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: ws,
            title: "Later".to_string(),
        })
        .unwrap();
    let fixing = task(&db, ws);
    describe(&db, &fixing, &format!("[Plan]({link})"));
    db.send_blocking(MoveTaskToFolderCommand {
        workspace_id: ws,
        task_id: fixing.id,
        folder_id: Some(folder.id),
    })
    .unwrap();
    assert_eq!(titles(&db, &link), ["Fix login"]);

    let mut batches = db.subscribe();
    db.send_blocking(DeleteTaskFolderCommand {
        workspace_id: ws,
        folder_id: folder.id,
    })
    .unwrap();
    assert!(titles(&db, &link).is_empty());
    assert!(published(&mut batches).contains(&key(LinkKind::Document, plan.id)));
}

#[test]
fn a_table_has_backlinks_and_its_rows_link_as_documents() {
    let f = Fixture::new();
    let (db, ws) = (&f.db, f.ws);
    f.property("Author", PropertyDataType::Text);
    let folder_link = format!("cohesion://document-folder/{ws}/{}", f.folder.id);
    document(db, ws, "Plan", &format!("[Books]({folder_link})"));
    assert_eq!(titles(db, &folder_link), ["Plan"]);

    // A row is a document: what it links to lists it as one.
    let task = task(db, ws);
    let task_link = format!("cohesion://task/{ws}/{}", task.id);
    let row = document(db, ws, "Dune", &format!("Before [Fix login]({task_link})"));
    db.send_blocking(crate::MoveDocumentCommand {
        workspace_id: ws,
        document_id: row.id,
        folder_id: Some(f.folder.id),
    })
    .unwrap();
    let linking = backlinks(db, &task_link);
    assert_eq!(linking.len(), 1);
    assert_eq!(linking[0].kind, EmbedKind::Document);
    assert_eq!(linking[0].title, "Dune");
}
