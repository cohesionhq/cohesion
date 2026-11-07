use cohesion_core::model::{
    ActorSessionStop, AgentAsk, AgentAskReply, AgentChoice, AgentCodeExecution, AgentCodeOutcome,
    AgentForwardedItem, AgentForwardedItemKind, AgentMemoryChange, AgentMessageContent, AgentPeer,
    AgentRunPart, AgentWake, ContextRebuildReason, HookKind, IntelligenceAgentColor,
    IntelligenceAgentMessageSender,
};

use cohesion_core::protocol::{AgentForwardTarget, ImageUpload};

use crate::test_support::{context, recording_context};
use crate::{
    CommandError, CreateAgentForkRunCommand, CreateAgentMessageCommand,
    CreateAgentMessageWithImagesCommand, CreateDocumentCommand, CreateIntelligenceAgentCommand,
    CreateTaskCommand, CreateUserAgentMessageCommand, CreateWorkspaceCommand, DatabaseContext,
    DatabaseEventKind, DeleteIntelligenceAgentCommand, DeleteTaskCommand, EnsureJournalCommand,
    GetAgentMessageCommand, GetIntelligenceAgentCommand, ListAgentMessageAttachmentsCommand,
    ListAgentMessagesCommand, MarkAgentReadCommand, PickAgentChoiceCommand,
    ReactToUserMessageCommand, SetAgentMessageReactionCommand,
};

/// An agent with messages `"0"` to `"<count - 1>"`, returning its id and the
/// messages' ids.
fn agent_with_messages(db: &DatabaseContext, count: usize) -> (i64, Vec<i64>) {
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
    let ids = (0..count)
        .map(|index| {
            db.send_blocking(CreateAgentMessageCommand {
                agent_id: agent.id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::User,
                content: text(&index.to_string()),
                reply_to: None,
            })
            .unwrap()
            .id
        })
        .collect();
    (agent.id, ids)
}

fn text(text: &str) -> AgentMessageContent {
    AgentMessageContent::Text {
        text: text.to_string(),
    }
}

fn contents(messages: Vec<cohesion_core::model::IntelligenceAgentMessage>) -> Vec<String> {
    messages
        .into_iter()
        .map(|message| match message.message_content {
            AgentMessageContent::Text { text } => text,
            other => panic!("not text: {other:?}"),
        })
        .collect()
}

#[test]
fn messages_page_backwards_oldest_first() {
    let db = context();
    let (agent_id, ids) = agent_with_messages(&db, 5);

    let newest = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: None,
            limit: 2,
        })
        .unwrap();
    assert_eq!(contents(newest), ["3", "4"]);

    let earlier = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: Some(ids[3]),
            limit: 2,
        })
        .unwrap();
    assert_eq!(contents(earlier), ["1", "2"]);
}

