use std::collections::HashMap;

use cohesion_core::{Document, DocumentFolder};

use crate::test_support::context;
use crate::{
    CommandError, CreateDocumentCommand, CreateDocumentFolderCommand, CreateWorkspaceCommand,
    DatabaseContext, DeleteDocumentCommand, DeleteDocumentFolderCommand, EnsureJournalCommand,
    GetDocumentCommand, ListDocumentFoldersCommand, ListDocumentsCommand,
    ListFiledDocumentsCommand, MoveDocumentCommand, MoveDocumentFolderCommand,
    RenameDocumentFolderCommand,
};

fn workspace(db: &DatabaseContext, title: &str) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: title.to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn folder(db: &DatabaseContext, ws: i64, title: &str, parent_id: Option<i64>) -> DocumentFolder {
    db.send_blocking(CreateDocumentFolderCommand {
        workspace_id: ws,
        title: title.to_string(),
        parent_id,
    })
    .unwrap()
}

fn document(db: &DatabaseContext, ws: i64, title: &str, folder_id: Option<i64>) -> Document {
    db.send_blocking(CreateDocumentCommand {
        workspace_id: ws,
        title: title.to_string(),
        contents: String::new(),
        folder_id,
    })
    .unwrap()
}

/// Each live folder of `ws` as `(title, parent title)`, in listing order.
fn tree(db: &DatabaseContext, ws: i64) -> Vec<(String, Option<String>)> {
    let folders = db
        .send_blocking(ListDocumentFoldersCommand { workspace_id: ws })
        .unwrap();
    let title = |id: i64| folders.iter().find(|f| f.id == id).unwrap().title.clone();
    folders
        .iter()
        .map(|f| (f.title.clone(), f.parent_id.map(title)))
        .collect()
}

fn filed(db: &DatabaseContext, ws: i64, folder_id: Option<i64>) -> Vec<String> {
    db.send_blocking(ListFiledDocumentsCommand {
        workspace_id: ws,
        folder_id,
    })
    .unwrap()
    .into_iter()
    .map(|d| d.title)
    .collect()
}

fn pair(title: &str, parent: Option<&str>) -> (String, Option<String>) {
    (title.to_string(), parent.map(str::to_string))
}

#[test]
fn folders_nest_and_documents_file_into_them_or_the_inbox() {
    let db = context();
    let ws = workspace(&db, "home");
    let work = folder(&db, ws, "Work", None);
    let plans = folder(&db, ws, "Plans", Some(work.id));
    let old = folder(&db, ws, "Old", Some(plans.id));
    assert_eq!(
        tree(&db, ws),
        [
            pair("Work", None),
            pair("Plans", Some("Work")),
            pair("Old", Some("Plans"))
        ]
    );

    let loose = document(&db, ws, "Loose", None);
    let launch = document(&db, ws, "Launch", Some(old.id));
    assert_eq!(loose.folder_id, None);
    assert_eq!(launch.folder_id, Some(old.id));
    assert_eq!(filed(&db, ws, None), ["Loose"]);
    assert_eq!(filed(&db, ws, Some(old.id)), ["Launch"]);
    // A folder lists only what is filed directly in it.
    assert!(filed(&db, ws, Some(work.id)).is_empty());

    let moved = db
        .send_blocking(MoveDocumentCommand {
            workspace_id: ws,
            document_id: loose.id,
            folder_id: Some(work.id),
        })
        .unwrap();
    assert_eq!(moved.folder_id, Some(work.id));
    assert!(filed(&db, ws, None).is_empty());
    let back = db
        .send_blocking(MoveDocumentCommand {
            workspace_id: ws,
            document_id: loose.id,
            folder_id: None,
        })
        .unwrap();
    assert_eq!(back.folder_id, None);

    let renamed = db
        .send_blocking(RenameDocumentFolderCommand {
            workspace_id: ws,
            folder_id: work.id,
            title: "Job".to_string(),
        })
        .unwrap();
    assert_eq!(renamed.title, "Job");
    assert!(matches!(
        db.send_blocking(RenameDocumentFolderCommand {
            workspace_id: ws,
            folder_id: work.id,
            title: String::new(),
        }),
        Err(CommandError::Validation(_))
    ));
}

