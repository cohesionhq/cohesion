use std::sync::Arc;

use cohesion_core::LocalIdentity;
use cohesion_core::model::{
    ActorDefinition, ActorSession, ActorSessionStatus, ActorSessionStop, AgentAskReply,
    AgentMessageContent, AgentWake, ContextRebuildReason, HookKind, IntelligenceAgentColor,
    IntelligenceAgentMessage, IntelligenceAgentMessageSender, IntelligenceAgentRunOutcome,
    IntelligenceAgentRunState, IntelligenceTier, MemoryScope,
};
use cohesion_core::protocol::ClientCapabilities;
use cohesion_db::{
    ASK_STOPPED_REASON, Container, CreateActorSessionCommand, CreateAgentMessageCommand,
    CreateIntelligenceAgentCommand, CreateMemoryCommand, DatabaseContext, DatabaseEventTrace,
    GetIntelligenceAgentCommand, GetStorageSettingsCommand, IdentifyLocalClientCommand,
    ListActorsCommand, ListAgentMessagesCommand, ListAgentRunsCommand,
    LookupAgentThreadMarkByAgentCommand, Secrets, SetAgentThreadMarkCommand, SetProfileBioCommand,
    SetProfileNameCommand, SyncCapabilitiesCommand, UpdateStorageSettingsCommand, WorkTracker,
};
use cohesion_js::ScriptEngine;
use serde_json::{Value, json};

use crate::activity::{Activity, ShowActivity, ShowFuture};
use crate::agents::agent_messenger::{ASK_LIMIT, AgentMessenger, Ask};
use crate::agents::conversation_mark::{ConversationMarks, ThreadMark};
use crate::agents::conversation_session;
use crate::agents::foreground_turn::ForegroundTurn;
use crate::session::{Session, tier_model};
use crate::system_prompt::{AboutUser, Profile, Roster, SystemPrompt};
use crate::test_model::{Answer, TestModel, calling_with, text, texts};
use crate::transcript_item::{Prompt, TranscriptItem};

struct Unshown;
impl ShowActivity for Unshown {
    fn show(&self, _activity: Activity) -> ShowFuture<'_> {
        Box::pin(async {})
    }
}

/// Two agents, Lead and App, whose sessions run on `model` as the `agent`
/// tier's default.
struct Team {
    db: DatabaseContext,
    model: Arc<TestModel>,
    messenger: AgentMessenger,
    marks: ConversationMarks,
    work: WorkTracker,
    lead: i64,
    app: i64,
}

fn team(answers: Vec<Answer>) -> Team {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(answers);
    model.store(&db, IntelligenceTier::Agent);
    let agent = |name: &str| {
        db.send_blocking(CreateIntelligenceAgentCommand {
            name: Some(name.to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id
    };
    let (lead, app) = (agent("Lead"), agent("App"));

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let messenger = AgentMessenger::new(db.clone(), engine.clone(), model.clone(), work.clone());
    let marks = messenger.marks().clone();
    Team {
        db,
        model,
        messenger,
        marks,
        work,
        lead,
        app,
    }
}

impl Team {
    /// Lead's question to App, from Lead's conversation.
    fn ask(&self, question: &str) -> Ask {
        Ask {
            asker_id: self.lead,
            asker_run_id: None,
            recipient_id: self.app,
            question: question.to_string(),
            trace: DatabaseEventTrace::default(),
        }
    }

    /// Lead's turn, as a question's wait sees it.
    fn turn(&self) -> ForegroundTurn {
        let turn = ForegroundTurn::default();
        turn.begin(DatabaseEventTrace::default());
        turn
    }

    fn messages(&self, agent_id: i64) -> Vec<IntelligenceAgentMessage> {
        self.db
            .send_blocking(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 100,
                ids: None,
            })
            .unwrap()
    }

    /// The reply both of the question's messages hold.
    fn reply(&self) -> AgentAskReply {
        let replies: Vec<_> = [self.lead, self.app]
            .into_iter()
            .flat_map(|agent_id| self.messages(agent_id))
            .filter_map(|message| match message.message_content {
                AgentMessageContent::AskedAgent(ask) | AgentMessageContent::AskedByAgent(ask) => {
                    Some(ask.reply)
                }
                _ => None,
            })
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0], replies[1]);
        replies[0].clone()
    }

    fn app_run_outcome(&self) -> IntelligenceAgentRunOutcome {
        let runs = self
            .db
            .send_blocking(ListAgentRunsCommand {
                agent_id: self.app,
                limit: 10,
            })
            .unwrap();
        assert_eq!(runs[0].sender_agent_id, Some(self.lead));
        match runs[0].state {
            IntelligenceAgentRunState::Running => IntelligenceAgentRunOutcome::Running,
            IntelligenceAgentRunState::Completed { .. } => IntelligenceAgentRunOutcome::Completed,
            IntelligenceAgentRunState::Failed { .. } => IntelligenceAgentRunOutcome::Failed,
        }
    }
}

