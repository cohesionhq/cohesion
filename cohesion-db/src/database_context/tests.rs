use cohesion_core::Named;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    AgentMessageContent, IntelligenceAgentColor, IntelligenceAgentMessageSender,
};
use cohesion_core::{TaskStatus, Workspace};

use super::unix_now;
use crate::test_support::{context, recording_context};
use crate::{
    ChangeTasksStatusCommand, Changed, CommandContext, CommandError, Container,
    CreateAgentMessageCommand, CreateIntelligenceAgentCommand, CreateTaskCommand,
    CreateWorkspaceCommand, DatabaseCommand, DatabaseCommandResult, DatabaseContext, DatabaseError,
    GetIntelligenceAgentCommand, ListWorkspacesCommand, Secrets, SetAgentActivityCommand, Write,
};

/// Creates a workspace inline, whose keys `ctx.run` keeps, then fails.
struct InsertThenFail;
impl DatabaseCommand for InsertThenFail {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.run(CreateWorkspaceCommand {
            title: "doomed".to_string(),
            emoji: None,
        })?;
        Err(CommandError::NotFound(Workspace::NOUN))
    }
}

#[test]
fn failed_command_rolls_back_and_publishes_nothing() {
    let db = context();
    let mut subscriber = db.subscribe();
    assert!(matches!(
        db.send_blocking(InsertThenFail),
        Err(CommandError::NotFound(Workspace::NOUN))
    ));
    assert!(
        db.send_blocking(ListWorkspacesCommand::default())
            .unwrap()
            .is_empty()
    );

    // The next batch a subscriber sees is from a later, successful command.
    let created = db
        .send_blocking(CreateWorkspaceCommand {
            title: "kept".to_string(),
            emoji: None,
        })
        .unwrap();
    let batch = subscriber.blocking_recv().unwrap();
    assert_eq!(batch.keys, [Invalidation::Workspace(created.id)]);
}

#[test]
fn successful_command_publishes_invalidations() {
    let db = context();
    let mut subscriber = db.subscribe();
    let created = db
        .send_blocking(CreateWorkspaceCommand {
            title: "new".to_string(),
            emoji: None,
        })
        .unwrap();
    let batch = subscriber.blocking_recv().unwrap();
    assert_eq!(batch.keys, [Invalidation::Workspace(created.id)]);
    let listed = db.send_blocking(ListWorkspacesCommand::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "new");
}

#[test]
fn agent_message_publishes_its_agent() {
    let db = context();
    let agent = db
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
        .unwrap();
    let mut subscriber = db.subscribe();
    db.send_blocking(CreateAgentMessageCommand {
        agent_id: agent.id,
        run_id: None,
        run_description: None,
        sender: IntelligenceAgentMessageSender::User,
        content: AgentMessageContent::Text {
            text: "hello".to_string(),
        },
        reply_to: None,
    })
    .unwrap();
    let batch = subscriber.blocking_recv().unwrap();
    assert_eq!(batch.keys, [Invalidation::Agent(agent.id)]);
}

#[test]
fn task_write_publishes_its_list() {
    let db = context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    let task = db
        .send_blocking(CreateTaskCommand {
            workspace_id: workspace.id,
            title: "laundry".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    let mut subscriber = db.subscribe();
    db.send_blocking(ChangeTasksStatusCommand {
        workspace_id: workspace.id,
        task_ids: vec![task.id],
        status: TaskStatus::Completed,
    })
    .unwrap();
    let batch = subscriber.blocking_recv().unwrap();
    assert_eq!(
        batch.keys,
        [Invalidation::TaskList {
            workspace_id: workspace.id,
            folder_id: None,
        }]
    );
}

/// A shutdown waits for the hooks a request queued, so they must be queued
/// before the request hears back.
#[test]
fn events_are_delivered_before_the_reply() {
    let (db, events) = recording_context();
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    db.send_blocking(CreateTaskCommand {
        workspace_id: workspace.id,
        title: "laundry".to_string(),
        folder_id: None,
        priority: None,
        size: None,
        assignee: None,
        description: None,
        start_date: None,
        due_date: None,
        tag_ids: Vec::new(),
    })
    .unwrap();
    assert!(!events.take().is_empty());
}

#[test]
fn close_refuses_later_commands() {
    let db = context();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(db.close());
    assert!(matches!(
        db.send_blocking(ListWorkspacesCommand::default()),
        Err(CommandError::Database(DatabaseError::Closed))
    ));
}

#[test]
fn close_leaves_no_work_marked_running() {
    let path = std::env::temp_dir().join(format!(
        "cohesion-close-{}-{}.db",
        std::process::id(),
        unix_now()
    ));
    let open = || {
        let mut container = Container::open(&path).unwrap();
        container.migrate().unwrap();
        DatabaseContext::new(container, Secrets::new(), ())
    };
    let db = open();
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Teal,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap();
    db.send_blocking(SetAgentActivityCommand {
        agent_id: agent.id,
        activity: Some("Thinking".to_string()),
    })
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(db.close());

    let reopened = open();
    let agent = reopened
        .send_blocking(GetIntelligenceAgentCommand { agent_id: agent.id })
        .unwrap();
    runtime.block_on(reopened.close());
    let _ = std::fs::remove_file(&path);
    assert_eq!(agent.activity, None);
}
