use cohesion_core::model::{IntelligenceAgentColor, TagFilter};
use cohesion_core::{TaskAssignee, TaskFolderFilter, TaskPriority, TaskSize, TaskSort, TaskStatus};

use crate::test_support::{context, recording_context};
use crate::{
    ChangeTaskAssigneeCommand, ChangeTaskDescriptionCommand, ChangeTaskPriorityCommand,
    ChangeTaskSizeCommand, ChangeTaskStatusCommand, ChangeTaskTitleCommand,
    ChangeTasksStatusCommand, CreateContactCommand, CreateIntelligenceAgentCommand,
    CreateTaskCommand, CreateTaskFolderCommand, CreateWorkspaceCommand, DatabaseContext,
    DatabaseEventKind, DeleteContactCommand, DeleteIntelligenceAgentCommand, DeleteTaskCommand,
    GetTaskCommand, ListTaskPageCommand, ListTasksCommand, ScheduleTaskCommand, TaskListing,
};

#[test]
fn tasks_list_by_workspace_and_folder() {
    let db = context();
    let [home, work] = ["home", "work"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
    });
    let folder = db
        .send_blocking(CreateTaskFolderCommand {
            workspace_id: home.id,
            title: "errands".to_string(),
        })
        .unwrap();
    for (workspace_id, title, folder_id) in [
        (home.id, "milk", Some(folder.id)),
        (home.id, "dishes", None),
        (work.id, "report", None),
    ] {
        db.send_blocking(CreateTaskCommand {
            workspace_id,
            title: title.to_string(),
            folder_id,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    }
    let titles = |folder| {
        let tasks = db
            .send_blocking(ListTasksCommand(TaskListing {
                tags: TagFilter::default(),
                ids: None,
                workspace_id: Some(home.id),
                folder,
                statuses: None,
                metadata: None,
                priorities: None,
                sizes: None,
                start_date: None,
                due_date: None,
                assignee: None,
                has_metadata: None,
                sort: TaskSort::Created,
                descending: false,
            }))
            .unwrap();
        tasks.into_iter().map(|t| t.title).collect::<Vec<_>>()
    };
    assert_eq!(titles(TaskFolderFilter::Any), ["milk", "dishes"]);
    assert_eq!(titles(TaskFolderFilter::Inbox), ["dishes"]);
    assert_eq!(titles(TaskFolderFilter::Folder(folder.id)), ["milk"]);
}

#[test]
fn a_page_lists_its_slice_and_the_whole_total() {
    let (db, workspace_id, _) = status_fixture(&["a", "b", "c", "d", "e"]);
    let page = |offset, limit| {
        let page = db
            .send_blocking(ListTaskPageCommand {
                tags: TagFilter::default(),
                ids: None,
                workspace_id: Some(workspace_id),
                folder: TaskFolderFilter::Any,
                statuses: None,
                metadata: None,
                priorities: None,
                sizes: None,
                start_date: None,
                due_date: None,
                assignee: None,
                has_metadata: None,
                sort: TaskSort::Title,
                descending: true,
                offset,
                limit: Some(limit),
            })
            .unwrap();
        let titles: Vec<_> = page.tasks.into_iter().map(|t| t.title).collect();
        (titles, page.total)
    };
    assert_eq!(page(0, 2), (vec!["e".to_string(), "d".to_string()], 5));
    assert_eq!(page(4, 2), (vec!["a".to_string()], 5));
}

/// A workspace holding `titles` as tasks.
fn status_fixture(titles: &[&str]) -> (DatabaseContext, i64, Vec<i64>) {
    let db = context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let ids = titles
        .iter()
        .map(|title| {
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
            .id
        })
        .collect();
    (db, workspace_id, ids)
}

#[test]
fn bulk_status_change_returns_only_changed_rows() {
    let (db, workspace_id, ids) = status_fixture(&["a", "b", "c"]);
    db.send_blocking(ChangeTaskStatusCommand {
        workspace_id,
        task_id: ids[0],
        status: TaskStatus::Completed,
    })
    .unwrap();

    let missing = ids[2] + 100;
    let changed = db
        .send_blocking(ChangeTasksStatusCommand {
            workspace_id,
            task_ids: vec![ids[0], ids[1], ids[2], missing],
            status: TaskStatus::Completed,
        })
        .unwrap();

    let changed_ids: Vec<i64> = changed.iter().map(|t| t.id).collect();
    assert_eq!(changed_ids, [ids[1], ids[2]]);
}

#[test]
fn deleted_task_is_not_listed() {
    let (db, workspace_id, ids) = status_fixture(&["a", "b"]);
    db.send_blocking(DeleteTaskCommand {
        workspace_id,
        task_id: ids[0],
    })
    .unwrap();
    let listed = db
        .send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(workspace_id),
            folder: TaskFolderFilter::Any,
            statuses: None,
            metadata: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            has_metadata: None,
            sort: TaskSort::Created,
            descending: false,
        }))
        .unwrap();
    assert_eq!(listed.iter().map(|t| t.id).collect::<Vec<_>>(), [ids[1]]);
}

