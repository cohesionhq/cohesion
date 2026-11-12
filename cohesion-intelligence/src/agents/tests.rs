use cohesion_core::model::{
    AgentChoice, AgentCodeExecution, AgentCodeOutcome, AgentMessageContent, AgentSkillScope,
    AgentWake, ContextRebuildReason, HookKind, Initiative, IntelligenceAgentColor,
    IntelligenceAgentMessage, IntelligenceAgentMessageSender, IntelligenceEffort, IntelligenceTier,
    MemoryFilter, MemoryScope, SpendPeriod, TaskAssignee,
};
use cohesion_db::{
    AddEndpointModelCommand, ClearIntelligenceSpendLimitCommand, Container,
    CreateAgentMessageCommand, CreateAgentSkillCommand, CreateIntelligenceAgentCommand,
    CreateIntelligenceCostEventCommand, CreateIntelligenceThreadCommand, CreateMemoryCommand,
    CreateTaskCommand, CreateWorkspaceCommand, DatabaseContext, DatabaseEventTrace,
    ForkIntelligenceThreadCommand, GetIntelligenceDefaultCommand, GetStorageSettingsCommand,
    ListAgentCuesCommand, ListAgentMessagesCommand, ListAgentRunsCommand, ListMemoriesCommand,
    LookupAgentThreadMarkByAgentCommand, Secrets, SetAgentCueCommand, SetAgentThreadMarkCommand,
    SetIntelligenceDefaultCommand, SetIntelligenceSpendLimitCommand, SetProfileBioCommand,
    SetProfileNameCommand, ShutdownCleanupCommand, UpdateStorageSettingsCommand, WorkTracker,
};
use cohesion_js::{ScriptArgument, ScriptEngine};
use jiff::Zoned;
use jiff::tz::TimeZone;
use std::sync::Arc;

use super::agent_coordinator::AgentCoordinator;
use super::agent_host::AgentHost;
use super::agent_messenger::AgentMessenger;
use super::agent_transcript::UNANSWERED;
use crate::account_credentials::{AccountCredentials, CredentialFuture};
use crate::intelligence_error::IntelligenceError;
use crate::test_model::{self, Answer, TestModel};
use crate::transcript_item::{Prompt, TranscriptItem};

/// 2026-10-14 10:30:00 UTC.
pub(super) const NOW: i64 = 1_791_973_800;

pub(super) fn now() -> Zoned {
    jiff::Timestamp::from_second(NOW)
        .unwrap()
        .to_zoned(TimeZone::UTC)
}

pub(super) fn message(
    id: i64,
    sent_at: i64,
    sender: IntelligenceAgentMessageSender,
    content: AgentMessageContent,
) -> IntelligenceAgentMessage {
    IntelligenceAgentMessage {
        id,
        agent_id: 1,
        run_id: None,
        run_description: None,
        run_part: None,
        sent_at,
        sender,
        message_content: content,
        reaction: None,
        reacted_at: None,
        attachment_count: 0,
        reply_to: None,
        switched_workspace_id: None,
        switched_workspace_title: None,
        excerpt: String::new(),
        excerpt_verb: None,
        reaction_excerpt: None,
        reaction_excerpt_verb: None,
    }
}

pub(super) fn text(
    id: i64,
    sent_at: i64,
    sender: IntelligenceAgentMessageSender,
    text: &str,
) -> IntelligenceAgentMessage {
    message(
        id,
        sent_at,
        sender,
        AgentMessageContent::Text {
            text: text.to_string(),
        },
    )
}

/// A note a run of an older cue posted, under what woke it.
pub(super) fn run_note(id: i64) -> IntelligenceAgentMessage {
    let mut message = text(
        id,
        NOW - 60,
        IntelligenceAgentMessageSender::BackgroundAgent,
        "Moved \"Fix the build\" to Inbox",
    );
    message.run_description = Some("\"Test task\" was created".to_string());
    message
}

pub(super) fn choice(id: i64, selected: Option<usize>) -> (IntelligenceAgentMessage, AgentChoice) {
    let choice = AgentChoice {
        question: "File \"Buy stamps\" where?".to_string(),
        options: vec!["Home".to_string(), "Errands".to_string()],
        context: Some("Task 7 landed in Inbox; move it with task.move()".to_string()),
        selected,
    };
    let message = message(
        id,
        NOW - 60,
        IntelligenceAgentMessageSender::BackgroundAgent,
        AgentMessageContent::Choice(choice.clone()),
    );
    (message, choice)
}

/// An account's endpoint that no test turn reaches.
struct NoAccount;
impl AccountCredentials for NoAccount {
    fn authorization(&self, _: i64) -> CredentialFuture<'_> {
        Box::pin(async { Err(IntelligenceError::AccountCredential(String::new())) })
    }
}

/// Scout's conversation on a test model serving `answers`, and the
/// saved message of the user's it answers.
fn answering(answers: Vec<Answer>) -> (Arc<TestModel>, Vec<IntelligenceAgentMessage>) {
    answering_after(answers, |_| {})
}

/// As [`answering`], once `setup` has run on the database.
fn answering_after(
    answers: Vec<Answer>,
    setup: impl FnOnce(&DatabaseContext),
) -> (Arc<TestModel>, Vec<IntelligenceAgentMessage>) {
    conversing(answers, setup, &["Is the build green?"], |_, _, _| {})
}

/// Scout's conversation on a test model serving `answers`, once `setup` has
/// run on the database, answering each of `messages` in turn in one host,
/// so in one live thread; `before` runs with Scout's id and the message's
/// index before each is sent. Returns the model and Scout's feed.
fn conversing(
    answers: Vec<Answer>,
    setup: impl FnOnce(&DatabaseContext),
    messages: &[&str],
    before: impl Fn(&DatabaseContext, i64, usize) + Sync,
) -> (Arc<TestModel>, Vec<IntelligenceAgentMessage>) {
    restarting(answers, setup, messages, before, &[])
}

