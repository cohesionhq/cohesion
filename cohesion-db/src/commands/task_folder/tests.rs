use std::collections::HashMap;

use crate::test_support::context;
use crate::{
    CreateTaskFolderCommand, CreateWorkspaceCommand, ListTaskFoldersCommand,
    ReorderTaskFoldersCommand,
};

#[test]
fn reordered_folders_list_in_the_new_order_and_new_ones_go_last() {
    let db = context();
    let ws = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    let [a, b, c] = ["a", "b", "c"].map(|title| {
        db.send_blocking(CreateTaskFolderCommand {
            workspace_id: ws.id,
            title: title.to_string(),
        })
        .unwrap()
    });
    let titles = |folders: Vec<cohesion_core::TaskFolder>| {
        folders.into_iter().map(|f| f.title).collect::<Vec<_>>()
    };

    let reordered = db
        .send_blocking(ReorderTaskFoldersCommand {
            workspace_id: ws.id,
            folder_ids: vec![c.id, a.id, b.id],
        })
        .unwrap();
    assert_eq!(titles(reordered), ["c", "a", "b"]);

    db.send_blocking(CreateTaskFolderCommand {
        workspace_id: ws.id,
        title: "d".to_string(),
    })
    .unwrap();
    let listed = db
        .send_blocking(ListTaskFoldersCommand {
            workspace_id: ws.id,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert_eq!(titles(listed), ["c", "a", "b", "d"]);
}