#[test]
fn sorted_lists_put_undated_tasks_last_either_way() {
    let db = context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    for (title, priority, due_date) in [
        ("undated", TaskPriority::Urgent, None),
        ("late", TaskPriority::None, Some("2026-10-20")),
        ("early", TaskPriority::High, Some("2026-10-01")),
    ] {
        db.send_blocking(CreateTaskCommand {
            workspace_id,
            title: title.to_string(),
            folder_id: None,
            priority: Some(priority),
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: due_date.map(str::to_string),
            tag_ids: Vec::new(),
        })
        .unwrap();
    }
    let titles = |sort, descending| {
        db.send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(workspace_id),
            folder: TaskFolderFilter::Any,
            statuses: None,
            metadata: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            has_metadata: None,
            sort,
            descending,
        }))
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(TaskSort::DueDate, false),
        ["early", "late", "undated"]
    );
    assert_eq!(
        titles(TaskSort::DueDate, true),
        ["late", "early", "undated"]
    );
    assert_eq!(
        titles(TaskSort::Priority, true),
        ["undated", "early", "late"]
    );
}

#[test]
fn sizes_filter_and_sort_with_unsized_tasks_last() {
    let db = context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let size = |number: u8| Some(TaskSize::try_from(number).unwrap());
    for (title, size) in [("unsized", None), ("big", size(5)), ("small", size(2))] {
        db.send_blocking(CreateTaskCommand {
            workspace_id,
            title: title.to_string(),
            folder_id: None,
            priority: None,
            size,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    }
    let titles = |sizes: Option<Vec<TaskSize>>, sort, descending| {
        db.send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(workspace_id),
            folder: TaskFolderFilter::Any,
            statuses: None,
            metadata: None,
            priorities: None,
            sizes,
            start_date: None,
            due_date: None,
            assignee: None,
            has_metadata: None,
            sort,
            descending,
        }))
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(Some(vec![size(5).unwrap()]), TaskSort::Created, false),
        ["big"]
    );
    assert_eq!(
        titles(None, TaskSort::Size, false),
        ["small", "big", "unsized"]
    );
    assert_eq!(
        titles(None, TaskSort::Size, true),
        ["big", "small", "unsized"]
    );
}