/// As [`conversing`], with the daemon restarted before each message whose
/// index is in `restarts`, once `before` has run: the database settled as a
/// stopped daemon leaves it, and a new host.
fn restarting(
    answers: Vec<Answer>,
    setup: impl FnOnce(&DatabaseContext),
    messages: &[&str],
    before: impl Fn(&DatabaseContext, i64, usize) + Sync,
    restarts: &[usize],
) -> (Arc<TestModel>, Vec<IntelligenceAgentMessage>) {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    setup(&db);
    let model = TestModel::serve(answers);
    model.store(&db, IntelligenceTier::Agent);
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

    let engine = ScriptEngine::new(db.read_write());
    let credentials: Arc<dyn AccountCredentials> = model.clone();
    let messenger = AgentMessenger::new(
        db.clone(),
        engine.clone(),
        credentials.clone(),
        WorkTracker::default(),
    );
    let activity = messenger.activity().clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let new_host = || AgentHost::new(agent_id, messenger.clone(), Arc::default());
        let mut host = new_host().await.unwrap();
        // The job runs on a task of its own, so the host goes in and back.
        for (index, text) in messages.iter().enumerate() {
            // Off the runtime's thread, which a blocking send would stall.
            std::thread::scope(|scope| {
                scope.spawn(|| before(&db, agent_id, index));
            });
            if restarts.contains(&index) {
                db.send(ShutdownCleanupCommand).await.unwrap();
                host = new_host().await.unwrap();
            }
            let message = db
                .send(CreateAgentMessageCommand {
                    agent_id,
                    run_id: None,
                    run_description: None,
                    sender: IntelligenceAgentMessageSender::User,
                    content: AgentMessageContent::Text {
                        text: text.to_string(),
                    },
                    reply_to: None,
                })
                .await
                .unwrap();
            host = activity
                .scope(
                    agent_id,
                    WorkTracker::default().begin("test"),
                    None,
                    |work| async move {
                        host.answer_message(message, Vec::new(), &work).await?;
                        Ok(host)
                    },
                )
                .await
                .unwrap();
        }
    });
    let feed = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id,
            before: None,
            limit: 10,
            ids: None,
        })
        .unwrap();
    (model, feed)
}

// A turn that answers the user with only its final reply is told once that
// the user saw nothing, and the reply itself is never posted.
#[test]
fn a_turn_that_sends_nothing_is_told_once_and_its_final_reply_stays_unposted() {
    let (model, feed) = answering(vec![
        test_model::text("Yes, it's green."),
        test_model::text("Still nothing."),
        test_model::text("Never asked for."),
    ]);
    let sent = model.sent();
    assert_eq!(sent.len(), 2);
    let prompts = test_model::texts(&sent[1], "user");
    assert_eq!(prompts.last().unwrap(), UNANSWERED);
    assert_eq!(feed.len(), 1);
    assert_eq!(feed[0].sender, IntelligenceAgentMessageSender::User);
}

// A thread started over earlier messages (here, from before the host
// existed, as after a restart) is a rebuilt one, and the feed says so once
// after the message that started it; a live thread's next turn posts no
// such line. A conversation's first thread holds nothing earlier and posts
// none either (`a_turn_that_sends_nothing_…`'s feed is the user's message).
#[test]
fn a_rebuilt_thread_says_it_compressed_the_context() {
    let (_, feed) = conversing(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |db, agent_id, index| {
            if index == 0 {
                db.send_blocking(CreateAgentMessageCommand {
                    agent_id,
                    run_id: None,
                    run_description: None,
                    sender: IntelligenceAgentMessageSender::Agent,
                    content: AgentMessageContent::Text {
                        text: "The build broke.".to_string(),
                    },
                    reply_to: None,
                })
                .unwrap();
            }
        },
    );
    let kinds: Vec<_> = feed
        .iter()
        .map(|message| match message.message_content {
            AgentMessageContent::Text { .. } => "text".to_string(),
            AgentMessageContent::RebuiltContext { .. } => "compressed".to_string(),
            _ => "other".to_string(),
        })
        .collect();
    assert_eq!(kinds, ["text", "text", "compressed", "text"]);
    assert_eq!(
        feed[2].excerpt,
        "Context rebuilt because there was no live thread"
    );
}

// A restarted daemon carries the conversation's thread on from the mark its
// last turn left while the provider still holds it cached: the next turn's
// request starts with everything the last one sent, under the same system
// prompt, and no rebuilt context is posted.
#[test]
fn a_restart_carries_a_warm_thread_on() {
    let (model, feed) = restarting(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |_, _, _| {},
        &[1],
    );
    // Each turn posts nothing, so it is told once and calls again: the
    // first turn's last request is sent[1], the restarted turn's first
    // sent[2].
    let sent = model.sent();
    assert_eq!(sent.len(), 4);
    assert_eq!(
        test_model::texts(&sent[2], "system"),
        test_model::texts(&sent[1], "system")
    );
    // The restarted turn's request repeats the last one's, every message
    // and tool exactly, then adds the reply it got and the new message:
    // the prefix the provider cached.
    let before = sent[1]["messages"].as_array().unwrap();
    let after = sent[2]["messages"].as_array().unwrap();
    assert_eq!(after.len(), before.len() + 2);
    assert_eq!(after[..before.len()], before[..]);
    assert_eq!(sent[2]["tools"], sent[1]["tools"]);
    assert!(
        after.last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("And now?")
    );
    assert!(feed.iter().all(|message| !matches!(
        message.message_content,
        AgentMessageContent::RebuiltContext { .. }
    )));
}