#[test]
fn every_kind_has_a_one_line_excerpt() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let verb_and_excerpt = |content: AgentMessageContent| {
        let message = db
            .send_blocking(CreateAgentMessageCommand {
                agent_id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content,
                reply_to: None,
            })
            .unwrap();
        (message.excerpt_verb, message.excerpt)
    };
    let excerpt = |content| verb_and_excerpt(content).1;
    assert_eq!(
        excerpt(text("**Done.**\nThe `fence`\tis __up__.")),
        "Done. The fence is up."
    );
    assert_eq!(
        excerpt(text(&"a".repeat(250))),
        format!("{}…", "a".repeat(200))
    );
    assert_eq!(
        excerpt(AgentMessageContent::Choice(AgentChoice {
            question: "When?".to_string(),
            options: vec!["Today".to_string()],
            context: None,
            selected: None,
        })),
        "When?"
    );
    assert_eq!(
        excerpt(AgentMessageContent::CodeExecution(AgentCodeExecution {
            title: "Count tasks".to_string(),
            code: "1".to_string(),
            outcome: AgentCodeOutcome::Cancelled,
        })),
        "Count tasks"
    );
    assert_eq!(
        excerpt(AgentMessageContent::Documentation {
            module: "network".to_string()
        }),
        "Learning about network"
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Memory {
            memory: "Jo likes tea".to_string(),
            change: AgentMemoryChange::Remembered,
        }),
        (Some("Remembered".to_string()), "Jo likes tea".to_string())
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Memory {
            memory: "Jo likes tea".to_string(),
            change: AgentMemoryChange::Forgotten,
        }),
        (Some("Forgot".to_string()), "Jo likes tea".to_string())
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Note {
            note: "Task 536 dispatched".to_string()
        }),
        (Some("Noted".to_string()), "Task 536 dispatched".to_string())
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Woke {
            by: AgentWake::ActorSession {
                actor_session_id: 264,
                title: "Task dates drive hooks".to_string(),
                stop: ActorSessionStop::Finished,
            }
        }),
        (
            Some("Woke".to_string()),
            "Actor session 264 stopped".to_string()
        )
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Woke {
            by: AgentWake::Cue {
                cue_name: "Morning".to_string(),
                hook: HookKind::Launch,
            }
        }),
        (Some("Woke".to_string()), "Cue “Morning” fired".to_string())
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::Woke {
            by: AgentWake::AssignedTask {
                workspace_id: 1,
                task_id: 603,
                title: "Fix the build".to_string(),
            }
        }),
        (
            Some("Woke".to_string()),
            "“Fix the build” was assigned to you".to_string()
        )
    );
    assert_eq!(
        verb_and_excerpt(AgentMessageContent::RebuiltContext {
            reason: Some(ContextRebuildReason::CacheExpired)
        }),
        (
            None,
            "Context rebuilt because provider cache expired".to_string()
        )
    );
    assert_eq!(verb_and_excerpt(text("Done.")), (None, "Done.".to_string()));
    assert_eq!(
        excerpt(AgentMessageContent::Error {
            error: "Timed out".to_string()
        }),
        "Timed out"
    );
    let item = AgentForwardedItem {
        kind: AgentForwardedItemKind::Task,
        workspace_id: 1,
        id: 1,
        title: "Fix login".to_string(),
    };
    assert_eq!(
        excerpt(AgentMessageContent::Forward {
            note: String::new(),
            item: item.clone(),
        }),
        "Fix login"
    );
    assert_eq!(
        excerpt(AgentMessageContent::Forward {
            note: "Look".to_string(),
            item,
        }),
        "Look"
    );
    let ask = AgentAsk {
        agent: AgentPeer {
            id: 2,
            name: "Ada".to_string(),
        },
        question: "Is the build green?".to_string(),
        reply: AgentAskReply::Waiting,
    };
    assert_eq!(
        excerpt(AgentMessageContent::AskedAgent(ask.clone())),
        "Asked Ada: Is the build green?"
    );
    assert_eq!(
        excerpt(AgentMessageContent::AskedByAgent(ask)),
        "Ada asked: Is the build green?"
    );

    let (images, _) = db
        .send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: String::new(),
            attachments: vec![image("aGk="), image("aGV5")],
            forward: None,
        })
        .unwrap();
    assert_eq!(images.excerpt, "2 images");
    assert_eq!(images.reaction_excerpt, None);

    let (long, _) = db
        .send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: "a".repeat(150),
            attachments: Vec::new(),
            forward: None,
        })
        .unwrap();
    let reacted = db
        .send_blocking(ReactToUserMessageCommand {
            agent_id,
            message_id: long.id,
            reaction: Some("👍".to_string()),
        })
        .unwrap();
    assert_eq!(
        (reacted.reaction_excerpt_verb, reacted.reaction_excerpt),
        (
            Some("Reacted".to_string()),
            Some(format!("👍 to “{}…”", "a".repeat(140)))
        )
    );
}

