use std::sync::Arc;

use cohesion_core::model::{
    DeletedItem, IntelligenceAgentColor, RestoredItem, VersionAuthor, VersionChange, VersionTarget,
    VersionedItem,
};
use cohesion_core::{Document, Task, TaskStatus};

use crate::changed::{Changed, Invalidates};
use crate::test_support::{RawSql, context, recording_context};
use crate::{
    ChangeTaskDescriptionCommand, ChangeTaskStatusCommand, ChangeTaskTitleCommand, CommandContext,
    CreateDocumentCommand, CreateIntelligenceAgentCommand, CreateTaskCommand,
    CreateTaskFolderCommand, CreateWorkspaceCommand, DatabaseCommand, DatabaseCommandResult,
    DatabaseContext, DatabaseEventKind, DatabaseEventTrace, DeleteDocumentCommand,
    DeleteIntelligenceAgentCommand, DeleteTaskFolderCommand, EditDocumentCommand,
    EditJournalCommand, EnsureJournalCommand, GetStorageSettingsCommand, GetTaskCommand,
    GetVersionCommand, ListDeletedItemsCommand, ListVersionsCommand, PeriodicCleanupCommand,
    RestoreVersionCommand, Write,
};

/// Puts every kept version's window behind it, so the next edit starts a new
/// one.
/// [`RestoreVersionCommand`], answering with the document it restored.
struct RestoreDocumentVersionCommand {
    workspace_id: i64,
    version_id: i64,
}
impl DatabaseCommand for RestoreDocumentVersionCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        let restored = ctx.run(RestoreVersionCommand {
            workspace_id: self.workspace_id,
            version_id: self.version_id,
        })?;
        match restored {
            RestoredItem::Document { document } => Ok(document.invalidating_nothing()),
            RestoredItem::Task { .. } => panic!("a task came back"),
        }
    }
}

/// [`RestoreVersionCommand`], answering with the task it restored.
struct RestoreTaskVersionCommand {
    workspace_id: i64,
    version_id: i64,
}
impl DatabaseCommand for RestoreTaskVersionCommand {
    type Response = Task;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Task>> {
        let restored = ctx.run(RestoreVersionCommand {
            workspace_id: self.workspace_id,
            version_id: self.version_id,
        })?;
        match restored {
            RestoredItem::Task { task } => Ok(task.invalidating_nothing()),
            RestoredItem::Document { .. } => panic!("a document came back"),
        }
    }
}

/// The ids of the deleted tasks Rewind lists in a workspace.
fn deleted_tasks(deleted: Vec<DeletedItem>) -> Vec<i64> {
    deleted
        .into_iter()
        .filter_map(|deleted| match deleted.item {
            VersionedItem::Task { task_id, .. } => Some(task_id),
            VersionedItem::Document { .. } => None,
        })
        .collect()
}

/// The ids of the deleted documents Rewind lists in a workspace.
fn deleted_documents(deleted: Vec<DeletedItem>) -> Vec<i64> {
    deleted
        .into_iter()
        .filter_map(|deleted| match deleted.item {
            VersionedItem::Document { document_id } => Some(document_id),
            VersionedItem::Task { .. } => None,
        })
        .collect()
}

fn later(db: &DatabaseContext) {
    db.send_blocking(RawSql("UPDATE versions SET created = created - 600"))
        .unwrap();
    db.send_blocking(RawSql("UPDATE versions SET created = created - 600"))
        .unwrap();
}

fn agent(db: &DatabaseContext, name: &str) -> i64 {
    db.send_blocking(CreateIntelligenceAgentCommand {
        name: Some(name.to_string()),
        color: IntelligenceAgentColor::Teal,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id: None,
    })
    .unwrap()
    .id
}

fn workspace(db: &DatabaseContext, title: &str) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: title.to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn task(db: &DatabaseContext, workspace_id: i64, folder_id: Option<i64>) -> Task {
    db.send_blocking(CreateTaskCommand {
        workspace_id,
        title: "first".to_string(),
        folder_id,
        priority: None,
        size: None,
        assignee: None,
        description: Some("first text".to_string()),
        start_date: None,
        due_date: None,
        tag_ids: Vec::new(),
    })
    .unwrap()
}