// A thread whose cache has expired is rebuilt from the feed: the new thread has
// no tool calls of the old one, only lines saying what happened, the prompt
// says the time, and the feed says why.
#[test]
fn a_thread_whose_cache_expired_rebuilds() {
    let (model, feed) = restarting(
        vec![
            test_model::calling_with("documentation", serde_json::json!({ "module": "actors" })),
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
            test_model::text("And yes."),
            test_model::text("And yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?", "And later?"],
        |db, agent_id, index| {
            if index == 2 {
                age_mark(db, agent_id, 13 * 60 * 60, 12 * 60 * 60);
            }
        },
        &[],
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 7);
    let after = sent[5]["messages"].as_array().unwrap();
    assert!(after.iter().all(|message| message["role"] != "tool"));
    let prompt = after.last().unwrap()["content"].as_str().unwrap();
    assert!(prompt.contains("# Status"));
    assert!(prompt.contains("# Recent conversation"));
    assert!(prompt.contains("Is the build green?"));
    assert!(prompt.contains("loaded the `actors` module"));
    assert!(prompt.contains("And later?"));
    assert_eq!(rebuilt(&feed), [ContextRebuildReason::CacheExpired]);
}

// A long cache is read back however long since the latest call, until it
// expires: nothing changes and nothing is posted.
#[test]
fn a_thread_within_its_long_cache_carries_on_after_hours() {
    let (model, feed) = restarting(
        vec![
            test_model::calling_with("documentation", serde_json::json!({ "module": "actors" })),
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |db, agent_id, index| {
            if index == 1 {
                age_mark(db, agent_id, 11 * 60 * 60, 12 * 60 * 60);
            }
        },
        &[1],
    );
    let sent = model.sent();
    let before = sent[2]["messages"].as_array().unwrap();
    let after = sent[3]["messages"].as_array().unwrap();
    assert_eq!(after[..before.len()], before[..]);
    assert!(rebuilt(&feed).is_empty());
}

// A live conversation follows the main thread a fork moved on: its next
// request carries that thread as it stands, without rebuilding or saying so.
#[test]
fn a_conversation_follows_a_main_thread_a_fork_moved() {
    let (model, feed) = restarting(
        vec![
            test_model::calling_with("documentation", serde_json::json!({ "module": "actors" })),
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::calling_with(
                "documentation",
                serde_json::json!({ "module": "connectors" }),
            ),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
            test_model::text("Again."),
            test_model::text("Again."),
        ],
        |_| {},
        &["Is the build green?", "And now?", "And later?"],
        |db, agent_id, index| {
            if index == 2 {
                let mark = db
                    .send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
                    .unwrap()
                    .unwrap();
                let (moved, _) = db
                    .send_blocking(ForkIntelligenceThreadCommand {
                        thread_id: mark.thread_id,
                        items: mark.items as usize,
                    })
                    .unwrap();
                db.send_blocking(SetAgentThreadMarkCommand {
                    agent_id,
                    thread_id: moved.id,
                    items: mark.items,
                    endpoint_model_id: mark.endpoint_model_id,
                    extended_from: mark.extended_from,
                    heard_through: mark.heard_through,
                    memory_limit: mark.memory_limit,
                    memories: mark.memories,
                    last_call_at: mark.last_call_at,
                    prompt_tokens: 0,
                    cache_seconds: mark.cache_seconds,
                })
                .unwrap();
            }
        },
        &[],
    );
    let sent = model.sent();
    let before = sent[5]["messages"].as_array().unwrap();
    let after = sent[6]["messages"].as_array().unwrap();
    assert_eq!(after.len(), before.len() + 2);
    assert_eq!(after[..before.len()], before[..]);
    assert!(
        after.last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("# Status")
    );
    assert!(rebuilt(&feed).is_empty());
}

/// Scout's conversation over four messages, with the Memory capacity set to 7
/// before the third, restarted before each message in `restarts`. Returns the
/// model and Scout's feed.
fn changing_capacity(restarts: &[usize]) -> (Arc<TestModel>, Vec<IntelligenceAgentMessage>) {
    restarting(
        ["Yes."; 8].map(test_model::text).to_vec(),
        |_| {},
        &["Is the build green?", "And now?", "And later?", "Still?"],
        |db, _, index| {
            if index == 2 {
                let settings = db.send_blocking(GetStorageSettingsCommand {}).unwrap();
                db.send_blocking(UpdateStorageSettingsCommand {
                    thread_retention_days: settings.thread_retention_days,
                    actor_session_retention_days: settings.actor_session_retention_days,
                    rewind_retention_days: settings.rewind_retention_days,
                    deleted_item_retention_days: settings.deleted_item_retention_days,
                    memory_limit: 7,
                })
                .unwrap();
            }
        },
        restarts,
    )
}

// A changed Memory capacity rebuilds the thread under a prompt stating the
// new number, once, and an unchanged one touches nothing. A restarted daemon
// finds the change from the mark, as a live host does.
#[test]
fn the_prompt_follows_the_memory_capacity_when_it_changes() {
    for restarts in [&[][..], &[2][..]] {
        let (model, feed) = changing_capacity(restarts);
        let sent = model.sent();
        let system = |call: usize| test_model::texts(&sent[call], "system").join("\n");
        let messages = |call: usize| sent[call]["messages"].as_array().unwrap();

        assert!(system(3).contains("Each scope keeps at most 20."));
        assert_eq!(system(2), system(3));
        assert_eq!(messages(3)[..messages(2).len()], messages(2)[..]);

        assert_eq!(
            system(4),
            system(3).replace("keeps at most 20.", "keeps at most 7.")
        );
        // The new thread opens from the feed, under the new system message.
        assert!(
            messages(4).last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("# Recent conversation")
        );
        assert_eq!(system(5), system(4));
        assert_eq!(system(6), system(4));
        assert_eq!(messages(6)[..messages(5).len()], messages(5)[..]);
        assert_eq!(
            rebuilt(&feed),
            [ContextRebuildReason::MemoryCapacityChanged]
        );
    }
}

// The system prompt names the model a thread runs on, and a changed default
// rebuilds the thread under a prompt naming the new one.
#[test]
fn the_prompt_follows_the_model_when_the_default_changes() {
    let (model, feed) = conversing(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |db, _, index| {
            if index == 0 {
                return;
            }
            let tier = IntelligenceTier::Agent;
            let endpoint_id = db
                .send_blocking(GetIntelligenceDefaultCommand { tier })
                .unwrap()
                .endpoint
                .id;
            let next = db
                .send_blocking(AddEndpointModelCommand {
                    endpoint_id,
                    model_identifier: "next".to_string(),
                    model_name: None,
                    context_size: None,
                    prices: None,
                })
                .unwrap();
            db.send_blocking(SetIntelligenceDefaultCommand {
                tier,
                endpoint_model_id: next.id,
            })
            .unwrap();
        },
    );
    let sent = model.sent();
    let system = |call: usize| test_model::texts(&sent[call], "system").join("\n");
    assert!(system(1).contains("\n# Your model: base\n"));
    assert!(system(2).contains("\n# Your model: next\n"));
    assert_eq!(
        system(1).replace("# Your model: base", ""),
        system(2).replace("# Your model: next", "")
    );
    assert_eq!(rebuilt(&feed), [ContextRebuildReason::ModelChanged]);
}

/// Agent `agent_id`'s conversation mark, as if the provider counted
/// `prompt_tokens` in its thread's latest prompt.
fn count_mark(db: &DatabaseContext, agent_id: i64, prompt_tokens: i64) {
    let mark = db
        .send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
        .unwrap()
        .unwrap();
    db.send_blocking(SetAgentThreadMarkCommand {
        agent_id,
        thread_id: mark.thread_id,
        items: mark.items,
        endpoint_model_id: mark.endpoint_model_id,
        extended_from: mark.extended_from,
        heard_through: mark.heard_through,
        memory_limit: mark.memory_limit,
        memories: mark.memories,
        last_call_at: mark.last_call_at,
        prompt_tokens,
        cache_seconds: mark.cache_seconds,
    })
    .unwrap();
}

// A thread past the context limit is rebuilt from the feed, and says why; the
// new thread then carries on.
#[test]
fn a_thread_over_the_context_limit_rebuilds() {
    let (model, feed) = restarting(
        ["Yes."; 6].map(test_model::text).to_vec(),
        |_| {},
        &["Is the build green?", "And now?", "And later?"],
        |db, agent_id, index| {
            if index == 1 {
                count_mark(db, agent_id, 100_001);
            }
        },
        &[],
    );
    let sent = model.sent();
    let messages = |call: usize| sent[call]["messages"].as_array().unwrap();
    assert!(messages(2).len() < messages(1).len());
    assert!(
        messages(2).last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("# Recent conversation")
    );
    assert_eq!(messages(4)[..messages(3).len()], messages(3)[..]);
    assert_eq!(rebuilt(&feed), [ContextRebuildReason::ContextLimit]);
}

// A mark whose cache has expired by the time the daemon restarts rebuilds, and
// the feed says the cache expired.
#[test]
fn a_restart_after_the_cache_expired_rebuilds_the_thread() {
    let (model, feed) = restarting(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |db, agent_id, index| {
            if index == 1 {
                // Last read a second past a short cache's lifetime.
                age_mark(db, agent_id, 5 * 60 + 1, 5 * 60);
            }
        },
        &[1],
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 4);
    let after = sent[2]["messages"].as_array().unwrap();
    let prompt = after.last().unwrap()["content"].as_str().unwrap();
    assert!(prompt.contains("# Recent conversation"));
    assert!(prompt.contains("Is the build green?"));
    assert_eq!(rebuilt(&feed), [ContextRebuildReason::CacheExpired]);
}

/// Dates agent `agent_id`'s conversation mark `age` seconds back, on a cache
/// lasting `cache_seconds`.
fn age_mark(db: &DatabaseContext, agent_id: i64, age: i64, cache_seconds: i64) {
    let mark = db
        .send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
        .unwrap()
        .unwrap();
    db.send_blocking(SetAgentThreadMarkCommand {
        agent_id,
        thread_id: mark.thread_id,
        items: mark.items,
        endpoint_model_id: mark.endpoint_model_id,
        extended_from: mark.extended_from,
        heard_through: mark.heard_through,
        memory_limit: mark.memory_limit,
        memories: mark.memories,
        last_call_at: mark.last_call_at - age * 1000,
        prompt_tokens: mark.prompt_tokens,
        cache_seconds,
    })
    .unwrap();
}

/// Why the feed says its context was rebuilt, each time it was.
fn rebuilt(feed: &[IntelligenceAgentMessage]) -> Vec<ContextRebuildReason> {
    feed.iter()
        .filter_map(|message| match message.message_content {
            AgentMessageContent::RebuiltContext { reason } => reason,
            _ => None,
        })
        .collect()
}

// A turn the daemon stopped during leaves its thread longer than its mark,
// so the restarted daemon rebuilds the thread from the feed, which holds
// what that turn heard and did, rather than carrying on without it.
#[test]
fn a_restart_during_a_turn_rebuilds_the_thread() {
    let (model, feed) = restarting(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            test_model::text("Still yes."),
            test_model::text("Still yes."),
        ],
        |_| {},
        &["Is the build green?", "And now?"],
        |db, agent_id, index| {
            if index == 1 {
                let mark = db
                    .send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
                    .unwrap()
                    .unwrap();
                let cut_short = TranscriptItem::UserMessage(Prompt {
                    text: "A turn the daemon stopped during.".to_string(),
                    images: Vec::new(),
                });
                db.send_blocking(cut_short.as_create_message_command(mark.thread_id))
                    .unwrap();
            }
        },
        &[1],
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 4);
    assert!(
        test_model::texts(&sent[2], "user")
            .iter()
            .all(|text| !text.contains("A turn the daemon stopped during."))
    );
    assert!(feed.iter().any(|message| matches!(
        message.message_content,
        AgentMessageContent::RebuiltContext { .. }
    )));
}

// A thread's system prompt lists the memories as they were when it began,
// and never changes while it lives, so the provider's cached prefix holds:
// the next turn hears what was remembered since, by the thread itself or by
// another agent, at the top of its message.
#[test]
fn a_live_thread_keeps_its_prompt_and_hears_memory_changes_in_its_next_message() {
    let remember = Some(serde_json::json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": "m1",
            "type": "function",
            "function": {
                "name": "remember",
                "arguments": serde_json::json!({
                    "memory": "Jo wants builds reported in the morning.",
                    "scope": "personal",
                })
                .to_string(),
            },
        }],
    }));
    let global = |db: &DatabaseContext, memory: &str| {
        db.send_blocking(CreateMemoryCommand {
            scope: MemoryScope::Global,
            memory: memory.to_string(),
        })
        .unwrap();
    };
    let (model, _) = conversing(
        vec![
            remember,
            test_model::text("Noted."),
            test_model::text("Still noted."),
            test_model::text("Yes."),
            test_model::text("Yes, still."),
        ],
        |_| {},
        &["Report builds in the morning.", "Is the build green?"],
        |db, _, index| match index {
            0 => global(db, "The user is called Jo."),
            _ => global(db, "The office closes on Fridays."),
        },
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 5);
    let system = |call: usize| test_model::texts(&sent[call], "system").join("\n");
    assert!(system(0).ends_with("\n[m1] (global) The user is called Jo."));
    for call in 1..sent.len() {
        assert_eq!(system(call), system(0));
    }

    let users = test_model::texts(&sent[3], "user");
    let second = users.last().unwrap();
    assert!(second.starts_with("# Memories changed\n"), "{second}");
    assert!(second.contains(
        "\n[m2] Jo wants builds reported in the morning.\n\
         [m3] (global) The office closes on Fridays.\n\n# User's message\n"
    ));
    assert!(
        test_model::texts(&sent[0], "user")
            .iter()
            .all(|text| !text.contains("# Memories"))
    );
}