#[test]
fn a_forward_carries_the_live_item_it_names() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "Home".to_string(),
            emoji: None,
        })
        .unwrap();
    let task = db
        .send_blocking(CreateTaskCommand {
            workspace_id: workspace.id,
            title: "Fix login".to_string(),
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
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: workspace.id,
            title: "Plans".to_string(),
            contents: String::new(),
        })
        .unwrap();
    let journal = db
        .send_blocking(EnsureJournalCommand {
            workspace_id: workspace.id,
            date: "2026-10-20".to_string(),
        })
        .unwrap();
    let forward = |note: &str, target| {
        db.send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: note.to_string(),
            attachments: Vec::new(),
            forward: Some(target),
        })
        .map(|(message, _)| message.message_content)
    };
    let item = |content| match content {
        AgentMessageContent::Forward { note, item } => (note, item.kind, item.id, item.title),
        other => panic!("not a forward: {other:?}"),
    };

    let sent = forward(
        " Look ",
        AgentForwardTarget::Task {
            workspace_id: workspace.id,
            id: task.id,
        },
    );
    assert_eq!(
        item(sent.unwrap()),
        (
            "Look".to_string(),
            AgentForwardedItemKind::Task,
            task.id,
            "Fix login".to_string()
        )
    );
    let sent = forward(
        "",
        AgentForwardTarget::Document {
            workspace_id: workspace.id,
            id: document.id,
        },
    );
    assert_eq!(item(sent.unwrap()).1, AgentForwardedItemKind::Document);
    let sent = forward(
        "",
        AgentForwardTarget::Document {
            workspace_id: workspace.id,
            id: journal.id,
        },
    );
    assert_eq!(
        item(sent.unwrap()),
        (
            String::new(),
            AgentForwardedItemKind::Journal,
            journal.id,
            "2026-10-20".to_string()
        )
    );

    db.send_blocking(DeleteTaskCommand {
        workspace_id: workspace.id,
        task_id: task.id,
    })
    .unwrap();
    let gone = forward(
        "Look",
        AgentForwardTarget::Task {
            workspace_id: workspace.id,
            id: task.id,
        },
    );
    assert!(matches!(gone, Err(CommandError::NotFound(_))));
    let elsewhere = forward(
        "Look",
        AgentForwardTarget::Document {
            workspace_id: workspace.id + 1,
            id: document.id,
        },
    );
    assert!(matches!(elsewhere, Err(CommandError::NotFound(_))));
}

#[test]
fn the_agent_reacts_to_a_forward() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "Home".to_string(),
            emoji: None,
        })
        .unwrap();
    let document = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: workspace.id,
            title: "Plans".to_string(),
            contents: String::new(),
        })
        .unwrap();
    let (forward, _) = db
        .send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: String::new(),
            attachments: Vec::new(),
            forward: Some(AgentForwardTarget::Document {
                workspace_id: workspace.id,
                id: document.id,
            }),
        })
        .unwrap();
    let reacted = db
        .send_blocking(ReactToUserMessageCommand {
            agent_id,
            message_id: forward.id,
            reaction: Some("👍".to_string()),
        })
        .unwrap();
    assert_eq!(reacted.reaction.as_deref(), Some("👍"));
}

#[test]
fn only_text_messages_and_forwards_are_reacted_to() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let lookup = db
        .send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::Documentation {
                module: "network".to_string(),
            },
            reply_to: None,
        })
        .unwrap();

    let react = |reaction: Option<&str>| {
        db.send_blocking(SetAgentMessageReactionCommand {
            message_id: lookup.id,
            reaction: reaction.map(str::to_string),
        })
    };
    assert!(matches!(
        react(Some("👍")),
        Err(CommandError::Validation(_))
    ));
    assert_eq!(react(None).unwrap().reaction, None);
}

#[test]
fn agent_messages_mark_the_agent_unread_until_read_or_answered() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 1);
    let unread = || {
        db.send_blocking(GetIntelligenceAgentCommand { agent_id })
            .unwrap()
            .agent_unread
    };
    let send = |sender| {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender,
            content: text("hi"),
            reply_to: None,
        })
        .unwrap();
    };
    assert!(!unread());

    send(IntelligenceAgentMessageSender::BackgroundAgent);
    assert!(unread());
    db.send_blocking(MarkAgentReadCommand { agent_id }).unwrap();
    assert!(!unread());

    send(IntelligenceAgentMessageSender::Agent);
    assert!(unread());
    send(IntelligenceAgentMessageSender::User);
    assert!(!unread());
}