#[test]
fn field_changes_raise_events_only_when_the_value_changes() {
    let (db, events) = recording_context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let task_id = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
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
        .id;
    // Sending any command delivers the previous command's events first.
    let flush = || {
        db.send_blocking(GetTaskCommand {
            workspace_id,
            task_id,
        })
        .unwrap();
        events.take()
    };
    flush();

    let title = |title: &str| ChangeTaskTitleCommand {
        workspace_id,
        task_id,
        title: title.to_string(),
    };
    db.send_blocking(title("renamed")).unwrap();
    assert_eq!(
        flush(),
        [DatabaseEventKind::TaskRenamed {
            task_id,
            workspace_id
        }]
    );
    db.send_blocking(title("renamed")).unwrap();
    assert_eq!(flush(), []);

    let priority = |priority| ChangeTaskPriorityCommand {
        workspace_id,
        task_id,
        priority,
    };
    db.send_blocking(priority(TaskPriority::High)).unwrap();
    assert_eq!(
        flush(),
        [DatabaseEventKind::TaskPriorityChanged {
            task_id,
            workspace_id
        }]
    );
    db.send_blocking(priority(TaskPriority::High)).unwrap();
    assert_eq!(flush(), []);

    // Clearing a size is a change too, and clearing it again is not.
    let medium = TaskSize::try_from(3).unwrap();
    for (size, changed) in [
        (Some(medium), true),
        (Some(medium), false),
        (None, true),
        (None, false),
    ] {
        let task = db
            .send_blocking(ChangeTaskSizeCommand {
                workspace_id,
                task_id,
                size,
            })
            .unwrap();
        assert_eq!(task.size, size);
        let expected = match changed {
            true => vec![DatabaseEventKind::TaskSizeChanged {
                task_id,
                workspace_id,
            }],
            false => vec![],
        };
        assert_eq!(flush(), expected);
    }

    let description = |description: &str| ChangeTaskDescriptionCommand {
        workspace_id,
        task_id,
        description: description.to_string(),
    };
    db.send_blocking(description("notes")).unwrap();
    assert_eq!(
        flush(),
        [DatabaseEventKind::TaskDescriptionChanged {
            task_id,
            workspace_id
        }]
    );
    db.send_blocking(description("notes")).unwrap();
    assert_eq!(flush(), []);

    let schedule = |start: Option<&str>, due: Option<&str>| ScheduleTaskCommand {
        workspace_id,
        task_id,
        start_date: start.map(str::to_string),
        due_date: due.map(str::to_string),
    };
    db.send_blocking(schedule(Some("2026-10-13"), Some("2026-10-20")))
        .unwrap();
    assert_eq!(
        flush(),
        [
            DatabaseEventKind::TaskStartDateChanged {
                task_id,
                workspace_id
            },
            DatabaseEventKind::TaskDueDateChanged {
                task_id,
                workspace_id
            }
        ]
    );
    db.send_blocking(schedule(Some("2026-10-13"), None))
        .unwrap();
    assert_eq!(
        flush(),
        [DatabaseEventKind::TaskDueDateChanged {
            task_id,
            workspace_id
        }]
    );
    db.send_blocking(schedule(Some("2026-10-13"), None))
        .unwrap();
    assert_eq!(flush(), []);
}

#[test]
fn deleting_a_task_raises_it_as_it_was_deleted() {
    let (db, events) = recording_context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let task_id = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
            title: "gone".to_string(),
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
        .id;
    events.take();
    db.send_blocking(DeleteTaskCommand {
        workspace_id,
        task_id,
    })
    .unwrap();
    let raised = events.take();
    let [DatabaseEventKind::TaskDeleted { task }] = raised.as_slice() else {
        panic!("deleting a task raises TaskDeleted alone");
    };
    assert_eq!(
        (task.id, task.title.as_str(), task.deleted),
        (task_id, "gone", true)
    );
}