// A thread's status lists the skills the agent may use; when the user
// changes them, the live thread's next message lists them again under
// `# Skills changed`, and a message with no change says nothing of them.
#[test]
fn a_live_thread_hears_its_skills_change() {
    let save = |db: &DatabaseContext, agent_id: i64, name: &str, agent_can_use: bool| {
        db.send_blocking(CreateAgentSkillCommand {
            scope: AgentSkillScope::Agent { agent_id },
            name: name.to_string(),
            description: String::new(),
            prompt: format!("Do {name}."),
            agent_can_use,
        })
        .unwrap();
    };
    let (model, _) = conversing(
        // Each turn answers with only its final reply, so it is told once
        // that the user saw nothing.
        vec![
            test_model::text("Hi."),
            test_model::text("Hi."),
            test_model::text("Hi again."),
            test_model::text("Hi again."),
            test_model::text("Still here."),
            test_model::text("Still here."),
        ],
        |_| {},
        &["Hello.", "Hello again.", "Still there?"],
        |db, agent_id, index| match index {
            0 => save(db, agent_id, "Standup", true),
            1 => {
                save(db, agent_id, "Weekly review", true);
                save(db, agent_id, "Private", false);
            }
            _ => {}
        },
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 6);
    // The user's messages open calls 0, 2 and 4; each turn's nudge follows.
    let users = |call: usize| test_model::texts(&sent[call], "user");
    assert!(
        users(0)
            .iter()
            .any(|text| text.contains("Your skills (run one with `use_skill`):\n- \"Standup\"\n"))
    );
    let second = users(2)
        .into_iter()
        .find(|text| text.contains("Hello again."))
        .unwrap();
    assert!(second.starts_with("# Skills changed\n"), "{second}");
    assert!(second.contains("- \"Standup\"\n- \"Weekly review\"\n\n# User's message\n"));
    assert!(!second.contains("Private"));
    let third = users(4)
        .into_iter()
        .find(|text| text.contains("Still there?"))
        .unwrap();
    assert!(!third.contains("skills"), "{third}");
}

