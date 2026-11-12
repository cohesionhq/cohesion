use std::collections::HashMap;

use cohesion_core::DocumentType;
use cohesion_core::model::{DocumentSummary, OPENING_CHARS};
use cohesion_core::{Document, Named};

use crate::test_support::{context, recording_context};
use crate::{
    AppendToDocumentCommand, AppendToJournalCommand, CommandError, CreateDocumentCommand,
    CreateWorkspaceCommand, DatabaseContext, DatabaseEventKind, DeleteDocumentCommand,
    EditDocumentCommand, EditJournalCommand, EnsureJournalCommand, GetDocumentCommand,
    ListDocumentsCommand, LookupJournalByDateCommand, RenameDocumentCommand,
    SetDocumentPinnedCommand,
};

#[test]
fn journal_commands() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap();
    // Appending creates the day's journal when nothing has fetched it yet.
    let mut journal = None;
    for line in ["a", "b"] {
        journal = Some(
            db.send_blocking(AppendToJournalCommand {
                workspace_id: workspace.id,
                date: "2026-10-13".to_string(),
                content: line.to_string(),
            })
            .unwrap(),
        );
    }
    let journal = journal.unwrap();
    assert_eq!(journal.title, "2026-10-13");
    assert_eq!(journal.journal_date.as_deref(), Some("2026-10-13"));
    let again = db
        .send_blocking(LookupJournalByDateCommand {
            workspace_id: workspace.id,
            date: "2026-10-13".to_string(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(again.id, journal.id);
    assert_eq!(again.contents, "a\nb");
    let edited = db
        .send_blocking(EditJournalCommand {
            workspace_id: workspace.id,
            date: "2026-10-13".to_string(),
            content: "c".to_string(),
        })
        .unwrap();
    assert_eq!(edited.contents, "c");
    // Getting a day nothing was written on creates nothing; editing it
    // creates another journal, and only a real `YYYY-MM-DD` makes one.
    let next_day = || LookupJournalByDateCommand {
        workspace_id: workspace.id,
        date: "2026-10-14".to_string(),
    };
    assert!(db.send_blocking(next_day()).unwrap().is_none());
    let next = db
        .send_blocking(EditJournalCommand {
            workspace_id: workspace.id,
            date: "2026-10-14".to_string(),
            content: "d".to_string(),
        })
        .unwrap();
    assert_ne!(next.id, journal.id);
    assert_eq!(db.send_blocking(next_day()).unwrap().unwrap().id, next.id);
    assert!(matches!(
        db.send_blocking(EnsureJournalCommand {
            workspace_id: workspace.id,
            date: "2026-02-30".to_string(),
        }),
        Err(CommandError::Validation(_))
    ));
    let listed = db
        .send_blocking(ListDocumentsCommand {
            workspace_id: workspace.id,
            document_type: None,
            journal_from: None,
            journal_to: None,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert_eq!(listed.len(), 2);
}

/// Titles of the live documents `ListDocumentsCommand` returns.
fn listed_titles(
    db: &DatabaseContext,
    workspace_id: i64,
    document_type: Option<DocumentType>,
    journal_range: Option<(&str, &str)>,
) -> Vec<String> {
    db.send_blocking(ListDocumentsCommand {
        workspace_id,
        document_type,
        journal_from: journal_range.map(|(from, _)| from.to_string()),
        journal_to: journal_range.map(|(_, to)| to.to_string()),
        metadata: HashMap::new(),
        has_metadata: Vec::new(),
    })
    .unwrap()
    .into_iter()
    .map(|document| document.title)
    .collect()
}

#[test]
fn document_lifecycle_and_list_filters() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap();
    let journal = db
        .send_blocking(EnsureJournalCommand {
            workspace_id: workspace.id,
            date: "2026-10-13".to_string(),
        })
        .unwrap();
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: workspace.id,
            title: "draft".to_string(),
            contents: "body".to_string(),
            folder_id: None,
        })
        .unwrap();
    assert_eq!(document.journal_date, None);
    let renamed = db
        .send_blocking(RenameDocumentCommand {
            workspace_id: workspace.id,
            document_id: document.id,
            title: "notes".to_string(),
        })
        .unwrap();
    assert_eq!(renamed.title, "notes");

    assert_eq!(
        listed_titles(&db, workspace.id, Some(DocumentType::Normal), None),
        ["notes"]
    );
    assert_eq!(
        listed_titles(&db, workspace.id, None, Some(("2026-10-13", "2026-10-14"))),
        std::slice::from_ref(&journal.title)
    );
    assert!(listed_titles(&db, workspace.id, None, Some(("2026-10-14", "2026-10-15"))).is_empty());
    assert!(listed_titles(&db, workspace.id, None, Some(("2026-10-12", "2026-10-13"))).is_empty());

    db.send_blocking(DeleteDocumentCommand {
        workspace_id: workspace.id,
        document_id: document.id,
    })
    .unwrap();
    assert!(matches!(
        db.send_blocking(GetDocumentCommand {
            workspace_id: workspace.id,
            document_id: document.id,
        }),
        Err(CommandError::NotFound(Document::NOUN))
    ));
    assert_eq!(
        listed_titles(&db, workspace.id, None, None),
        [journal.title]
    );
}

#[test]
fn journal_and_document_saves_raise_events() {
    let (db, events) = recording_context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let date = || "2026-10-14".to_string();
    // Sending any command delivers the previous command's events first.
    let flush = || {
        db.send_blocking(LookupJournalByDateCommand {
            workspace_id,
            date: date(),
        })
        .unwrap();
        events.take()
    };
    // Creating a journal is not a save.
    flush();
    let journal_id = db
        .send_blocking(EnsureJournalCommand {
            workspace_id,
            date: date(),
        })
        .unwrap()
        .id;
    assert_eq!(flush(), []);
    let saved = [DatabaseEventKind::JournalSaved {
        document_id: journal_id,
        workspace_id,
    }];

    db.send_blocking(AppendToJournalCommand {
        workspace_id,
        date: date(),
        content: "a".to_string(),
    })
    .unwrap();
    assert_eq!(flush(), saved);
    db.send_blocking(EditJournalCommand {
        workspace_id,
        date: date(),
        content: "b".to_string(),
    })
    .unwrap();
    assert_eq!(flush(), saved);
    // A journal edited as a document (e.g. `Document.contents` in scripts) counts too.
    db.send_blocking(EditDocumentCommand {
        workspace_id,
        document_id: journal_id,
        contents: "c".to_string(),
    })
    .unwrap();
    assert_eq!(flush(), saved);
    db.send_blocking(AppendToDocumentCommand {
        workspace_id,
        document_id: journal_id,
        contents: "d".to_string(),
    })
    .unwrap();
    assert_eq!(flush(), saved);

    // An ordinary document raises `DocumentSaved` instead, on edits and
    // appends but not on creation.
    let document_id = db
        .send_blocking(CreateDocumentCommand {
            workspace_id,
            title: "draft".to_string(),
            contents: String::new(),
            folder_id: None,
        })
        .unwrap()
        .id;
    db.send_blocking(EditDocumentCommand {
        workspace_id,
        document_id,
        contents: "body".to_string(),
    })
    .unwrap();
    let document_saved = [DatabaseEventKind::DocumentSaved {
        document_id,
        workspace_id,
    }];
    assert_eq!(flush(), document_saved);
    db.send_blocking(AppendToDocumentCommand {
        workspace_id,
        document_id,
        contents: " more".to_string(),
    })
    .unwrap();
    assert_eq!(flush(), document_saved);
}

#[test]
fn a_document_can_be_pinned_and_unpinned_without_being_edited() {
    let db = context();
    let home = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    let away = db
        .send_blocking(CreateWorkspaceCommand {
            title: "away".to_string(),
            emoji: None,
        })
        .unwrap();
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: home.id,
            title: "draft".to_string(),
            contents: "body".to_string(),
            folder_id: None,
        })
        .unwrap();
    assert!(!document.pinned);

    let pinned = db
        .send_blocking(SetDocumentPinnedCommand {
            workspace_id: home.id,
            document_id: document.id,
            pinned: true,
        })
        .unwrap();
    assert!(pinned.pinned);
    assert_eq!(pinned.modified, document.modified);
    let listed = db
        .send_blocking(ListDocumentsCommand {
            workspace_id: home.id,
            document_type: None,
            journal_from: None,
            journal_to: None,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert!(listed.iter().any(|d| d.id == document.id && d.pinned));

    let unpinned = db
        .send_blocking(SetDocumentPinnedCommand {
            workspace_id: home.id,
            document_id: document.id,
            pinned: false,
        })
        .unwrap();
    assert!(!unpinned.pinned);

    let elsewhere = db.send_blocking(SetDocumentPinnedCommand {
        workspace_id: away.id,
        document_id: document.id,
        pinned: true,
    });
    assert!(matches!(elsewhere, Err(CommandError::NotFound(_))));
}

#[test]
fn a_listed_documents_opening_is_cut_as_a_summary_of_it_is() {
    let db = context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id,
            title: "Long".to_string(),
            contents: "é".repeat(OPENING_CHARS + 100),
            folder_id: None,
        })
        .unwrap();
    let listed = db
        .send_blocking(ListDocumentsCommand {
            workspace_id,
            document_type: None,
            journal_from: None,
            journal_to: None,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    // SQLite's `substr` and `From<Document>` count characters alike.
    assert_eq!(listed[0].opening, DocumentSummary::from(document).opening);
    assert_eq!(listed[0].opening.chars().count(), OPENING_CHARS);
}
