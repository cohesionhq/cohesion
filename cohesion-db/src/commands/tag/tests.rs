use cohesion_core::Task;
use cohesion_core::model::TagFilter;

use crate::test_support::{context, recording_context};
use crate::{
    CommandError, CreateTagCommand, CreateTaskCommand, CreateWorkspaceCommand, DatabaseContext,
    DatabaseEventKind, DeleteTagCommand, ListTagsCommand, ListTasksCommand, ReorderTagsCommand,
    SetTaggedCommand, TaskListing,
};

fn workspace(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: "w".to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn tag(db: &DatabaseContext, ws: i64, name: &str) -> i64 {
    db.send_blocking(CreateTagCommand {
        workspace_id: ws,
        name: name.to_string(),
    })
    .unwrap()
    .id
}

fn task(db: &DatabaseContext, ws: i64) -> i64 {
    db.send_blocking(CreateTaskCommand {
        workspace_id: ws,
        title: "t".to_string(),
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
    .id
}

fn set(
    db: &DatabaseContext,
    ws: i64,
    tag_id: i64,
    task_id: i64,
    tagged: bool,
) -> Result<Task, CommandError> {
    db.send_blocking(SetTaggedCommand {
        workspace_id: ws,
        tag_id,
        task_id,
        tagged,
    })
}

fn tagged_tasks(db: &DatabaseContext, ws: i64, tag_id: i64) -> Vec<i64> {
    listed_tasks(
        db,
        ws,
        TagFilter {
            with: vec![tag_id],
            without: Vec::new(),
        },
    )
}

fn listed_tasks(db: &DatabaseContext, ws: i64, tags: TagFilter) -> Vec<i64> {
    db.send_blocking(ListTasksCommand(TaskListing {
        ids: None,
        workspace_id: Some(ws),
        folder: Default::default(),
        priorities: None,
        sizes: None,
        start_date: None,
        due_date: None,
        assignee: None,
        tags,
        statuses: None,
        metadata: None,
        has_metadata: None,
        sort: Default::default(),
        descending: false,
    }))
    .unwrap()
    .into_iter()
    .map(|task| task.id)
    .collect()
}

#[test]
fn a_tag_goes_on_its_workspaces_tasks_and_lists_them() {
    let db = context();
    let (ws, other) = (workspace(&db), workspace(&db));
    let urgent = tag(&db, ws, "urgent");
    let (here, elsewhere) = (task(&db, ws), task(&db, other));

    assert_eq!(
        set(&db, ws, urgent, here, true).unwrap().tag_ids.0,
        [urgent]
    );
    // Tagging twice keeps one.
    set(&db, ws, urgent, here, true).unwrap();
    assert_eq!(tagged_tasks(&db, ws, urgent), [here]);

    assert!(matches!(
        set(&db, ws, urgent, elsewhere, true),
        Err(CommandError::InvalidReference)
    ));

    set(&db, ws, urgent, here, false).unwrap();
    assert!(tagged_tasks(&db, ws, urgent).is_empty());
}

#[test]
fn a_listing_keeps_tasks_with_every_wanted_tag_and_none_unwanted() {
    let db = context();
    let ws = workspace(&db);
    let core = tag(&db, ws, "Core");
    let plus = tag(&db, ws, "Plus");
    let both = task(&db, ws);
    let only_core = task(&db, ws);
    let untagged = task(&db, ws);
    set(&db, ws, core, both, true).unwrap();
    set(&db, ws, plus, both, true).unwrap();
    set(&db, ws, core, only_core, true).unwrap();
    let filter = |with: Vec<i64>, without: Vec<i64>| TagFilter { with, without };

    assert_eq!(
        listed_tasks(&db, ws, filter(vec![], vec![])),
        [both, only_core, untagged]
    );
    assert_eq!(
        listed_tasks(&db, ws, filter(vec![core, plus], vec![])),
        [both]
    );
    assert_eq!(
        listed_tasks(&db, ws, filter(vec![core], vec![plus])),
        [only_core]
    );
    assert_eq!(
        listed_tasks(&db, ws, filter(vec![], vec![plus])),
        [only_core, untagged]
    );
    assert!(listed_tasks(&db, ws, filter(vec![core], vec![core])).is_empty());
}

#[test]
fn a_tag_name_is_unique_in_a_workspace_ignoring_case() {
    let db = context();
    let (ws, other) = (workspace(&db), workspace(&db));
    tag(&db, ws, "Reading");
    tag(&db, other, "reading");
    let taken = db.send_blocking(CreateTagCommand {
        workspace_id: ws,
        name: "reading".to_string(),
    });
    assert!(matches!(taken, Err(CommandError::Validation(_))));
}

#[test]
fn deleting_a_tag_takes_it_off_everything() {
    let db = context();
    let ws = workspace(&db);
    let urgent = tag(&db, ws, "urgent");
    let t = task(&db, ws);
    set(&db, ws, urgent, t, true).unwrap();
    db.send_blocking(DeleteTagCommand {
        workspace_id: ws,
        tag_id: urgent,
    })
    .unwrap();
    let again = tag(&db, ws, "urgent");
    assert!(tagged_tasks(&db, ws, again).is_empty());
    assert!(tagged_tasks(&db, ws, urgent).is_empty());
}

#[test]
fn tags_keep_the_order_they_are_given() {
    let db = context();
    let ws = workspace(&db);
    let (b, a) = (tag(&db, ws, "b"), tag(&db, ws, "a"));
    let list = |db: &DatabaseContext| -> Vec<i64> {
        db.send_blocking(ListTagsCommand { workspace_id: ws })
            .unwrap()
            .into_iter()
            .map(|tag| tag.id)
            .collect()
    };
    assert_eq!(list(&db), [b, a]);

    let reordered = db
        .send_blocking(ReorderTagsCommand {
            workspace_id: ws,
            tag_ids: vec![a, b],
        })
        .unwrap();
    assert_eq!(reordered.iter().map(|t| t.id).collect::<Vec<_>>(), [a, b]);

    let c = tag(&db, ws, "0");
    assert_eq!(list(&db), [a, b, c]);
}

#[test]
fn tagging_raises_events_only_when_a_tag_goes_on_or_comes_off() {
    let (db, events) = recording_context();
    let ws = workspace(&db);
    let task_tag = tag(&db, ws, "urgent");
    let task_id = task(&db, ws);
    events.take();

    let task_tagged = [DatabaseEventKind::TaskTagged {
        task_id,
        workspace_id: ws,
    }];
    set(&db, ws, task_tag, task_id, true).unwrap();
    assert_eq!(events.take(), task_tagged);
    set(&db, ws, task_tag, task_id, true).unwrap();
    assert_eq!(events.take(), []);
    set(&db, ws, task_tag, task_id, false).unwrap();
    assert_eq!(events.take(), task_tagged);
    set(&db, ws, task_tag, task_id, false).unwrap();
    assert_eq!(events.take(), []);
}

#[test]
fn a_task_created_with_tags_carries_them_or_is_not_created() {
    let (db, events) = recording_context();
    let ws = workspace(&db);
    let other = workspace(&db);
    let urgent = tag(&db, ws, "urgent");
    let elsewhere = tag(&db, other, "elsewhere");
    let create = |tag_ids: Vec<i64>| {
        db.send_blocking(CreateTaskCommand {
            workspace_id: ws,
            title: "t".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids,
        })
    };
    events.take();

    let task = create(vec![urgent]).unwrap();
    assert_eq!(task.tag_ids.0, [urgent]);
    assert_eq!(
        events.take(),
        [
            DatabaseEventKind::TaskCreated {
                task_id: task.id,
                workspace_id: ws,
            },
            DatabaseEventKind::TaskTagged {
                task_id: task.id,
                workspace_id: ws,
            },
        ]
    );

    assert!(create(vec![urgent, elsewhere]).is_err());
    let listed = db
        .send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(ws),
            folder: Default::default(),
            statuses: None,
            metadata: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            has_metadata: None,
            sort: Default::default(),
            descending: false,
        }))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(events.take(), []);
}