fn answered(text: &str) -> AgentAskReply {
    AgentAskReply::Answered {
        text: text.to_string(),
    }
}

/// The model's answer replying `text` to the agent that asked.
fn replying(text: &str) -> Answer {
    calling_with("reply_to_agent", json!({ "text": text }))
}

/// Whether `request` offers the model tool `name`.
fn offers(request: &Value, name: &str) -> bool {
    request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["function"]["name"] == name)
}

/// What the tool calls in `request`'s transcript returned, in order.
fn tool_results(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "tool")
        .map(|message| message["content"].as_str().unwrap().to_string())
        .collect()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_waiting_asker_gets_the_forks_reply_which_settles_the_question() {
    let team = team(vec![replying("Green."), text("")]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        let answer = asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        answer
    });

    assert_eq!(answer, Some(answered("Green.")));
    assert_eq!(team.reply(), answered("Green."));
    assert_eq!(
        team.app_run_outcome(),
        IntelligenceAgentRunOutcome::Completed
    );
    let sent = team.model.sent();
    assert!(offers(&sent[0], "reply_to_agent"));
    assert!(texts(&sent[0], "user")[1].contains("call `reply_to_agent`"));
    // The reply is the answer; the empty final response after it is no one's.
    assert_eq!(sent.len(), 2);
}