// A call to a tool that doesn't exist, such as one of the code tool's
// globals, is told the tools there are and where the globals belong.
#[test]
fn an_unknown_tool_is_told_the_tools_and_that_globals_go_in_a_script() {
    let (model, _) = answering(vec![
        test_model::calling("storage", &["s1"]),
        test_model::text("Done"),
        test_model::text("Still done."),
    ]);
    let sent = model.sent();
    let result = &test_model::texts(&sent[1], "tool")[0];
    assert!(result.starts_with("s1\nError: There is no tool named `storage`."));
    for tool in ["`code-tool`", "`send_message`", "`react`", "`remember`"] {
        assert!(result.contains(tool), "{tool} is missing from {result}");
    }
    for global in [
        "`storage`",
        "`actors`",
        "`agent`",
        "`cohesion`",
        "`connectors`",
    ] {
        assert!(result.contains(global), "{global} is missing from {result}");
    }
    assert!(!result.contains("`agentJob`"));
    assert!(result.ends_with("in a script you pass to `code-tool`."));
}

// A reaction answers the user, so the turn ends without being told.
#[test]
fn a_turn_that_reacts_is_not_told() {
    let react = |message_id: i64| {
        Some(serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "r1",
                "type": "function",
                "function": {
                    "name": "react",
                    "arguments": serde_json::json!({ "message_id": message_id, "emoji": "👍" }).to_string(),
                },
            }],
        }))
    };
    let (model, feed) = answering(vec![
        react(1),
        test_model::text("Done"),
        test_model::text("Never asked for."),
    ]);
    assert_eq!(model.sent().len(), 2);
    assert_eq!(feed[0].reaction.as_deref(), Some("👍"));
}