#[test]
fn code_runs_and_lookups_leave_the_unread_mark_alone() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 1);
    let unread = || {
        db.send_blocking(GetIntelligenceAgentCommand { agent_id })
            .unwrap()
            .agent_unread
    };
    let send = |content| {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::BackgroundAgent,
            content,
            reply_to: None,
        })
        .unwrap();
    };

    send(AgentMessageContent::CodeExecution(AgentCodeExecution {
        title: "Check".to_string(),
        code: "1".to_string(),
        outcome: AgentCodeOutcome::Succeeded {
            result: "1".to_string(),
        },
    }));
    assert!(!unread());
    send(AgentMessageContent::Documentation {
        module: "network".to_string(),
    });
    assert!(!unread());
    send(text("Checked."));
    assert!(unread());
    send(AgentMessageContent::Documentation {
        module: "network".to_string(),
    });
    assert!(unread());
}

#[test]
fn every_new_message_raises_an_event() {
    let (db, events) = recording_context();
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: None,
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
    let send = |sender, content| {
        let message_id = db
            .send_blocking(CreateAgentMessageCommand {
                agent_id,
                run_id: None,
                run_description: None,
                sender,
                content,
                reply_to: None,
            })
            .unwrap()
            .id;
        // Sending any command delivers the previous command's events first.
        db.send_blocking(GetAgentMessageCommand { message_id })
            .unwrap();
        (message_id, events.take())
    };
    events.take();

    use IntelligenceAgentMessageSender::{Agent, BackgroundAgent, User};
    for (sender, content) in [
        (User, text("hi")),
        (Agent, text("hi")),
        (BackgroundAgent, text("hi")),
    ] {
        let (message_id, raised) = send(sender, content);
        assert_eq!(
            raised,
            [DatabaseEventKind::AgentMessageCreated { message_id }]
        );
    }
}

fn image(data: &str) -> ImageUpload {
    ImageUpload {
        media_type: "image/png".to_string(),
        data: data.to_string(),
    }
}

#[test]
fn attachments_are_counted_listed_and_deleted_with_their_agent() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let (message, attachments) = db
        .send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: String::new(),
            attachments: vec![image("aGk="), image("aGV5")],
            forward: None,
        })
        .unwrap();
    assert_eq!(message.attachment_count, 2);
    let listed = db
        .send_blocking(ListAgentMessageAttachmentsCommand {
            message_id: message.id,
        })
        .unwrap();
    let data: Vec<_> = listed.iter().map(|a| a.data.to_base64()).collect();
    assert_eq!(data, ["aGk=", "aGV5"]);
    assert_eq!(listed[0].id, attachments[0].id);

    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id })
        .unwrap();
    let left = db
        .send_blocking(ListAgentMessageAttachmentsCommand {
            message_id: message.id,
        })
        .unwrap();
    assert!(left.is_empty());
}

#[test]
fn the_agents_images_are_checked_counted_and_listed_like_the_users() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let send = |attachments| {
        db.send_blocking(CreateAgentMessageWithImagesCommand {
            message: CreateAgentMessageCommand {
                agent_id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content: text("Here's the screenshot"),
                reply_to: None,
            },
            attachments,
        })
    };
    let message = send(vec![image("aGk="), image("aGV5")]).unwrap();
    assert_eq!(message.attachment_count, 2);
    let listed = db
        .send_blocking(ListAgentMessageAttachmentsCommand {
            message_id: message.id,
        })
        .unwrap();
    let data: Vec<_> = listed.iter().map(|a| a.data.to_base64()).collect();
    assert_eq!(data, ["aGk=", "aGV5"]);
    assert_eq!(send(Vec::new()).unwrap().attachment_count, 0);

    // An image the user couldn't attach refuses the whole message.
    let before = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: None,
            limit: 10,
        })
        .unwrap()
        .len();
    assert!(matches!(
        send(vec![image("aGk="), image("not base64")]),
        Err(CommandError::Validation(_))
    ));
    let after = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: None,
            limit: 10,
        })
        .unwrap()
        .len();
    assert_eq!(before, after);
}

