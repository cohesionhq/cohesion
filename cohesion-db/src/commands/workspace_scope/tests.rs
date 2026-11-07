use std::collections::HashMap;

use cohesion_core::{Named, Workspace};

use crate::test_support::context;
use crate::{
    CommandError, CreateWorkspaceCommand, DeleteWorkspaceCommand, ListTaskFoldersCommand,
    ListWorkspacesCommand, LookupJournalByDateCommand, SearchDocumentsCommand,
};

#[test]
fn workspace_commands_require_a_live_workspace() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap();
    assert!(
        db.send_blocking(ListTaskFoldersCommand {
            workspace_id: workspace.id,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap()
        .is_empty()
    );
    db.send_blocking(DeleteWorkspaceCommand { id: workspace.id })
        .unwrap();
    assert!(
        db.send_blocking(ListWorkspacesCommand::default())
            .unwrap()
            .is_empty()
    );
    for workspace_id in [workspace.id, 999] {
        assert!(matches!(
            db.send_blocking(LookupJournalByDateCommand {
                workspace_id,
                date: "2026-10-13".to_string(),
            }),
            Err(CommandError::NotFound(Workspace::NOUN))
        ));
        assert!(matches!(
            db.send_blocking(ListTaskFoldersCommand {
                workspace_id,
                metadata: HashMap::new(),
                has_metadata: Vec::new()
            }),
            Err(CommandError::NotFound(Workspace::NOUN))
        ));
        assert!(matches!(
            db.send_blocking(SearchDocumentsCommand {
                workspace_id,
                query: String::new(),
            }),
            Err(CommandError::NotFound(Workspace::NOUN))
        ));
    }
}