#[test]
fn a_task_is_assigned_to_the_user_or_one_agent_until_the_agent_is_deleted() {
    let (db, events) = recording_context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let task = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
            title: "t".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: Some(TaskAssignee::User),
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    assert_eq!(task.assignee, Some(TaskAssignee::User));
    let task_id = task.id;
    let flush = || {
        db.send_blocking(GetTaskCommand {
            workspace_id,
            task_id,
        })
        .unwrap();
        events.take()
    };
    flush();

    // Each real change raises the event, clearing included; setting the
    // same assignee again raises nothing.
    let agent = TaskAssignee::Agent { agent_id };
    for (assignee, changed) in [
        (Some(TaskAssignee::User), false),
        (Some(agent), true),
        (Some(agent), false),
        (None, true),
        (None, false),
        (Some(agent), true),
    ] {
        let task = db
            .send_blocking(ChangeTaskAssigneeCommand {
                workspace_id,
                task_id,
                assignee,
            })
            .unwrap();
        assert_eq!(task.assignee, assignee);
        let expected = match changed {
            true => vec![DatabaseEventKind::TaskAssigneeChanged {
                task_id,
                workspace_id,
            }],
            false => vec![],
        };
        assert_eq!(flush(), expected, "{assignee:?}");
    }

    // An agent that doesn't exist is refused, and the task keeps its
    // assignee.
    let missing = db.send_blocking(ChangeTaskAssigneeCommand {
        workspace_id,
        task_id,
        assignee: Some(TaskAssignee::Agent {
            agent_id: agent_id + 1,
        }),
    });
    assert!(missing.is_err());

    // Deleting the agent leaves the task unassigned.
    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id })
        .unwrap();
    let task = db
        .send_blocking(GetTaskCommand {
            workspace_id,
            task_id,
        })
        .unwrap();
    assert_eq!(task.assignee, None);
}

#[test]
fn a_task_is_assigned_only_to_a_live_contact_of_its_workspace() {
    let db = context();
    let [home, work] = ["home", "work"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
        .id
    });
    let [ada, bob] = [(home, "Ada"), (work, "Bob")].map(|(workspace_id, first_name)| {
        db.send_blocking(CreateContactCommand {
            workspace_id,
            first_name: first_name.to_string(),
            middle_name: None,
            last_name: "Lee".to_string(),
            notes: String::new(),
            methods: Vec::new(),
        })
        .unwrap()
        .id
    });
    let task_id = db
        .send_blocking(CreateTaskCommand {
            workspace_id: home,
            title: "t".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: Some(TaskAssignee::Contact { contact_id: ada }),
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap()
        .id;
    let assign = |contact_id| {
        db.send_blocking(ChangeTaskAssigneeCommand {
            workspace_id: home,
            task_id,
            assignee: Some(TaskAssignee::Contact { contact_id }),
        })
    };
    let assignee = || {
        db.send_blocking(GetTaskCommand {
            workspace_id: home,
            task_id,
        })
        .unwrap()
        .assignee
    };
    assert_eq!(assignee(), Some(TaskAssignee::Contact { contact_id: ada }));

    // Another workspace's contact is refused, and the task keeps its
    // assignee.
    assert!(assign(bob).is_err());
    assert_eq!(assignee(), Some(TaskAssignee::Contact { contact_id: ada }));

    // Deleting the contact leaves the task unassigned, and a deleted
    // contact can't be assigned again.
    db.send_blocking(DeleteContactCommand {
        workspace_id: home,
        contact_id: ada,
    })
    .unwrap();
    assert_eq!(assignee(), None);
    assert!(assign(ada).is_err());
}

#[test]
fn a_listing_keeps_only_the_tasks_of_one_assignee() {
    let db = context();
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let agent = TaskAssignee::Agent { agent_id };
    for (title, assignee) in [
        ("mine", Some(TaskAssignee::User)),
        ("scout's", Some(agent)),
        ("no one's", None),
    ] {
        db.send_blocking(CreateTaskCommand {
            workspace_id,
            title: title.to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    }
    let titles = |assignee| {
        db.send_blocking(ListTasksCommand(TaskListing {
            tags: TagFilter::default(),
            ids: None,
            workspace_id: Some(workspace_id),
            folder: TaskFolderFilter::Any,
            statuses: None,
            metadata: None,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee,
            has_metadata: None,
            sort: TaskSort::Created,
            descending: false,
        }))
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect::<Vec<_>>()
    };
    assert_eq!(titles(Some(TaskAssignee::User)), ["mine"]);
    assert_eq!(titles(Some(agent)), ["scout's"]);
    assert_eq!(titles(None), ["mine", "scout's", "no one's"]);
}