#[test]
fn a_message_needs_text_or_valid_images() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let send = |text: &str, attachments| {
        db.send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: text.to_string(),
            attachments,
            forward: None,
        })
    };
    assert!(matches!(
        send(" ", vec![]),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        send("hi", vec![image("not base64!")]),
        Err(CommandError::Validation(_))
    ));
    let mut pdf = image("aGk=");
    pdf.media_type = "application/pdf".to_string();
    assert!(matches!(
        send("hi", vec![pdf]),
        Err(CommandError::Validation(_))
    ));
    // A rejected message leaves nothing behind.
    let messages = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: None,
            limit: 10,
        })
        .unwrap();
    assert!(messages.is_empty());
}

#[test]
fn each_side_reacts_only_to_the_others_messages() {
    let db = context();
    let (agent_id, ids) = agent_with_messages(&db, 1);
    let reply = db
        .send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: text("Done."),
            reply_to: None,
        })
        .unwrap();

    let user_reacts = |message_id, reaction: Option<&str>| {
        db.send_blocking(SetAgentMessageReactionCommand {
            message_id,
            reaction: reaction.map(str::to_string),
        })
    };
    assert!(matches!(
        user_reacts(ids[0], Some("👍")),
        Err(CommandError::Validation(_))
    ));
    let reacted = user_reacts(reply.id, Some(" 👍 ")).unwrap();
    assert_eq!(reacted.reaction.as_deref(), Some("👍"));
    assert!(reacted.reacted_at.is_some());

    let agent_reacts = |agent_id, message_id| {
        db.send_blocking(ReactToUserMessageCommand {
            agent_id,
            message_id,
            reaction: Some("❤️".to_string()),
        })
    };
    assert!(matches!(
        agent_reacts(agent_id, reply.id),
        Err(CommandError::Validation(_))
    ));
    let (other_agent, _) = agent_with_messages(&db, 0);
    assert!(matches!(
        agent_reacts(other_agent, ids[0]),
        Err(CommandError::NotFound(_))
    ));
    assert_eq!(
        agent_reacts(agent_id, ids[0]).unwrap().reaction.as_deref(),
        Some("❤️")
    );

    // Any single emoji goes, not only those clients offer.
    assert_eq!(
        user_reacts(reply.id, Some("🦀"))
            .unwrap()
            .reaction
            .as_deref(),
        Some("🦀")
    );
    let cleared = user_reacts(reply.id, Some(" ")).unwrap();
    assert_eq!(cleared.reaction, None);
}

#[test]
fn reacting_raises_an_event_and_clearing_does_not() {
    let (db, events) = recording_context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let reply = db
        .send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: text("Done."),
            reply_to: None,
        })
        .unwrap();
    let react = |reaction: Option<&str>| {
        db.send_blocking(SetAgentMessageReactionCommand {
            message_id: reply.id,
            reaction: reaction.map(str::to_string),
        })
        .unwrap();
        // Sending any command delivers the previous command's events first.
        db.send_blocking(GetAgentMessageCommand {
            message_id: reply.id,
        })
        .unwrap();
        events.take()
    };
    events.take();
    assert_eq!(
        react(Some("🎉")),
        [DatabaseEventKind::AgentMessageReacted {
            message_id: reply.id
        }]
    );
    assert!(react(None).is_empty());
}

#[test]
fn a_choice_is_picked_once_from_its_options() {
    let db = context();
    let (agent_id, ids) = agent_with_messages(&db, 1);
    let choice = db
        .send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::Choice(AgentChoice {
                question: "When?".to_string(),
                options: vec!["Today".to_string(), "Tomorrow".to_string()],
                context: None,
                selected: None,
            }),
            reply_to: None,
        })
        .unwrap();
    let pick = |message_id, option| db.send_blocking(PickAgentChoiceCommand { message_id, option });

    assert!(matches!(pick(ids[0], 0), Err(CommandError::Validation(_))));
    assert!(matches!(
        pick(choice.id, 2),
        Err(CommandError::Validation(_))
    ));
    let picked = pick(choice.id, 1).unwrap();
    let AgentMessageContent::Choice(picked) = picked.message_content else {
        panic!("a choice stays a choice");
    };
    assert_eq!(picked.selected, Some(1));
    assert!(matches!(
        pick(choice.id, 0),
        Err(CommandError::Validation(_))
    ));
}