// A cue whose function throws says so in its agent's feed every time it
// fails, whatever the error says, and nothing once it evaluates cleanly.
#[test]
fn a_failing_cue_posts_its_error_every_time() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
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
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(
            db.clone(),
            engine,
            Arc::new(NoAccount),
            WorkTracker::default(),
        );
        let set = |script: &str| {
            db.send(SetAgentCueCommand {
                agent_id,
                name: "Watch".to_string(),
                hook: HookKind::Launch,
                script: script.to_string(),
                instructions: String::new(),
                effort: IntelligenceEffort::Low,
            })
        };
        let errors = || async {
            db.send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 50,
                ids: None,
            })
            .await
            .unwrap()
            .into_iter()
            .filter_map(|message| match message.message_content {
                AgentMessageContent::Error { error } => {
                    assert_eq!(
                        message.sender,
                        IntelligenceAgentMessageSender::BackgroundAgent
                    );
                    Some(error)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
        };
        let trace = DatabaseEventTrace::default();
        let fire = || coordinator.fire(HookKind::Launch, Vec::new(), &trace);

        set("() => { throw new Error('boom ' + Math.random()) }")
            .await
            .unwrap();
        for _ in 0..3 {
            fire().await;
        }
        let posted = errors().await;
        assert_eq!(posted.len(), 3);
        assert!(
            posted
                .iter()
                .all(|error| error.contains("\"Watch\"") && error.contains("boom")),
            "{posted:?}"
        );

        set("() => false").await.unwrap();
        fire().await;
        assert_eq!(errors().await.len(), 3);
    });
}

/// Records one model call costing `cost` dollars, now.
fn spend(db: &DatabaseContext, cost: f64) {
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: String::new(),
        })
        .unwrap();
    db.send_blocking(CreateIntelligenceCostEventCommand {
        thread_id: thread.id,
        provider_identifier: "test".to_string(),
        model_identifier: "base".to_string(),
        input_tokens: 1,
        output_tokens: 1,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        total_cost: cost,
        input_cost: None,
        output_cost: None,
        cache_read_cost: None,
        cache_write_cost: None,
    })
    .unwrap();
}

// A reached spend limit keeps a cue's runs from calling the model, and says
// so in the agent's feed once however often the cue fires; once spending is
// under every limit the runs call the model again, and a limit reached
// afresh says so once more.
#[test]
fn a_reached_spend_limit_holds_cue_runs_back_and_says_so_once() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(vec![test_model::text("Nothing new."); 4]);
    model.store(&db, IntelligenceTier::Low);
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
    db.send_blocking(SetAgentCueCommand {
        agent_id,
        name: "Watch".to_string(),
        hook: HookKind::Launch,
        script: "() => true".to_string(),
        instructions: String::new(),
        effort: IntelligenceEffort::Low,
    })
    .unwrap();
    spend(&db, 2.0);
    let limit = || {
        db.send_blocking(SetIntelligenceSpendLimitCommand {
            period: SpendPeriod::Day,
            amount: 1.0,
        })
        .unwrap()
    };
    assert!(limit()[1].reached);

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(db.clone(), engine, model.clone(), work.clone());
        let notices = || async {
            db.send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 50,
                ids: None,
            })
            .await
            .unwrap()
            .into_iter()
            .filter_map(|message| match message.message_content {
                AgentMessageContent::Error { error } => Some(error),
                _ => None,
            })
            .collect::<Vec<_>>()
        };
        let trace = DatabaseEventTrace::default();
        let fire = || async {
            coordinator.fire(HookKind::Launch, Vec::new(), &trace).await;
            work.idle().await;
        };

        fire().await;
        fire().await;
        assert_eq!(model.sent().len(), 0);
        let posted = notices().await;
        assert_eq!(posted.len(), 1);
        assert!(posted[0].contains("daily spend limit"), "{posted:?}");

        db.send(ClearIntelligenceSpendLimitCommand {
            period: SpendPeriod::Day,
        })
        .await
        .unwrap();
        fire().await;
        let called = model.sent().len();
        assert!(called > 0);

        db.send(SetIntelligenceSpendLimitCommand {
            period: SpendPeriod::Day,
            amount: 1.0,
        })
        .await
        .unwrap();
        fire().await;
        fire().await;
        assert_eq!(model.sent().len(), called);
        assert_eq!(notices().await.len(), 2);
    });
}

// A fork a cue woke was started by an event, so its role's database finds
// no area for the agent's memories or cues: `remember` and a script's
// `agent.cue` are refused, and nothing is written.
#[test]
fn a_cues_fork_changes_no_memory_or_cue() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(vec![
        test_model::calling_with(
            "remember",
            serde_json::json!({ "memory": "The user likes green.", "scope": "personal" }),
        ),
        test_model::calling_with(
            "code-tool",
            serde_json::json!({
                "title": "Adding a cue",
                "code": "agent.cue({ name: 'More', hook: 'task:create', code: '() => true', \
                         instructions: 'look', effort: 'low' })",
            }),
        ),
        test_model::text("Done."),
    ]);
    model.store(&db, IntelligenceTier::Low);
    let agent_id = new_agent(&db, "Scout");
    db.send_blocking(SetAgentCueCommand {
        agent_id,
        name: "Watch".to_string(),
        hook: HookKind::Launch,
        script: "() => true".to_string(),
        instructions: String::new(),
        effort: IntelligenceEffort::Low,
    })
    .unwrap();

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(db.clone(), engine, model.clone(), work.clone());
        coordinator
            .fire(HookKind::Launch, Vec::new(), &DatabaseEventTrace::default())
            .await;
        work.idle().await;
    });

    let sent = model.sent();
    assert_eq!(sent.len(), 3);
    let results = test_model::texts(&sent[2], "tool");
    assert!(results[0].contains("can't change memories"), "{results:?}");
    assert!(results[1].contains("can't set cues"), "{results:?}");
    let memories = db
        .send_blocking(ListMemoriesCommand {
            filter: MemoryFilter::SeenBy(agent_id),
        })
        .unwrap();
    assert!(memories.is_empty());
    let cues = db.send_blocking(ListAgentCuesCommand { agent_id }).unwrap();
    let names: Vec<_> = cues.iter().map(|cue| cue.name.as_str()).collect();
    assert_eq!(names, ["Watch"]);
}