fn document(db: &DatabaseContext, workspace_id: i64) -> Document {
    db.send_blocking(CreateDocumentCommand {
        workspace_id,
        title: "Plan".to_string(),
        contents: "first".to_string(),
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

/// A document's versions, newest first, as what each records and holds.
fn history(db: &DatabaseContext, document: &Document) -> Vec<(VersionChange, String)> {
    db.send_blocking(ListVersionsCommand {
        workspace_id: document.workspace_id,
        item: VersionTarget::Document {
            document_id: document.id,
        },
    })
    .unwrap()
    .into_iter()
    .map(|summary| {
        let version = db
            .send_blocking(GetVersionCommand {
                workspace_id: document.workspace_id,
                version_id: summary.id,
            })
            .unwrap();
        (summary.change, version.body)
    })
    .collect()
}

#[test]
fn edits_within_two_minutes_share_a_version_whoever_makes_them() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    let agent = agent(&db, "Ada");

    edit(&db, &doc, "typed");
    edit(&db, &doc, "typed more");
    assert_eq!(
        history(&db, &doc),
        [(VersionChange::Created, "typed more".to_string())],
    );

    later(&db);
    edit(&db, &doc, "second");
    db.send_blocking_traced(
        EditDocumentCommand {
            workspace_id: ws,
            document_id: doc.id,
            contents: "second, by Ada".to_string(),
        },
        DatabaseEventTrace::default().from_agent(agent),
    )
    .unwrap();

    assert_eq!(
        history(&db, &doc),
        [
            (VersionChange::Edited, "second, by Ada".to_string()),
            (VersionChange::Created, "typed more".to_string()),
        ],
    );
    let latest = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id,
            },
        })
        .unwrap();
    assert_eq!(latest[0].author, VersionAuthor::Agent);
    assert_eq!(latest[0].author_name.as_deref(), Some("Ada"));
    assert!(latest[0].current);
    assert!(!latest[1].current);
}

#[test]
fn a_save_that_changes_nothing_keeps_no_version() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    later(&db);
    edit(&db, &doc, "first");
    assert_eq!(history(&db, &doc).len(), 1);
}

#[test]
fn a_restore_within_the_window_can_be_undone() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    later(&db);
    edit(&db, &doc, "good text");

    let first = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id,
            },
        })
        .unwrap()[1]
        .id;
    let restored = db
        .send_blocking(RestoreDocumentVersionCommand {
            workspace_id: ws,
            version_id: first,
        })
        .unwrap();
    assert_eq!(restored.contents, "first");
    assert_eq!(
        history(&db, &doc),
        [
            (VersionChange::Restored, "first".to_string()),
            (VersionChange::Edited, "good text".to_string()),
            (VersionChange::Created, "first".to_string()),
        ],
    );

    // Undoing the restore is restoring what it replaced.
    let replaced = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id,
            },
        })
        .unwrap()[1]
        .id;
    let undone = db
        .send_blocking(RestoreDocumentVersionCommand {
            workspace_id: ws,
            version_id: replaced,
        })
        .unwrap();
    assert_eq!(undone.contents, "good text");
    assert_eq!(history(&db, &doc).len(), 4);
}

#[test]
fn a_task_restores_its_text_and_status_only_raising_what_changed() {
    let (db, events) = recording_context();
    let ws = workspace(&db, "w");
    let task = task(&db, ws, None);
    later(&db);
    db.send_blocking(ChangeTaskTitleCommand {
        workspace_id: ws,
        task_id: task.id,
        title: "second".to_string(),
    })
    .unwrap();
    db.send_blocking(ChangeTaskStatusCommand {
        workspace_id: ws,
        task_id: task.id,
        status: TaskStatus::Completed,
    })
    .unwrap();
    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: task.id },
        })
        .unwrap();
    assert_eq!(versions.len(), 2);
    events.take();

    let restored = db
        .send_blocking(RestoreTaskVersionCommand {
            workspace_id: ws,
            version_id: versions[1].id,
        })
        .unwrap();
    assert_eq!(restored.title, "first");
    assert_eq!(restored.status, TaskStatus::Incomplete);
    let raised = events.take();
    assert_eq!(raised.len(), 2);
    assert!(matches!(raised[0], DatabaseEventKind::TaskRenamed { .. }));
    assert!(matches!(
        raised[1],
        DatabaseEventKind::TaskStatusChanged { .. }
    ));
}