#[test]
fn a_fork_that_ends_without_replying_is_told_once_then_the_asker_hears_no_answer() {
    let team = team(vec![text("Green."), text("Still green.")]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    let unanswered = AgentAskReply::Unanswered {
        reason: "It ended its turn without replying.".to_string(),
    };
    assert_eq!(answer, Some(unanswered.clone()));
    assert_eq!(team.reply(), unanswered);
    // Its final responses never reached the asker; it was told once.
    let sent = team.model.sent();
    assert_eq!(sent.len(), 2);
    let users = texts(&sent[1], "user");
    assert!(users.last().unwrap().starts_with("# No reply sent"));
}

#[test]
fn a_fork_told_it_has_not_replied_can_still_reply() {
    let team = team(vec![text("Green."), replying("Green."), text("")]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    assert_eq!(answer, Some(answered("Green.")));
    assert_eq!(team.reply(), answered("Green."));
}

// A reply marked `done` is the fork's last model call: nothing asks it for
// a final response.
#[test]
fn a_reply_marked_done_ends_the_fork_on_it() {
    let team = team(vec![calling_with(
        "reply_to_agent",
        json!({ "text": "Green.", "done": true }),
    )]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        let answer = asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        answer
    });

    assert_eq!(answer, Some(answered("Green.")));
    assert_eq!(team.model.sent().len(), 1);
    assert_eq!(
        team.app_run_outcome(),
        IntelligenceAgentRunOutcome::Completed
    );
}

// A one-way hand-off marked `done` ends the fork that made it: the agent
// handed the work wakes on its own fork, and the fork that handed it over
// is asked for nothing more.
#[test]
fn a_one_way_hand_off_marked_done_ends_the_fork_on_it() {
    let team = team(vec![
        calling_with(
            "ask_agent",
            json!({ "agent_id": 1, "question": "Release started.", "reply": false, "done": true }),
        ),
        text("OK"),
    ]);
    runtime().block_on(async {
        team.messenger
            .hand_off(team.ask("Start the release, and tell me once it has."))
            .await
            .unwrap();
        team.work.idle().await;
    });

    assert_eq!(team.model.sent().len(), 2);
    assert_eq!(team.messages(team.lead).len(), 2);
}

// A one-way fork that ends on a call marked `done`, with no final response,
// did the work it was handed.
#[test]
fn a_one_way_fork_that_ends_on_a_call_marked_done_settles_as_ok() {
    let team = team(vec![calling_with(
        "send_note",
        json!({ "note": "Release started.", "done": true }),
    )]);
    runtime().block_on(async {
        team.messenger
            .hand_off(team.ask("Start the release."))
            .await
            .unwrap();
        team.work.idle().await;
    });

    assert_eq!(team.model.sent().len(), 1);
    assert_eq!(team.reply(), answered("OK"));
}

#[test]
fn a_one_way_question_has_no_reply_to_give_and_wakes_no_one() {
    let team = team(vec![replying("Started."), text("OK")]);
    runtime().block_on(async {
        team.messenger
            .hand_off(team.ask("Start the release."))
            .await
            .unwrap();
        team.work.idle().await;
    });

    let sent = team.model.sent();
    assert_eq!(sent.len(), 2);
    assert!(texts(&sent[0], "user")[1].contains("only perform the task, then respond with OK"));
    // The fork's `reply_to_agent` has no one to reply to.
    let results = tool_results(&sent[1]);
    assert!(
        results[0].contains("No agent waits for a reply"),
        "{results:?}"
    );
    // Its final response settles the question, and reaches no one: the
    // asker's conversation holds only its question, and runs nothing.
    assert_eq!(team.reply(), answered("OK"));
    assert_eq!(team.messages(team.lead).len(), 1);
    let lead_runs = team
        .db
        .send_blocking(ListAgentRunsCommand {
            agent_id: team.lead,
            limit: 10,
        })
        .unwrap();
    assert!(lead_runs.is_empty());
}

#[test]
fn a_waiting_asker_is_let_go_when_cohesion_stops() {
    let team = team(vec![None]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        team.work.close();
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    assert_eq!(
        answer,
        Some(AgentAskReply::Unanswered {
            reason: ASK_STOPPED_REASON.to_string()
        })
    );
    // The fork stopped without settling: shutdown's cleanup settles the
    // question.
    assert_eq!(team.reply(), AgentAskReply::Waiting);
}

#[test]
fn a_reply_after_the_asker_stopped_waiting_settles_the_question_and_wakes_no_one() {
    let team = team(vec![replying("Green."), text("")]);
    let waited = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        let turn = team.turn();
        turn.stop();
        let waited = asked.wait(ASK_LIMIT, &turn).await;
        team.work.idle().await;
        waited
    });

    assert_eq!(waited, None);
    assert_eq!(team.reply(), answered("Green."));
    assert_eq!(team.messages(team.lead).len(), 1);
}

#[test]
fn the_users_stop_of_the_asked_agent_stops_its_fork() {
    let team = team(vec![None]);
    let answer = runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        // Once the fork has called the model, it is running and counted.
        let mut calls = team.model.calls();
        calls.wait_for(|calls| *calls >= 1).await.unwrap();
        team.messenger.stop_forks(team.app);
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    let reason = "The user stopped it.".to_string();
    assert_eq!(
        answer,
        Some(AgentAskReply::Unanswered {
            reason: reason.clone()
        })
    );
    assert_eq!(team.reply(), AgentAskReply::Unanswered { reason });
    assert_eq!(team.app_run_outcome(), IntelligenceAgentRunOutcome::Failed);
}

