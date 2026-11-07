use std::collections::HashMap;

use cohesion_core::model::{MetadataOwner, TagFilter};
use cohesion_core::{TaskFolderFilter, TaskSort};

use crate::test_support::context;
use crate::{
    CreateTaskCommand, CreateWorkspaceCommand, DeleteWorkspaceCommand, GetMetadataCommand,
    ListTasksCommand, ListWorkspacesCommand, SetMetadataCommand, TaskListing,
};

#[test]
fn metadata_round_trips_and_filters_lists() {
    let db = context();
    let workspace = |title: &str| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
    };
    let (home, work) = (workspace("home"), workspace("work"));
    let task = |workspace_id, title: &str| {
        db.send_blocking(CreateTaskCommand {
            workspace_id,
            title: title.to_string(),
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
    };
    let set = |owner, key: &str, value: Option<&str>| {
        db.send_blocking(SetMetadataCommand {
            owner,
            key: key.to_string(),
            value: value.map(str::to_string),
        })
        .unwrap()
    };
    let get = |entity_id, key: &str| {
        db.send_blocking(GetMetadataCommand {
            owner: MetadataOwner::Task { task_id: entity_id },
            key: key.to_string(),
        })
        .unwrap()
    };
    let pairs = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    };
    let titles = |workspace_id, metadata| {
        db.send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id,
            folder: TaskFolderFilter::Any,
            statuses: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            metadata: Some(metadata),
            has_metadata: None,
            sort: TaskSort::Created,
            descending: false,
        }))
        .unwrap()
        .into_iter()
        .map(|task| task.title)
        .collect::<Vec<_>>()
    };
    let keyed = |keys: &[&str]| {
        db.send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(home.id),
            folder: TaskFolderFilter::Any,
            statuses: None,
            metadata: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            has_metadata: Some(keys.iter().map(|key| key.to_string()).collect()),
            sort: TaskSort::Created,
            descending: false,
        }))
        .unwrap()
        .into_iter()
        .map(|task| task.title)
        .collect::<Vec<_>>()
    };

    let (apples, pears, other) = (
        task(home.id, "apples"),
        task(home.id, "pears"),
        task(work.id, "other"),
    );
    assert_eq!(get(apples.id, "todoist_id"), None);
    set(
        MetadataOwner::Task { task_id: apples.id },
        "todoist_id",
        Some("1"),
    );
    set(
        MetadataOwner::Task { task_id: apples.id },
        "todoist_id",
        Some("2"),
    );
    assert_eq!(get(apples.id, "todoist_id").as_deref(), Some("2"));
    set(
        MetadataOwner::Task { task_id: apples.id },
        "source",
        Some("sync"),
    );
    set(
        MetadataOwner::Task { task_id: pears.id },
        "todoist_id",
        Some("2"),
    );
    set(
        MetadataOwner::Task { task_id: other.id },
        "todoist_id",
        Some("2"),
    );
    // The same id on another kind of entity is a different entity.
    set(
        MetadataOwner::Workspace {
            workspace_id: apples.id,
        },
        "todoist_id",
        Some("3"),
    );

    assert_eq!(
        titles(Some(home.id), pairs(&[("todoist_id", "2")])),
        ["apples", "pears"]
    );
    assert_eq!(
        titles(
            Some(home.id),
            pairs(&[("todoist_id", "2"), ("source", "sync")])
        ),
        ["apples"]
    );
    assert!(titles(Some(home.id), pairs(&[("todoist_id", "3")])).is_empty());
    assert_eq!(keyed(&["todoist_id"]), ["apples", "pears"]);
    assert_eq!(keyed(&["todoist_id", "source"]), ["apples"]);
    assert!(keyed(&["missing"]).is_empty());

    set(
        MetadataOwner::Task { task_id: apples.id },
        "todoist_id",
        None,
    );
    assert_eq!(get(apples.id, "todoist_id"), None);
    assert_eq!(
        titles(Some(home.id), pairs(&[("todoist_id", "2")])),
        ["pears"]
    );

    set(
        MetadataOwner::Workspace {
            workspace_id: work.id,
        },
        "team",
        Some("a"),
    );
    let filtered = db
        .send_blocking(ListWorkspacesCommand {
            metadata: pairs(&[("team", "a")]),
            has_metadata: Vec::new(),
        })
        .unwrap();
    assert_eq!(filtered.iter().map(|w| w.id).collect::<Vec<_>>(), [work.id]);
    let keyed_workspaces = db
        .send_blocking(ListWorkspacesCommand {
            metadata: HashMap::new(),
            has_metadata: vec!["team".to_string()],
        })
        .unwrap();
    assert_eq!(
        keyed_workspaces.iter().map(|w| w.id).collect::<Vec<_>>(),
        [work.id]
    );
    db.send_blocking(DeleteWorkspaceCommand { id: work.id })
        .unwrap();
    assert!(
        db.send_blocking(ListWorkspacesCommand {
            metadata: pairs(&[("team", "a")]),
            has_metadata: Vec::new(),
        })
        .unwrap()
        .is_empty()
    );
}