#[test]
fn a_deleted_task_comes_back_to_the_inbox_when_its_folder_is_gone() {
    let (db, events) = recording_context();
    let ws = workspace(&db, "w");
    let folder = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: ws,
            title: "Folder".to_string(),
        })
        .unwrap();
    let task = task(&db, ws, Some(folder.id));
    db.send_blocking(DeleteTaskFolderCommand {
        workspace_id: ws,
        folder_id: folder.id,
    })
    .unwrap();

    let deleted = deleted_tasks(
        db.send_blocking(ListDeletedItemsCommand { workspace_id: ws })
            .unwrap(),
    );
    assert_eq!(deleted, [task.id]);

    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: task.id },
        })
        .unwrap();
    assert_eq!(versions[0].change, VersionChange::Deleted);
    assert!(versions.iter().all(|version| !version.current));

    events.take();
    let restored = db
        .send_blocking(RestoreTaskVersionCommand {
            workspace_id: ws,
            version_id: versions[0].id,
        })
        .unwrap();
    assert!(!restored.deleted);
    assert_eq!(restored.task_folder_id, None);
    assert!(matches!(
        events.take()[0],
        DatabaseEventKind::TaskCreated { .. }
    ));
    assert!(
        deleted_tasks(
            db.send_blocking(ListDeletedItemsCommand { workspace_id: ws })
                .unwrap()
        )
        .is_empty()
    );
    db.send_blocking(GetTaskCommand {
        workspace_id: ws,
        task_id: task.id,
    })
    .unwrap();
}

#[test]
fn a_deletion_never_joins_the_edit_before_it() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: doc.id,
    })
    .unwrap();
    assert_eq!(
        history(&db, &doc),
        [
            (VersionChange::Deleted, "first".to_string()),
            (VersionChange::Created, "first".to_string()),
        ],
    );
    assert_eq!(
        deleted_documents(
            db.send_blocking(ListDeletedItemsCommand { workspace_id: ws })
                .unwrap()
        )[0],
        doc.id,
    );
}

#[test]
fn a_deleted_journal_stays_deleted_while_its_day_has_a_new_entry() {
    let db = context();
    let ws = workspace(&db, "w");
    let date = "2026-11-03".to_string();
    let old = db
        .send_blocking(EnsureJournalCommand {
            workspace_id: ws,
            date: date.clone(),
        })
        .unwrap();
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: old.id,
    })
    .unwrap();
    db.send_blocking(EditJournalCommand {
        workspace_id: ws,
        date,
        content: "a new entry".to_string(),
    })
    .unwrap();
    let version = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: old.id,
            },
        })
        .unwrap()[0]
        .id;
    let refused = db.send_blocking(RestoreDocumentVersionCommand {
        workspace_id: ws,
        version_id: version,
    });
    assert!(matches!(refused, Err(crate::CommandError::Validation(_))));
    // The refusal rolled its restoring mark back: the next edit is an edit.
    later(&db);
    let doc = document(&db, ws);
    later(&db);
    edit(&db, &doc, "after");
    assert_eq!(history(&db, &doc)[0].0, VersionChange::Edited);
}

#[test]
fn the_first_change_to_a_row_without_versions_keeps_what_it_held() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    db.send_blocking(RawSql("DELETE FROM versions")).unwrap();
    edit(&db, &doc, "changed");
    assert_eq!(
        history(&db, &doc),
        [
            (VersionChange::Edited, "changed".to_string()),
            (VersionChange::Earlier, "first".to_string()),
        ],
    );
}

#[test]
fn the_cleanup_removes_versions_past_the_retention() {
    let db = context();
    let ws = workspace(&db, "w");
    let task = task(&db, ws, None);
    assert_eq!(
        db.send_blocking(GetStorageSettingsCommand {})
            .unwrap()
            .rewind_retention_days,
        7,
    );
    db.send_blocking(RawSql(
        "UPDATE versions SET created = created - 8 * 86400, modified = modified - 8 * 86400",
    ))
    .unwrap();
    db.send_blocking(ChangeTaskDescriptionCommand {
        workspace_id: ws,
        task_id: task.id,
        description: "recent".to_string(),
    })
    .unwrap();
    db.send_blocking(PeriodicCleanupCommand).unwrap();
    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: task.id },
        })
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].change, VersionChange::Edited);
}