#[test]
fn a_question_is_a_hop_and_is_asked_however_far_from_the_user() {
    let team = team(vec![text("OK"), text("OK")]);
    let hops = |n| (0..n).fold(DatabaseEventTrace::default(), |trace, _| trace.hop());
    let from = |trace| Ask {
        asker_run_id: Some(1),
        trace,
        ..team.ask("Is the build green?")
    };
    let past = runtime().block_on(async {
        team.messenger.hand_off(from(hops(9))).await.unwrap();
        team.work.idle().await;
        // Far along a chain asks too: hops are counted, never limited.
        let past = team.messenger.hand_off(from(hops(50))).await;
        team.work.idle().await;
        past
    });

    assert!(past.is_ok());
    // Both questions were asked and answered, in both conversations.
    let replies: Vec<_> = [team.lead, team.app]
        .into_iter()
        .flat_map(|agent_id| team.messages(agent_id))
        .filter_map(|message| match message.message_content {
            AgentMessageContent::AskedAgent(ask) | AgentMessageContent::AskedByAgent(ask) => {
                Some(ask.reply)
            }
            _ => None,
        })
        .collect();
    assert_eq!(replies, vec![answered("OK"); 4]);
}

#[test]
fn a_fork_carries_on_the_conversation_as_its_last_turn_left_it() {
    let team = team(vec![
        text("On it."),
        replying("Started the share sheet."),
        text(""),
    ]);
    let answer = runtime().block_on(async {
        // App's conversation answers the user once, then starts a turn it
        // hasn't finished.
        let mut session = Session::new(
            team.db.clone(),
            team.model.clone(),
            tier_model(&team.db, IntelligenceTier::Agent).await.unwrap(),
            SystemPrompt {
                profile: Profile::default(),
                roster: Roster::default(),
                user: AboutUser::default(),
                memories: Vec::new(),
                memory_limit: 20,
            },
            Vec::new(),
            conversation_session::cache_key(team.app),
        )
        .await
        .unwrap();
        session
            .send_steerable(
                "Build the share sheet.".to_string(),
                Vec::new(),
                &Unshown,
                &ForegroundTurn::default(),
            )
            .await
            .unwrap();
        team.marks
            .set(team.app, ThreadMark::of(&session, 1, 0))
            .await
            .unwrap();
        let running = TranscriptItem::UserMessage(Prompt {
            text: "A turn still running.".to_string(),
            images: Vec::new(),
        });
        team.db
            .send(running.as_create_message_command(session.thread()))
            .await
            .unwrap();
        // What the conversation said since the mark.
        team.db
            .send(CreateAgentMessageCommand {
                agent_id: team.app,
                run_id: None,
                run_description: Some("The tests ran".to_string()),
                sender: IntelligenceAgentMessageSender::BackgroundAgent,
                content: AgentMessageContent::Text {
                    text: "The tests pass now.".to_string(),
                },
                reply_to: None,
            })
            .await
            .unwrap();

        let asked = team
            .messenger
            .ask(team.ask("Is the share sheet done?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    assert_eq!(answer, Some(answered("Started the share sheet.")));
    let sent = team.model.sent();
    let users = texts(&sent[1], "user");
    assert_eq!(users.len(), 2);
    assert_eq!(users[0], "Build the share sheet.");
    let question = &users[1];
    assert!(question.contains("The tests pass now."));
    assert!(question.contains("# Lead asks\nIs the share sheet done?"));
    assert_eq!(texts(&sent[1], "system"), texts(&sent[0], "system"));
}

// A fork that carries on the conversation's thread keeps its system prompt,
// so the cached prefix holds, and hears the memories kept since the thread
// last heard at the top of its question.
#[test]
fn a_fork_hears_the_memories_kept_since_the_mark_in_its_question() {
    let team = team(vec![text("On it."), replying("Not yet."), text("")]);
    runtime().block_on(async {
        let mut session = Session::new(
            team.db.clone(),
            team.model.clone(),
            tier_model(&team.db, IntelligenceTier::Agent).await.unwrap(),
            conversation_session::system_prompt(
                &team.db,
                &team
                    .db
                    .send(GetIntelligenceAgentCommand { agent_id: team.app })
                    .await
                    .unwrap(),
            )
            .await
            .unwrap(),
            Vec::new(),
            conversation_session::cache_key(team.app),
        )
        .await
        .unwrap();
        session
            .send_steerable(
                "Build the share sheet.".to_string(),
                Vec::new(),
                &Unshown,
                &ForegroundTurn::default(),
            )
            .await
            .unwrap();
        team.marks
            .set(team.app, ThreadMark::of(&session, 1, 0))
            .await
            .unwrap();
        team.db
            .send(CreateMemoryCommand {
                scope: MemoryScope::Global,
                memory: "The user is called Jo.".to_string(),
            })
            .await
            .unwrap();

        let asked = team
            .messenger
            .ask(team.ask("Is the share sheet done?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await
    });

    let sent = team.model.sent();
    let before = texts(&sent[0], "system");
    assert!(before.join("\n").ends_with("\nNone yet."));
    assert_eq!(texts(&sent[1], "system"), before);
    let users = texts(&sent[1], "user");
    let question = users.last().unwrap();
    assert!(question.contains("# Memories changed\n"), "{question}");
    assert!(question.contains("\n[m1] (global) The user is called Jo.\n\n# Lead asks\n"));
}

// A thread's mark keeps the capacity its prompt states, not the setting's when
// the turn ended, so a change made during a turn still rebuilds the thread. The
// next start does it, whichever it is: here a fork.
#[test]
fn a_memory_capacity_changed_during_a_turn_rebuilds_the_thread() {
    let team = team(vec![text("On it."), replying("Not yet."), text("")]);
    let marked = runtime().block_on(async {
        let mut session = Session::new(
            team.db.clone(),
            team.model.clone(),
            tier_model(&team.db, IntelligenceTier::Agent).await.unwrap(),
            conversation_session::system_prompt(
                &team.db,
                &team
                    .db
                    .send(GetIntelligenceAgentCommand { agent_id: team.app })
                    .await
                    .unwrap(),
            )
            .await
            .unwrap(),
            Vec::new(),
            conversation_session::cache_key(team.app),
        )
        .await
        .unwrap();
        session
            .send_steerable(
                "Build the share sheet.".to_string(),
                Vec::new(),
                &Unshown,
                &ForegroundTurn::default(),
            )
            .await
            .unwrap();
        let settings = team.db.send(GetStorageSettingsCommand {}).await.unwrap();
        team.db
            .send(UpdateStorageSettingsCommand {
                thread_retention_days: settings.thread_retention_days,
                actor_session_retention_days: settings.actor_session_retention_days,
                rewind_retention_days: settings.rewind_retention_days,
                deleted_item_retention_days: settings.deleted_item_retention_days,
                memory_limit: 7,
            })
            .await
            .unwrap();
        team.marks
            .set(team.app, ThreadMark::of(&session, 1, 0))
            .await
            .unwrap();
        let lookup = || {
            team.db
                .send(LookupAgentThreadMarkByAgentCommand { agent_id: team.app })
        };
        let marked = lookup().await.unwrap().unwrap().memory_limit;

        let asked = team
            .messenger
            .ask(team.ask("Is the share sheet done?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        assert_eq!(lookup().await.unwrap().unwrap().memory_limit, 7);
        marked
    });

    assert_eq!(marked, 20);
    let sent = team.model.sent();
    assert!(
        texts(&sent[0], "system")
            .join("\n")
            .contains("Each scope keeps at most 20.")
    );
    assert!(
        texts(&sent[1], "system")
            .join("\n")
            .contains("Each scope keeps at most 7.")
    );
    let feed = team
        .db
        .send_blocking(ListAgentMessagesCommand {
            agent_id: team.app,
            before: None,
            limit: 10,
            ids: None,
        })
        .unwrap();
    assert!(feed.iter().any(|message| matches!(
        message.message_content,
        AgentMessageContent::RebuiltContext {
            reason: Some(ContextRebuildReason::MemoryCapacityChanged),
        }
    )));
}

// A fork that copies the conversation's thread reads its cached prompt back,
// which keeps that cache alive from the fork's first request: the mark
// takes its time, so the conversation's next turn and later forks judge the
// thread warm from then, not from the conversation's own last call.
#[test]
fn a_fork_that_copies_the_conversation_refreshes_its_mark() {
    let team = team(vec![text("On it."), replying("Not yet."), text("")]);
    let (marked, refreshed) = runtime().block_on(async {
        let mut session = Session::new(
            team.db.clone(),
            team.model.clone(),
            tier_model(&team.db, IntelligenceTier::Agent).await.unwrap(),
            SystemPrompt {
                profile: Profile::default(),
                roster: Roster::default(),
                user: AboutUser::default(),
                memories: Vec::new(),
                memory_limit: 20,
            },
            Vec::new(),
            conversation_session::cache_key(team.app),
        )
        .await
        .unwrap();
        session
            .send_steerable(
                "Build the share sheet.".to_string(),
                Vec::new(),
                &Unshown,
                &ForegroundTurn::default(),
            )
            .await
            .unwrap();
        team.marks
            .set(team.app, ThreadMark::of(&session, 1, 0))
            .await
            .unwrap();
        // The conversation last called a minute ago, still well inside the
        // cache's five minutes.
        let lookup = || {
            team.db
                .send(LookupAgentThreadMarkByAgentCommand { agent_id: team.app })
        };
        let mark = lookup().await.unwrap().unwrap();
        let marked = mark.last_call_at - 60_000;
        team.db
            .send(SetAgentThreadMarkCommand {
                agent_id: team.app,
                thread_id: mark.thread_id,
                items: mark.items,
                endpoint_model_id: mark.endpoint_model_id,
                extended_from: mark.extended_from,
                heard_through: mark.heard_through,
                memory_limit: mark.memory_limit,
                memories: mark.memories,
                last_call_at: marked,
                prompt_tokens: mark.prompt_tokens,
                cache_seconds: mark.cache_seconds,
            })
            .await
            .unwrap();

        let asked = team
            .messenger
            .ask(team.ask("Is the share sheet done?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        (marked, lookup().await.unwrap().unwrap().last_call_at)
    });

    // The fork copied the conversation's thread rather than opening one.
    let sent = team.model.sent();
    assert_eq!(texts(&sent[1], "user")[0], "Build the share sheet.");
    assert!(refreshed >= marked + 60_000, "{refreshed} vs {marked}");
}

// A fork that finds the main thread's cache expired rebuilds it from the feed:
// the mark moves onto the new thread, whose opening the next fork copies, so only
// the first writes it.
#[test]
fn a_fork_rebuilds_the_expired_main_thread_and_the_next_fork_copies_that() {
    let team = team(vec![
        text("On it."),
        replying("Not yet."),
        text(""),
        replying("Still not."),
        text(""),
    ]);
    let (old_thread, new_thread) = runtime().block_on(async {
        let mut session = Session::new(
            team.db.clone(),
            team.model.clone(),
            tier_model(&team.db, IntelligenceTier::Agent).await.unwrap(),
            SystemPrompt {
                profile: Profile::default(),
                roster: Roster::default(),
                user: AboutUser::default(),
                memories: Vec::new(),
                memory_limit: 20,
            },
            Vec::new(),
            conversation_session::cache_key(team.app),
        )
        .await
        .unwrap();
        session
            .send_steerable(
                "Build the share sheet.".to_string(),
                Vec::new(),
                &Unshown,
                &ForegroundTurn::default(),
            )
            .await
            .unwrap();
        team.marks
            .set(team.app, ThreadMark::of(&session, 1, 0))
            .await
            .unwrap();
        let lookup = || {
            team.db
                .send(LookupAgentThreadMarkByAgentCommand { agent_id: team.app })
        };
        // A twelve-hour cache, last read thirteen hours ago.
        let mark = lookup().await.unwrap().unwrap();
        let old_last_call = mark.last_call_at - 13 * 60 * 60 * 1000;
        team.db
            .send(SetAgentThreadMarkCommand {
                agent_id: team.app,
                thread_id: mark.thread_id,
                items: mark.items,
                endpoint_model_id: mark.endpoint_model_id,
                extended_from: mark.extended_from,
                heard_through: mark.heard_through,
                memory_limit: mark.memory_limit,
                memories: mark.memories,
                last_call_at: old_last_call,
                prompt_tokens: mark.prompt_tokens,
                cache_seconds: 12 * 60 * 60,
            })
            .await
            .unwrap();

        let old_thread = mark.thread_id;
        let mut moved_to = Vec::new();
        for question in ["Is the share sheet done?", "And now?"] {
            let asked = team.messenger.ask(team.ask(question)).await.unwrap();
            asked.wait(ASK_LIMIT, &team.turn()).await;
            team.work.idle().await;
            moved_to.push(lookup().await.unwrap().unwrap().thread_id);
        }
        assert_eq!(moved_to[0], moved_to[1]);
        (old_thread, moved_to[0])
    });

    assert_ne!(new_thread, old_thread);
    let sent = team.model.sent();
    assert_eq!(sent.len(), 5);
    let first = texts(&sent[1], "user");
    assert!(first[0].starts_with("# Recent conversation"), "{first:?}");
    assert!(
        first[1].starts_with("# Status\nCurrent time: "),
        "{first:?}"
    );
    assert!(first[1].contains("Is the share sheet done?"), "{first:?}");
    assert!(!first.join("\n").contains("Build the share sheet."));
    let next = texts(&sent[3], "user");
    assert_eq!(next[0], first[0]);
    assert!(next[1].contains("And now?"), "{next:?}");
    assert_eq!(next.len(), 2, "{next:?}");
}

// A fork with no main thread to copy opens it with the recent conversation,
// which the next fork carries on while it is warm: the next hears, after it,
// what the conversation said since, the first fork's question and what it
// posted among it.
#[test]
fn a_fork_opens_the_main_thread_when_there_is_none_and_the_next_copies_it() {
    let team = team(vec![
        calling_with("send_message", json!({ "text": "Telling the user." })),
        replying("Green."),
        text(""),
        replying("Still green."),
        text(""),
    ]);
    runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        team.db
            .send(CreateAgentMessageCommand {
                agent_id: team.app,
                run_id: None,
                run_description: Some("The tests ran".to_string()),
                sender: IntelligenceAgentMessageSender::BackgroundAgent,
                content: AgentMessageContent::Text {
                    text: "The tests pass now.".to_string(),
                },
                reply_to: None,
            })
            .await
            .unwrap();
        let asked = team.messenger.ask(team.ask("Still green?")).await.unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
    });

    let sent = team.model.sent();
    assert_eq!(sent.len(), 5);
    let first = texts(&sent[0], "user");
    assert_eq!(first.len(), 2);
    assert!(first[0].starts_with("# Recent conversation\n"), "{first:?}");
    assert!(!first[0].contains("Current time"), "{first:?}");
    assert!(
        first[1].starts_with("# Status\nCurrent time: "),
        "{first:?}"
    );
    let users = texts(&sent[3], "user");
    assert_eq!(users.len(), 2);
    assert_eq!(users[0], first[0]);
    let prompt = &users[1];
    assert!(prompt.starts_with("# Status\nCurrent time: "), "{prompt}");
    assert!(prompt.contains("Is the build green?"), "{prompt}");
    assert!(prompt.contains("Telling the user."), "{prompt}");
    assert!(prompt.contains("The tests pass now."), "{prompt}");
    assert!(prompt.contains("# Lead asks\nStill green?"));
}

// A fork that opens the main thread writes who the user is into its prompt,
// and a later fork copies that prompt as it stands: a profile changed in
// between waits for the next rebuild, so the cached prefix holds.
#[test]
fn a_thread_keeps_the_user_its_prompt_began_with() {
    let team = team(vec![
        replying("Green."),
        text(""),
        replying("Still green."),
        text(""),
    ]);
    team.db
        .send_blocking(SetProfileNameCommand {
            name: Some("Ada".to_string()),
        })
        .unwrap();
    team.db
        .send_blocking(SetProfileBioCommand {
            bio: Some("I run a bakery.".to_string()),
        })
        .unwrap();
    runtime().block_on(async {
        let asked = team
            .messenger
            .ask(team.ask("Is the build green?"))
            .await
            .unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
        team.db
            .send(SetProfileBioCommand {
                bio: Some("I sold the bakery.".to_string()),
            })
            .await
            .unwrap();
        let asked = team.messenger.ask(team.ask("Still green?")).await.unwrap();
        asked.wait(ASK_LIMIT, &team.turn()).await;
        team.work.idle().await;
    });

    let sent = team.model.sent();
    assert_eq!(sent.len(), 4);
    let system = texts(&sent[0], "system");
    assert!(
        system[0].contains(
            "\n\n# About the user\nName: Ada\nIn their own words:\nI run a bakery.\n\n# Memories\n"
        ),
        "{system:?}"
    );
    assert_eq!(texts(&sent[2], "system"), system);
}

/// An actor session started on the one actor a client offers.
fn actor_session(db: &DatabaseContext) -> ActorSession {
    let client_id = db
        .send_blocking(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "MacBook".to_string(),
                name: "MacBook".to_string(),
            },
        })
        .unwrap()
        .id;
    let actors = vec![ActorDefinition {
        identifier: "cohesion".to_string(),
        product: "Claude Code".to_string(),
        scope: "/src/cohesion".to_string(),
        modes: Vec::new(),
        models: Vec::new(),
    }];
    db.send_blocking(SyncCapabilitiesCommand {
        client_id,
        offers: ClientCapabilities { actors },
    })
    .unwrap();
    let actor_id = db.send_blocking(ListActorsCommand).unwrap()[0].id;
    db.send_blocking(CreateActorSessionCommand {
        actor_id,
        prompt: "Fix the build".to_string(),
        title: None,
        mode: None,
        model: None,
        effort: None,
        attachments: Vec::new(),
    })
    .unwrap()
    .0
}

#[test]
fn an_actor_session_followed_wakes_one_fork_whose_reply_goes_to_no_one() {
    let team = team(vec![text("Nothing to tell.")]);
    let actor_session = ActorSession {
        status: ActorSessionStatus::Idle,
        ..actor_session(&team.db)
    };
    runtime().block_on(async {
        // However far along a chain the actor_session moved, its follower wakes:
        // hops are counted, never limited.
        let moved = (0..30)
            .fold(DatabaseEventTrace::default(), |trace, _| trace.hop())
            .deeper();
        team.messenger
            .follow_up(team.app, actor_session, ActorSessionStop::Finished, &moved)
            .await;
        team.work.idle().await;
    });

    let sent = team.model.sent();
    assert_eq!(sent.len(), 1);
    let users = texts(&sent[0], "user");
    assert!(users[1].contains("# Actor session \"Fix the build\""));
    let runs = team
        .db
        .send_blocking(ListAgentRunsCommand {
            agent_id: team.app,
            limit: 10,
        })
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].hook, HookKind::ActorMoved);
    assert!(matches!(
        runs[0].state,
        IntelligenceAgentRunState::Completed { .. }
    ));
    // The follower's feed says why it woke; the final reply reaches
    // neither conversation.
    let app = team.messages(team.app);
    assert_eq!(app.len(), 1);
    assert!(matches!(
        &app[0].message_content,
        AgentMessageContent::Woke {
            by: AgentWake::ActorSession {
                stop: ActorSessionStop::Finished,
                ..
            }
        }
    ));
    assert_eq!(app[0].run_id, Some(runs[0].id));
    assert!(team.messages(team.lead).is_empty());
}