// A cue's run starts fresh: its request holds none of the conversation, only
// its own prompt, and it leaves the main thread as it was.
#[test]
fn a_cues_run_starts_without_the_conversation() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(vec![test_model::text("Done.")]);
    model.store(&db, IntelligenceTier::Low);
    let agent_id = new_agent(&db, "Scout");
    db.send_blocking(CreateAgentMessageCommand {
        agent_id,
        run_id: None,
        run_description: None,
        sender: IntelligenceAgentMessageSender::User,
        content: AgentMessageContent::Text {
            text: "Something from the chat.".to_string(),
        },
        reply_to: None,
    })
    .unwrap();
    db.send_blocking(SetAgentCueCommand {
        agent_id,
        name: "Watch".to_string(),
        hook: HookKind::Launch,
        script: "() => true".to_string(),
        instructions: "Check the inbox.".to_string(),
        effort: IntelligenceEffort::Low,
    })
    .unwrap();

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(db.clone(), engine, model.clone(), work.clone());
        coordinator
            .fire(HookKind::Launch, Vec::new(), &DatabaseEventTrace::default())
            .await;
        work.idle().await;
    });

    let sent = model.sent();
    let users = test_model::texts(&sent[0], "user");
    assert_eq!(users.len(), 1, "{users:?}");
    assert!(users[0].starts_with("# Status\n"), "{users:?}");
    assert!(users[0].contains("Check the inbox."), "{users:?}");
    assert!(!users[0].contains("Something from the chat."), "{users:?}");
    assert!(!users[0].contains("Your cues"), "{users:?}");
    let mark = db
        .send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
        .unwrap();
    assert!(mark.is_none());
}

// A cue's run has a thread of its own, whose prompt says who the user is as
// the conversation's does.
#[test]
fn a_cues_run_reads_who_the_user_is() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(vec![test_model::text("Done.")]);
    model.store(&db, IntelligenceTier::Low);
    let agent_id = new_agent(&db, "Scout");
    db.send_blocking(SetProfileNameCommand {
        name: Some("Ada".to_string()),
    })
    .unwrap();
    db.send_blocking(SetProfileBioCommand {
        bio: Some("I run a bakery.".to_string()),
    })
    .unwrap();
    db.send_blocking(SetAgentCueCommand {
        agent_id,
        name: "Watch".to_string(),
        hook: HookKind::Launch,
        script: "() => true".to_string(),
        instructions: "Check the inbox.".to_string(),
        effort: IntelligenceEffort::Low,
    })
    .unwrap();

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(db.clone(), engine, model.clone(), work.clone());
        coordinator
            .fire(HookKind::Launch, Vec::new(), &DatabaseEventTrace::default())
            .await;
        work.idle().await;
    });

    let sent = model.sent();
    let system = test_model::texts(&sent[0], "system");
    assert!(
        system[0].contains(
            "\n\n# About the user\nName: Ada\nIn their own words:\nI run a bakery.\n\n# Memories\n"
        ),
        "{system:?}"
    );
}

// A turn that fails rebuilds the context for the next, which reads what the
// failed turn was told from the feed.
#[test]
fn a_failed_turn_rebuilds_the_context_for_the_next() {
    let malformed = || Some(serde_json::json!({ "role": "assistant", "content": 7 }));
    let (model, feed) = restarting(
        vec![
            test_model::text("Yes."),
            test_model::text("Yes."),
            malformed(),
            test_model::text("Back."),
            test_model::text("Back."),
        ],
        |_| {},
        &["Is the build green?", "And now?", "And later?"],
        |_, _, _| {},
        &[],
    );
    let sent = model.sent();
    assert_eq!(sent.len(), 5);
    let after = sent[3]["messages"].as_array().unwrap();
    let prompt = after.last().unwrap()["content"].as_str().unwrap();
    assert!(prompt.contains("# Recent conversation"), "{prompt}");
    assert!(prompt.contains("And later?"), "{prompt}");
    assert!(prompt.contains("And now?"), "{prompt}");
    assert_eq!(rebuilt(&feed), [ContextRebuildReason::TurnFailed]);
}

// The user's own turn is never held back by a spend limit.
#[test]
fn a_reached_spend_limit_still_answers_the_user() {
    let (model, _) = answering_after(
        vec![
            test_model::text("Yes, it's green."),
            test_model::text("Still nothing."),
        ],
        |db| {
            spend(db, 2.0);
            db.send_blocking(SetIntelligenceSpendLimitCommand {
                period: SpendPeriod::Hour,
                amount: 1.0,
            })
            .unwrap();
        },
    );
    assert!(!model.sent().is_empty());
}