#[test]
fn a_version_says_who_wrote_it_even_once_they_are_gone() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    let agent = agent(&db, "Ada");

    let by = |trace: DatabaseEventTrace, contents: &str| {
        later(&db);
        db.send_blocking_traced(
            EditDocumentCommand {
                workspace_id: ws,
                document_id: doc.id,
                contents: contents.to_string(),
            },
            trace,
        )
        .unwrap();
    };
    by(DatabaseEventTrace::default().from_agent(agent), "agent");
    by(
        DatabaseEventTrace::default().from_extension(Arc::new("no.such.extension".to_string())),
        "extension",
    );
    by(DatabaseEventTrace::default().from_script(), "script");
    by(DatabaseEventTrace::default(), "user");
    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: agent })
        .unwrap();

    let authors: Vec<_> = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id,
            },
        })
        .unwrap()
        .into_iter()
        .map(|version| (version.author, version.author_name))
        .collect();
    assert_eq!(
        authors,
        [
            (VersionAuthor::User, None),
            (VersionAuthor::Script, None),
            (
                VersionAuthor::Extension,
                Some("no.such.extension".to_string())
            ),
            (VersionAuthor::Agent, Some("Ada".to_string())),
            (VersionAuthor::User, None),
        ],
    );
}

#[test]
fn a_rolled_back_command_does_not_leave_its_author_behind() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    let agent = agent(&db, "Ada");
    let refused = db.send_blocking_traced(
        EditDocumentCommand {
            workspace_id: ws,
            document_id: 999,
            contents: String::new(),
        },
        DatabaseEventTrace::default().from_agent(agent),
    );
    assert!(refused.is_err());
    later(&db);
    edit(&db, &doc, "by the user");
    assert_eq!(
        db.send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id
            }
        })
        .unwrap()[0]
            .author,
        VersionAuthor::User,
    );
}

#[test]
fn a_version_is_out_of_reach_from_another_workspace() {
    let db = context();
    let (ws, other) = (workspace(&db, "w"), workspace(&db, "o"));
    let task = task(&db, ws, None);
    let version = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: task.id },
        })
        .unwrap()[0]
        .id;
    assert!(
        db.send_blocking(ListVersionsCommand {
            workspace_id: other,
            item: VersionTarget::Task { task_id: task.id }
        })
        .unwrap()
        .is_empty()
    );
    assert!(
        db.send_blocking(GetVersionCommand {
            workspace_id: other,
            version_id: version,
        })
        .is_err()
    );
    assert!(
        db.send_blocking(RestoreTaskVersionCommand {
            workspace_id: other,
            version_id: version,
        })
        .is_err()
    );
}

// What restoring promises under rapid edits and repeated restores. The first
// tests state what must hold whatever spacing rule wins: a restore loses
// neither what the item held nor what it was put back to. The last ones pin
// where the two-minute rule and a restore meet, and what a window can lose.

/// The ids of a document's versions, newest first.
fn ids(db: &DatabaseContext, document: &Document) -> Vec<i64> {
    db.send_blocking(ListVersionsCommand {
        workspace_id: document.workspace_id,
        item: VersionTarget::Document {
            document_id: document.id,
        },
    })
    .unwrap()
    .into_iter()
    .map(|version| version.id)
    .collect()
}

fn restore(db: &DatabaseContext, document: &Document, version_id: i64) -> Document {
    db.send_blocking(RestoreDocumentVersionCommand {
        workspace_id: document.workspace_id,
        version_id,
    })
    .unwrap()
}

/// Whether some kept version of `document` still holds `text`.
fn recoverable(db: &DatabaseContext, document: &Document, text: &str) -> bool {
    history(db, document).iter().any(|(_, kept)| kept == text)
}

/// A document holding "second", with "first" kept in a version before it, both
/// past their windows; returns their ids, oldest first.
fn two_versions(db: &DatabaseContext, ws: i64) -> (Document, i64, i64) {
    let doc = document(db, ws);
    later(db);
    edit(db, &doc, "second");
    later(db);
    let versions = ids(db, &doc);
    (doc, versions[1], versions[0])
}

