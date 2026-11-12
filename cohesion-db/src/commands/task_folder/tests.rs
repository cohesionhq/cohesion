use std::collections::HashMap;

use crate::test_support::context;
use crate::{
    CreateTaskFolderCommand, CreateWorkspaceCommand, ListTaskFoldersCommand,
    ReorderTaskFoldersCommand, SetTaskFolderPinnedCommand,
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

#[test]
fn a_pinned_folder_keeps_its_place_and_lists_as_pinned() {
    let db = context();
    let ws = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    let [a, b] = ["a", "b"].map(|title| {
        db.send_blocking(CreateTaskFolderCommand {
            workspace_id: ws.id,
            title: title.to_string(),
        })
        .unwrap()
    });
    let pinned = db
        .send_blocking(SetTaskFolderPinnedCommand {
            workspace_id: ws.id,
            folder_id: b.id,
            pinned: true,
        })
        .unwrap();
    assert!(pinned.pinned);
    assert_eq!(pinned.sort_order, b.sort_order);

    let listed = db
        .send_blocking(ListTaskFoldersCommand {
            workspace_id: ws.id,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    let pins: Vec<_> = listed.iter().map(|f| (f.id, f.pinned)).collect();
    assert_eq!(pins, [(a.id, false), (b.id, true)]);
}