#[test]
fn a_folder_moves_among_siblings_and_never_into_itself() {
    let db = context();
    let ws = workspace(&db, "home");
    let a = folder(&db, ws, "A", None);
    let b = folder(&db, ws, "B", None);
    let c = folder(&db, ws, "C", None);
    let inner = folder(&db, ws, "Inner", Some(a.id));
    let move_folder = |folder_id: i64, parent_id: Option<i64>, before_id: Option<i64>| {
        db.send_blocking(MoveDocumentFolderCommand {
            workspace_id: ws,
            folder_id,
            parent_id,
            before_id,
        })
    };

    // Before a sibling, at the same level.
    move_folder(c.id, None, Some(a.id)).unwrap();
    let top: Vec<String> = tree(&db, ws)
        .into_iter()
        .filter(|(_, parent)| parent.is_none())
        .map(|(title, _)| title)
        .collect();
    assert_eq!(top, ["C", "A", "B"]);

    // Into another folder, last there.
    move_folder(b.id, Some(a.id), None).unwrap();
    let in_a: Vec<String> = tree(&db, ws)
        .into_iter()
        .filter(|(_, parent)| parent.as_deref() == Some("A"))
        .map(|(title, _)| title)
        .collect();
    assert_eq!(in_a, ["Inner", "B"]);

    // Not into itself or anything inside it.
    for parent in [a.id, inner.id] {
        assert!(matches!(
            move_folder(a.id, Some(parent), None),
            Err(CommandError::Validation(_))
        ));
    }
    // Not before a folder that isn't a sibling there.
    assert!(matches!(
        move_folder(c.id, None, Some(inner.id)),
        Err(CommandError::Validation(_))
    ));
    // Back to the top level, last.
    let moved = move_folder(inner.id, None, None).unwrap();
    let inner = moved.iter().find(|f| f.id == inner.id).unwrap();
    assert_eq!(inner.parent_id, None);
}

#[test]
fn deleting_a_folder_deletes_those_inside_and_sends_their_documents_to_the_inbox() {
    let db = context();
    let ws = workspace(&db, "home");
    let keep = folder(&db, ws, "Keep", None);
    let work = folder(&db, ws, "Work", None);
    let plans = folder(&db, ws, "Plans", Some(work.id));
    let kept = document(&db, ws, "Kept", Some(keep.id));
    let launch = document(&db, ws, "Launch", Some(work.id));
    let draft = document(&db, ws, "Draft", Some(plans.id));
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: draft.id,
    })
    .unwrap();

    let deleted = db
        .send_blocking(DeleteDocumentFolderCommand {
            workspace_id: ws,
            folder_id: work.id,
        })
        .unwrap();
    assert!(deleted.deleted);
    assert_eq!(tree(&db, ws), [pair("Keep", None)]);
    assert_eq!(filed(&db, ws, None), ["Launch"]);
    assert_eq!(filed(&db, ws, Some(keep.id)), ["Kept"]);
    // A deleted document leaves the folder too, so restoring it never
    // finds its folder gone.
    let draft_folder: Option<i64> = db
        .send_blocking(crate::GetDocumentIncludingDeletedCommand {
            workspace_id: ws,
            document_id: draft.id,
        })
        .unwrap()
        .folder_id;
    assert_eq!(draft_folder, None);
    let launch = db
        .send_blocking(GetDocumentCommand {
            workspace_id: ws,
            document_id: launch.id,
        })
        .unwrap();
    assert_eq!(launch.folder_id, None);
    assert_eq!(kept.folder_id, Some(keep.id));
    // A deleted folder takes nothing new.
    assert!(matches!(
        db.send_blocking(CreateDocumentFolderCommand {
            workspace_id: ws,
            title: "Child".to_string(),
            parent_id: Some(plans.id),
        }),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn folders_stay_in_their_workspace_and_journals_stay_out() {
    let db = context();
    let home = workspace(&db, "home");
    let work = workspace(&db, "work");
    let theirs = folder(&db, work, "Theirs", None);
    let doc = document(&db, home, "Mine", None);
    fn not_found<T>(result: Result<T, CommandError>) {
        assert!(matches!(result, Err(CommandError::NotFound(_))));
    }
    not_found(db.send_blocking(CreateDocumentCommand {
        workspace_id: home,
        title: "x".to_string(),
        contents: String::new(),
        folder_id: Some(theirs.id),
    }));
    not_found(db.send_blocking(MoveDocumentCommand {
        workspace_id: home,
        document_id: doc.id,
        folder_id: Some(theirs.id),
    }));
    not_found(db.send_blocking(CreateDocumentFolderCommand {
        workspace_id: home,
        title: "x".to_string(),
        parent_id: Some(theirs.id),
    }));
    not_found(db.send_blocking(DeleteDocumentFolderCommand {
        workspace_id: home,
        folder_id: theirs.id,
    }));

    let mine = folder(&db, home, "Mine", None);
    let journal = db
        .send_blocking(EnsureJournalCommand {
            workspace_id: home,
            date: "2026-11-09".to_string(),
        })
        .unwrap();
    assert!(matches!(
        db.send_blocking(MoveDocumentCommand {
            workspace_id: home,
            document_id: journal.id,
            folder_id: Some(mine.id),
        }),
        Err(CommandError::Validation(_))
    ));
    // The Inbox lists no journals, though they are in no folder.
    assert_eq!(filed(&db, home, None), ["Mine"]);
    let all = db
        .send_blocking(ListDocumentsCommand {
            workspace_id: home,
            document_type: None,
            journal_from: None,
            journal_to: None,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert_eq!(all.len(), 2);
}