#[test]
fn what_a_restore_replaces_stays_recoverable_through_rapid_edits() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, second) = two_versions(&db, ws);

    restore(&db, &doc, first);
    edit(&db, &doc, "typed right after");
    edit(&db, &doc, "typed again");

    // The version it replaced never changes, however fast the edits come.
    let back = restore(&db, &doc, second);
    assert_eq!(back.contents, "second");
    // Nor does the version it restored from.
    assert_eq!(restore(&db, &doc, first).contents, "first");
    // And the edits made since are still there to come back to.
    assert!(recoverable(&db, &doc, "typed again"));
}

#[test]
fn repeated_restores_each_leave_what_they_replaced() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, second) = two_versions(&db, ws);

    for _ in 0..3 {
        assert_eq!(restore(&db, &doc, first).contents, "first");
        assert_eq!(restore(&db, &doc, second).contents, "second");
    }
    let kept = history(&db, &doc);
    assert_eq!(kept.len(), 8);
    assert!(
        kept[..6]
            .iter()
            .all(|(change, _)| *change == VersionChange::Restored)
    );
    // The first time and the last, each is what its restore left, in order.
    assert_eq!(kept[0].1, "second");
    assert_eq!(kept[1].1, "first");
}

#[test]
fn restoring_what_the_item_already_holds_keeps_no_version() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, _) = two_versions(&db, ws);
    restore(&db, &doc, first);
    let before = history(&db, &doc).len();
    restore(&db, &doc, first);
    assert_eq!(history(&db, &doc).len(), before);
}

#[test]
fn a_task_keeps_what_a_restore_replaced_through_rapid_changes() {
    let db = context();
    let ws = workspace(&db, "w");
    let task = task(&db, ws, None);
    later(&db);
    db.send_blocking(ChangeTaskStatusCommand {
        workspace_id: ws,
        task_id: task.id,
        status: TaskStatus::Completed,
    })
    .unwrap();
    later(&db);
    let versions = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Task { task_id: task.id },
        })
        .unwrap();
    let (done, open) = (versions[0].id, versions[1].id);

    let restored = db
        .send_blocking(RestoreTaskVersionCommand {
            workspace_id: ws,
            version_id: open,
        })
        .unwrap();
    assert_eq!(restored.status, TaskStatus::Incomplete);
    db.send_blocking(ChangeTaskTitleCommand {
        workspace_id: ws,
        task_id: task.id,
        title: "renamed right after".to_string(),
    })
    .unwrap();

    let undone = db
        .send_blocking(RestoreTaskVersionCommand {
            workspace_id: ws,
            version_id: done,
        })
        .unwrap();
    assert_eq!(undone.status, TaskStatus::Completed);
    assert_eq!(undone.title, "first");
    let kept = db
        .send_blocking(GetVersionCommand {
            workspace_id: ws,
            version_id: db
                .send_blocking(ListVersionsCommand {
                    workspace_id: ws,
                    item: VersionTarget::Task { task_id: task.id },
                })
                .unwrap()[1]
                .id,
        })
        .unwrap();
    assert_eq!(kept.title, "renamed right after");
}

#[test]
fn a_deleted_item_edited_right_after_it_comes_back_keeps_what_it_held() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    later(&db);
    edit(&db, &doc, "held when deleted");
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: doc.id,
    })
    .unwrap();
    let deletion = ids(&db, &doc)[0];
    assert_eq!(restore(&db, &doc, deletion).contents, "held when deleted");
    edit(&db, &doc, "typed right after");
    let back = restore(&db, &doc, deletion);
    assert_eq!(back.contents, "held when deleted");
    assert!(recoverable(&db, &doc, "typed right after"));
}

/// Where the rule and a restore meet: edits alone stay two minutes apart, but
/// a restore keeps its own version however recent the one before it, which is
/// what lets it be undone.
#[test]
fn a_restore_or_a_deletion_can_land_inside_the_two_minutes() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, _) = two_versions(&db, ws);
    edit(&db, &doc, "a fresh edit");
    assert_eq!(history(&db, &doc).len(), 3);
    restore(&db, &doc, first);
    assert_eq!(history(&db, &doc).len(), 4);
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: doc.id,
    })
    .unwrap();
    let kept = history(&db, &doc);
    assert_eq!(kept.len(), 5);
    assert_eq!(kept[0].0, VersionChange::Deleted);
    assert_eq!(kept[1].0, VersionChange::Restored);
    assert_eq!(kept[2].1, "a fresh edit");
}