#[test]
fn a_reply_answers_a_message_of_its_own_conversation() {
    let db = context();
    let (agent, ids) = agent_with_messages(&db, 1);
    let (other, other_ids) = agent_with_messages(&db, 1);
    let reply = |agent_id, reply_to| {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: text("On it."),
            reply_to: Some(reply_to),
        })
    };

    assert_eq!(reply(agent, ids[0]).unwrap().reply_to, Some(ids[0]));
    assert!(reply(agent, other_ids[0]).is_err());
    assert!(reply(other, other_ids[0] + 100).is_err());
}

#[test]
fn bookkeeping_leaves_the_agent_read_and_raises_no_hook() {
    let (db, events) = recording_context();
    let (agent_id, _) = agent_with_messages(&db, 1);
    db.send_blocking(MarkAgentReadCommand { agent_id }).unwrap();
    events.take();
    for content in [
        AgentMessageContent::Note {
            note: "Waiting on the brief".to_string(),
        },
        AgentMessageContent::Woke {
            by: AgentWake::Cue {
                cue_name: "Nightly".to_string(),
                hook: HookKind::Timer,
            },
        },
        AgentMessageContent::RebuiltContext {
            reason: Some(ContextRebuildReason::ModelChanged),
        },
    ] {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content,
            reply_to: None,
        })
        .unwrap();
    }
    let agent = db
        .send_blocking(GetIntelligenceAgentCommand { agent_id })
        .unwrap();
    assert!(!agent.agent_unread);
    assert!(
        !events
            .take()
            .iter()
            .any(|event| matches!(event, DatabaseEventKind::AgentMessageCreated { .. }))
    );
}

/// Every kind a fork posts lands on the side of its collapse that a client
/// shows it on, and a message of the conversation is no part of any fork.
#[test]
fn a_forks_messages_say_what_they_are_to_it() {
    let db = context();
    let (agent_id, _) = agent_with_messages(&db, 0);
    let run = db
        .send_blocking(CreateAgentForkRunCommand {
            agent_id,
            hook: HookKind::Timer,
        })
        .unwrap();
    let post = |run_id, content| {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content,
            reply_to: None,
        })
        .unwrap()
        .run_part
    };
    let ask = || AgentAsk {
        agent: AgentPeer {
            id: 2,
            name: "Inbox keeper".to_string(),
        },
        question: "Where?".to_string(),
        reply: AgentAskReply::Waiting,
    };
    let forked = |content| post(Some(run.id), content);

    for trigger in [
        AgentMessageContent::Woke {
            by: AgentWake::Cue {
                cue_name: "Morning".to_string(),
                hook: HookKind::Timer,
            },
        },
        AgentMessageContent::AskedByAgent(ask()),
    ] {
        assert_eq!(forked(trigger), Some(AgentRunPart::Trigger));
    }
    for step in [
        AgentMessageContent::CodeExecution(AgentCodeExecution {
            title: "Read".to_string(),
            code: "1".to_string(),
            outcome: AgentCodeOutcome::Cancelled,
        }),
        AgentMessageContent::Documentation {
            module: "network".to_string(),
        },
        AgentMessageContent::Memory {
            memory: "Jo".to_string(),
            change: AgentMemoryChange::Remembered,
        },
        AgentMessageContent::Note {
            note: "Later".to_string(),
        },
        AgentMessageContent::AskedAgent(ask()),
    ] {
        assert_eq!(forked(step), Some(AgentRunPart::Step));
    }
    for surfaced in [
        text("Done."),
        AgentMessageContent::Choice(AgentChoice {
            question: "When?".to_string(),
            options: vec!["Today".to_string()],
            context: None,
            selected: None,
        }),
        AgentMessageContent::Error {
            error: "It failed".to_string(),
        },
    ] {
        assert_eq!(forked(surfaced), Some(AgentRunPart::Surfaced));
    }
    assert_eq!(post(None, text("Done.")), None);
}