// Assigning a task to an agent wakes one fork of its conversation, with the
// user's rights, when the user (or a script, or another agent's turn) made
// the change; the agent's own assignment, and a job's, wake nothing.
#[test]
fn a_task_assigned_to_an_agent_wakes_a_fork_unless_it_or_a_job_assigned_it() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(vec![test_model::text("On it.")]);
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
    let (scout, lead) = (agent("Scout"), agent("Lead"));
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "Home".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let task = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
            title: "Fix the build".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: Some(TaskAssignee::Agent { agent_id: scout }),
            description: Some("The release job fails on lint.".to_string()),
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap();
    let assigned = vec![ScriptArgument::Task {
        workspace_id,
        task_id: task.id,
    }];

    let engine = ScriptEngine::new(db.read_write());
    let work = WorkTracker::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let coordinator = AgentCoordinator::new(db.clone(), engine, model.clone(), work.clone());
        let fire = |trace: DatabaseEventTrace| {
            let (coordinator, work, assigned) = (&coordinator, &work, assigned.clone());
            async move {
                coordinator
                    .fire(HookKind::TaskAssigned, assigned, &trace.deeper())
                    .await;
                work.idle().await;
            }
        };

        fire(DatabaseEventTrace::default().from_agent(scout)).await;
        fire(
            DatabaseEventTrace::default()
                .started_by(Initiative::Event)
                .from_agent(lead),
        )
        .await;
        assert!(model.sent().is_empty());

        fire(DatabaseEventTrace::default().from_agent(lead)).await;
    });

    let sent = model.sent();
    assert_eq!(sent.len(), 1);
    let users = test_model::texts(&sent[0], "user");
    let prompt = users.last().unwrap();
    assert!(
        prompt.contains("# Task \"Fix the build\" assigned to you"),
        "{prompt}"
    );
    assert!(
        prompt.contains("The release job fails on lint."),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!("cohesion://task/{workspace_id}/{}", task.id)),
        "{prompt}"
    );
    assert!(
        prompt.contains("everything a turn of your conversation has"),
        "{prompt}"
    );
    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: scout,
            limit: 10,
        })
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].hook, HookKind::TaskAssigned);
    let feed = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id: scout,
            before: None,
            limit: 10,
            ids: None,
        })
        .unwrap();
    assert_eq!(feed.len(), 1);
    assert!(matches!(
        &feed[0].message_content,
        AgentMessageContent::Woke {
            by: AgentWake::AssignedTask { task_id, .. }
        } if *task_id == task.id
    ));
    assert_eq!(feed[0].excerpt, "“Fix the build” was assigned to you");
}

/// A new agent named `name` on `db`, by its id.
fn new_agent(db: &DatabaseContext, name: &str) -> i64 {
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
}

// A conversation's code tool records a script's whole output as a
// `codeExecution` message, returns only its first 32,000 characters with a
// note naming that message, and puts its effects' notes after the cut;
// `read_message` reads the rest of that message for its own agent, and
// another agent can't read it.
#[test]
fn the_code_tool_caps_its_output_and_read_message_recovers_the_rest() {
    use super::agent_tool::{AgentTool, Seat};
    use super::conversation_session::Branch;
    use super::foreground_code_tool::ForegroundCodeTool;
    use super::read_message_tool::ReadMessageTool;
    use crate::transcript_item::ToolCallStatus;

    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let agent_id = new_agent(&db, "Scout");
    let other_id = new_agent(&db, "Other");
    let engine = ScriptEngine::new(db.read_write());
    let code_tool = ForegroundCodeTool {
        engine,
        database: db.clone(),
        agent_id,
        services: Default::default(),
        connected_services: Default::default(),
        images: Default::default(),
    };
    let read = |agent_id| ReadMessageTool {
        database: db.clone(),
        agent_id,
    };
    let seat = Seat {
        turn: Arc::default(),
        branch: Branch::Conversation {
            sent: Arc::default(),
            reacted: Arc::default(),
        },
    };
    let call =
        |code: &str| serde_json::json!({ "title": "Listing everything", "code": code }).to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        // A quote, 31,999 `a`s, 500 `b`s and a quote: 32,501 characters, the
        // first 32,000 of them no `b`.
        let whole = format!("\"{}{}\"", "a".repeat(31_999), "b".repeat(500));
        let script = "'a'.repeat(31999) + 'b'.repeat(500)";

        let ToolCallStatus::Success(result) = code_tool.call(&seat, &call(script)).await else {
            panic!("the script failed");
        };
        let feed = db
            .send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 10,
                ids: None,
            })
            .await
            .unwrap();
        let [recorded] = feed.as_slice() else {
            panic!("expected one recorded run: {feed:?}");
        };
        let AgentMessageContent::CodeExecution(AgentCodeExecution {
            outcome: AgentCodeOutcome::Succeeded { result: saved },
            ..
        }) = &recorded.message_content
        else {
            panic!("expected a succeeded codeExecution: {recorded:?}");
        };
        assert!(saved == &whole, "the run was not recorded whole");
        let id = recorded.id;
        assert!(
            result
                == format!(
                    "{}… [501 more characters; `read_message` {id} from 32000 for the rest]",
                    &whole[..32_000]
                ),
            "the result was not cut with its note: {}",
            &result[31_990..]
        );

        let read_call = |start: usize| serde_json::json!({ "id": id, "start": start }).to_string();
        let ToolCallStatus::Success(rest) = read(agent_id).call(&seat, &read_call(32_000)).await
        else {
            panic!("read_message failed");
        };
        assert!(
            format!("{}{rest}", &result[..32_000]) == whole,
            "read_message returned the wrong rest: {rest}"
        );
        let ToolCallStatus::Failure(refused) = read(other_id).call(&seat, &read_call(0)).await
        else {
            panic!("another agent read the run");
        };
        assert_eq!(refused, "no message in this conversation has that id");

        // A refused effect's note follows the cut output, and the result is
        // a failure so the model notices.
        let ToolCallStatus::Failure(result) = code_tool
            .call(
                &seat,
                &call(&format!("agent.removeCue('Missing'); {script}")),
            )
            .await
        else {
            panic!("a refused effect passed");
        };
        let (cut, note) = result.split_once("for the rest]").unwrap();
        assert!(cut.starts_with(&whole[..32_000]) && !cut.contains('b'));
        assert_eq!(note, "\n\nYou have no cue named \"Missing\".");

        // Output within the cap comes back whole, with no note.
        let ToolCallStatus::Success(short) = code_tool.call(&seat, &call("'short'")).await else {
            panic!("the script failed");
        };
        assert_eq!(short, "\"short\"");
    });
}
