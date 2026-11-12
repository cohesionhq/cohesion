use std::collections::HashMap;

use cohesion_core::model::IntelligenceAgentColor;

use crate::CommandError;
use crate::test_support::context;
use crate::{
    CreateTaskFolderCommand, CreateWorkspaceCommand, ListTaskFoldersCommand,
    ReorderTaskFoldersCommand, SetTaskFolderColorCommand, SetTaskFolderPinnedCommand,
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

#[test]
fn a_folder_starts_with_no_color_and_takes_one_until_cleared() {
    let db = context();
    let ws = db
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
    let folder = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: ws.id,
            title: "a".to_string(),
        })
        .unwrap();
    assert_eq!(folder.color, None);

    let colored = db
        .send_blocking(SetTaskFolderColorCommand {
            workspace_id: ws.id,
            folder_id: folder.id,
            color: Some(IntelligenceAgentColor::Teal),
        })
        .unwrap();
    assert_eq!(colored.color, Some(IntelligenceAgentColor::Teal));
    let listed = db
        .send_blocking(ListTaskFoldersCommand {
            workspace_id: ws.id,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert_eq!(listed[0].color, Some(IntelligenceAgentColor::Teal));

    let elsewhere = db.send_blocking(SetTaskFolderColorCommand {
        workspace_id: away.id,
        folder_id: folder.id,
        color: Some(IntelligenceAgentColor::Red),
    });
    assert!(matches!(elsewhere, Err(CommandError::NotFound(_))));

    let cleared = db
        .send_blocking(SetTaskFolderColorCommand {
            workspace_id: ws.id,
            folder_id: folder.id,
            color: None,
        })
        .unwrap();
    assert_eq!(cleared.color, None);
}