/// An edit inside the window joins the restored version: it holds the edit and
/// says Edited from then on, so the restored text is only in the version it
/// came from.
#[test]
fn an_edit_right_after_a_restore_takes_over_its_version() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, _) = two_versions(&db, ws);
    restore(&db, &doc, first);
    assert_eq!(history(&db, &doc)[0].0, VersionChange::Restored);
    edit(&db, &doc, "typed right after");
    let kept = history(&db, &doc);
    assert_eq!(kept.len(), 3);
    assert_eq!(
        kept[0],
        (VersionChange::Edited, "typed right after".to_string())
    );
}

/// What that costs: the version a restore came from can expire, and then the
/// text restored is gone if an edit took over the restored version.
#[test]
fn restored_text_goes_with_its_source_once_an_edit_took_over_its_version() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, first, _) = two_versions(&db, ws);
    restore(&db, &doc, first);
    edit(&db, &doc, "typed right after");
    db.send_blocking(RawSql(
        "UPDATE versions SET modified = modified - 8 * 86400 WHERE body = 'first'",
    ))
    .unwrap();
    db.send_blocking(PeriodicCleanupCommand).unwrap();
    assert!(!recoverable(&db, &doc, "first"));
    assert!(recoverable(&db, &doc, "second"));
    assert!(recoverable(&db, &doc, "typed right after"));
}

/// What the window itself gives up: edits inside it keep only the last.
#[test]
fn edits_inside_a_window_keep_only_the_last_whoever_made_the_others() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, _, _) = two_versions(&db, ws);
    edit(&db, &doc, "the user's good text");
    let agent = agent(&db, "Ada");
    db.send_blocking_traced(
        EditDocumentCommand {
            workspace_id: ws,
            document_id: doc.id,
            contents: "the agent's overwrite".to_string(),
        },
        DatabaseEventTrace::default().from_agent(agent),
    )
    .unwrap();
    assert!(!recoverable(&db, &doc, "the user's good text"));
    assert!(recoverable(&db, &doc, "second"));
    assert!(recoverable(&db, &doc, "the agent's overwrite"));
}

#[test]
fn the_window_runs_two_minutes_from_a_versions_start() {
    let db = context();
    let ws = workspace(&db, "w");
    let (doc, _, _) = two_versions(&db, ws);
    edit(&db, &doc, "one");
    db.send_blocking(RawSql(
        "UPDATE versions SET created = unixepoch() - 60
         WHERE id = (SELECT MAX(id) FROM versions)",
    ))
    .unwrap();
    edit(&db, &doc, "joins at a minute");
    assert_eq!(history(&db, &doc).len(), 3);
    db.send_blocking(RawSql(
        "UPDATE versions SET created = unixepoch() - 130
         WHERE id = (SELECT MAX(id) FROM versions)",
    ))
    .unwrap();
    edit(&db, &doc, "starts anew past two");
    assert_eq!(history(&db, &doc).len(), 4);
}

#[test]
fn an_extensions_write_carries_its_name_even_once_it_is_gone() {
    let db = context();
    let ws = workspace(&db, "w");
    let doc = document(&db, ws);
    db.send_blocking(RawSql(
        "INSERT INTO extensions (name, version, script, script_identifier)
         VALUES ('Linear sync', 1, '', 'com.example.linear')",
    ))
    .unwrap();
    later(&db);
    db.send_blocking_traced(
        EditDocumentCommand {
            workspace_id: ws,
            document_id: doc.id,
            contents: "synced".to_string(),
        },
        DatabaseEventTrace::default().from_extension(Arc::new("com.example.linear".to_string())),
    )
    .unwrap();
    db.send_blocking(RawSql("DELETE FROM extensions")).unwrap();
    let latest = db
        .send_blocking(ListVersionsCommand {
            workspace_id: ws,
            item: VersionTarget::Document {
                document_id: doc.id,
            },
        })
        .unwrap()
        .remove(0);
    assert_eq!(latest.author, VersionAuthor::Extension);
    assert_eq!(latest.author_name.as_deref(), Some("Linear sync"));
}
