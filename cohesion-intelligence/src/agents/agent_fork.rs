//! A fork of an agent's conversation: a session that carries on the agent's
//! main thread (or opens it with the recent conversation when there is none),
//! or a cue's background run, which starts fresh, with the conversation's rights
//! and tools, for one turn, never seen by the user unless it messages them.
//! Another agent's question wakes one. A question that asks for a reply
//! gets it through the fork's `reply_to_agent`, never its final reply; a
//! fork that ends without replying is told so once, and the asker then
//! hears there is no answer. A one-way question's fork does what it asks
//! and replies to no one. An actor session the agent follows wakes one
//! as the actor_session stops working, to carry on the work the conversation handed
//! it, and a task someone else assigns the agent wakes one to work on it;
//! their final replies go to no one.

use std::sync::Arc;

use cohesion_core::log::Module;
use cohesion_core::model::{
    ActorSession, ActorSessionStop, AgentAskReply, CacheLength, ContextRebuildReason,
    IntelligenceAgentCue, IntelligenceTier, Task, Workspace,
};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    AgentAskRecord, AnswerAgentAskCommand, DatabaseContext, FinishAgentRunCommand,
    GetIntelligenceAgentCommand, ListAgentCuesCommand, SetAgentRunThreadCommand, TierModel,
};
use cohesion_js::ScriptOutput;
use jiff::Zoned;

use super::actor_exchange;
use super::agenda;
use super::agent_activity::Work;
use super::agent_error::AgentError;
use super::agent_messenger::{AgentMessenger, AskMode, OwedReply, RunningFork};
use super::agent_transcript;
use super::conversation_mark::ThreadMark;
use super::conversation_session::{self, Branch};
use super::foreground_turn::ForegroundTurn;
use super::installed_services::InstalledServices;
use super::main_thread::{self, MainThread};
use super::relevant_set;
use crate::connected_services::ConnectedServices;
use crate::intelligence_error::IntelligenceError;
use crate::session::{Session, tier_model};
use crate::spend_limits::SpendCap;
use crate::system_prompt::HeardMemories;
use crate::tools::SessionTool;

/// A fork about to run: its run, as recorded, and its turn.
pub(super) struct Fork {
    pub agent_id: i64,
    pub run_id: i64,
    /// The fork's turn, begun under a trace one hop further than what woke
    /// it, which the user's stop of the agent stops.
    pub turn: Arc<ForegroundTurn>,
    /// The fork's place among the agent's running forks, held until it is
    /// done.
    pub running: RunningFork,
}

/// What a fork was woken to answer.
pub(super) struct Question {
    /// The question's two messages and the run, as recorded.
    pub record: AgentAskRecord,
    pub text: String,
    /// The reply the asker waits for; `None` for a one-way question.
    pub owed: Option<Arc<OwedReply>>,
    /// The fork answering it, whose turn's trace is the question's.
    pub fork: Fork,
}

/// How a fork's one turn opens, in place of a message of the user's.
struct Opening {
    heading: String,
    text: String,
    /// The message in the conversation that stands for the fork, which its
    /// prompt carries itself, so the transcript leaves it out.
    held: Option<i64>,
    /// The reply the fork owes, which its `reply_to_agent` gives.
    owed: Option<Arc<OwedReply>>,
    start: Start,
}

/// What a fork's thread starts from.
#[derive(Clone, Copy)]
enum Start {
    /// The agent's main thread, on the `agent` tier's model.
    Main,
    /// Nothing but the fork's own prompt and the system prompt, on the short
    /// cache and this tier's model: a cue's background run, which never touches
    /// the main thread.
    Background(IntelligenceTier),
}

impl Start {
    fn tier(self) -> IntelligenceTier {
        match self {
            Self::Main => IntelligenceTier::Agent,
            Self::Background(tier) => tier,
        }
    }
}

/// Answers `question` in a fork of the asked agent's conversation, as
/// `work`. A question asking for a reply is settled by the fork's
/// `reply_to_agent`, which hands the reply to the asker if it still waits;
/// a fork that ends without it is told once, and if it still doesn't
/// reply, the question is settled as unanswered, saying why. A one-way
/// question is settled with the fork's final reply, which reaches no one,
/// `OK` when it ended without one.
/// The user's stop of the agent stops the fork, which answers that it was
/// stopped; the daemon's shutdown stops it and leaves the question to
/// `ShutdownCleanupCommand`. Records how it went on the run; only the fork
/// writes the question's messages.
pub(super) async fn answer(messenger: AgentMessenger, work: Work, question: Question) {
    let Question {
        record,
        text,
        owed,
        fork,
    } = question;
    let asker = &record.asker;
    let mode = match owed {
        Some(_) => AskMode::Reply,
        None => AskMode::OneWay,
    };
    let (heading, prompt) = agent_transcript::question(asker, &text, mode);
    let opening = Opening {
        heading,
        text: prompt,
        held: Some(record.asked_by.id),
        owed: owed.clone(),
        start: Start::Main,
    };
    let ended = run(&messenger, work, fork, opening).await;
    if let Some(owed) = owed {
        let reason = match ended {
            Ended::ShutDown => return,
            Ended::Replied(_) => NO_REPLY.to_string(),
            Ended::Failed(reason) => reason,
        };
        // A fork that replied settled the question already.
        owed.settle(AgentAskReply::Unanswered { reason }).await;
        return;
    }
    // A one-way question wants the work done, not words: a fork that ends
    // without a final response (on a call marked `done`, say) did it.
    let reply = match ended {
        Ended::ShutDown => return,
        Ended::Replied(text) if text.is_empty() => AgentAskReply::Answered {
            text: "OK".to_string(),
        },
        Ended::Replied(text) => AgentAskReply::Answered { text },
        Ended::Failed(reason) => AgentAskReply::Unanswered { reason },
    };
    let settled = messenger
        .database()
        .send(AnswerAgentAskCommand {
            asked: record.asked.id,
            asked_by: record.asked_by.id,
            reply,
        })
        .await;
    if let Err(e) = settled {
        log_error!(
            Module::Intelligence,
            "Agent {}: could not record its answer to agent {}: {e:?}",
            record.run.agent_id,
            asker.id
        );
    }
}

/// Why a question asking for a reply went unanswered when its fork ended
/// without replying, even once told.
const NO_REPLY: &str = "It ended its turn without replying.";

/// Carries on, in `fork`, the work the agent's conversation handed `task`,
/// an actor session it follows, which has just stopped working, as `work`. The
/// fork's final reply goes to no one: it messages the user, or does the
/// next thing, as its turn sees fit.
pub(super) async fn follow_up(
    messenger: AgentMessenger,
    work: Work,
    fork: Fork,
    actor_session: ActorSession,
    stop: ActorSessionStop,
) {
    let agent_id = fork.agent_id;
    let exchange = match actor_exchange::exchange(messenger.database(), actor_session.id).await {
        Ok(exchange) => Some(exchange),
        Err(e) => {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not read actor session {} to follow it up: {e:?}",
                actor_session.id
            );
            None
        }
    };
    let (heading, text) = agent_transcript::actor_session(&actor_session, stop, exchange.as_ref());
    let opening = Opening {
        heading,
        text,
        held: None,
        owed: None,
        start: Start::Main,
    };
    if let Ended::Failed(reason) = run(&messenger, work, fork, opening).await {
        log_info!(
            Module::Intelligence,
            "Agent {agent_id}: following up actor session {} ended without a reply: {reason}",
            actor_session.id
        );
    }
}

/// Works, in `fork`, on `task`, which someone other than the agent just
/// assigned it, as `work`. The fork's final reply goes to no one: it
/// messages the user, or does the work, as its turn sees fit.
pub(super) async fn assigned(messenger: AgentMessenger, work: Work, fork: Fork, task: Task) {
    let agent_id = fork.agent_id;
    let (heading, text) = agent_transcript::assigned_task(&task);
    let opening = Opening {
        heading,
        text,
        held: None,
        owed: None,
        start: Start::Main,
    };
    if let Ended::Failed(reason) = run(&messenger, work, fork, opening).await {
        log_info!(
            Module::Intelligence,
            "Agent {agent_id}: working on task {} assigned to it ended without a reply: {reason}",
            task.id
        );
    }
}

/// Wakes `fork` for `cue`, whose function returned `payload`, on the
/// default model of the tier the cue's effort names, as `work`. The fork's
/// trace is the cue's hook's, started by an event, so it only reads.
pub(super) async fn cue(
    messenger: AgentMessenger,
    work: Work,
    fork: Fork,
    cue: &IntelligenceAgentCue,
    payload: &ScriptOutput,
) {
    let agent_id = fork.agent_id;
    let (heading, text) = agent_transcript::cue_fired(cue, payload);
    let opening = Opening {
        heading,
        text,
        held: None,
        owed: None,
        start: Start::Background(cue.effort.into()),
    };
    if let Ended::Failed(reason) = run(&messenger, work, fork, opening).await {
        log_info!(
            Module::Intelligence,
            "Agent {agent_id}: cue \"{}\" ended without a reply: {reason}",
            cue.name
        );
    }
}

/// How a fork's one turn ended.
enum Ended {
    /// With its final reply, empty when it gave none.
    Replied(String),
    /// Without one, and why.
    Failed(String),
    /// The daemon shut down, which leaves the run to
    /// `ShutdownCleanupCommand`.
    ShutDown,
}

/// Runs `fork`'s one turn, opening as `opening` says, as `work`, and
/// records how it went on its run, unless the daemon shut down. Nothing carries a
/// fork's thread on, and what it posted reaches later turns as news; only
/// the main thread a fork opened is copied, once the fork has ended well.
async fn run(messenger: &AgentMessenger, work: Work, fork: Fork, opening: Opening) -> Ended {
    let Fork {
        agent_id,
        run_id,
        turn,
        running,
    } = fork;
    let outcome = match session(messenger, agent_id, run_id, &opening, &turn).await {
        Ok(Opened {
            session,
            prompt,
            opening: shared,
            copied,
        }) => {
            // A fork works without the user, so the spend limits hold it.
            let session = session.capped(SpendCap {
                limits: messenger.spend().clone(),
                agent_id,
                trace: turn.trace(),
            });
            let thread = SetAgentRunThreadCommand {
                run_id,
                thread_id: session.thread(),
            };
            if let Err(e) = messenger.database().send(thread).await {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not tie fork run {run_id} to its thread: {e:?}"
                );
            }
            let ended = converse(messenger, work, session, prompt, opening.owed, &turn).await;
            match ended {
                Some(Ok((reply, session))) => {
                    keep_marks(messenger, agent_id, &session, shared, copied).await;
                    Ended::Replied(reply)
                }
                Some(Err(reason)) => Ended::Failed(reason),
                None => Ended::ShutDown,
            }
        }
        Err(e) => Ended::Failed(format!("The run couldn’t start: {e}")),
    };
    drop(running);
    let error = match &outcome {
        Ended::ShutDown => return outcome,
        Ended::Replied(_) => None,
        Ended::Failed(reason) => Some(reason.clone()),
    };
    let finished = messenger
        .database()
        .send(FinishAgentRunCommand { run_id, error })
        .await;
    if let Err(e) = finished {
        log_error!(
            Module::Intelligence,
            "Agent {agent_id}: could not finish fork run {run_id}: {e:?}"
        );
    }
    outcome
}

/// Records what the fork that ran `session` left its agent: the main thread it
/// opened, or else that its first request read back the main thread it
/// `copied`, which keeps that thread's cache alive from then, as its own next
/// call would.
async fn keep_marks(
    messenger: &AgentMessenger,
    agent_id: i64,
    session: &Session,
    shared: Option<SharedOpening>,
    copied: Option<ThreadMark>,
) {
    let marks = messenger.marks();
    let kept = match (shared, copied, session.first_sent_at()) {
        (
            Some(SharedOpening {
                extended_from,
                heard_through,
                heard,
            }),
            _,
            _,
        ) => match ThreadMark::shared(session, extended_from, heard_through, heard) {
            Some(mark) => marks.set(agent_id, Some(mark)).await,
            None => Ok(()),
        },
        (None, Some(mark), Some(read_at)) => marks.refresh(agent_id, &mark, read_at).await,
        (None, _, _) => Ok(()),
    };
    if let Err(e) = kept {
        log_error!(
            Module::Intelligence,
            "Agent {agent_id}: could not keep its fork's thread mark: {e:?}"
        );
    }
}

/// Runs the fork's one turn on `prompt` until it ends or is stopped: the
/// final reply and the session that gave it, why there is none, or `None`
/// when the daemon shut down. A fork that owes a reply and ends without
/// giving it is told so once, on the same thread, and goes on for one more
/// answer.
async fn converse(
    messenger: &AgentMessenger,
    work: Work,
    mut session: Session,
    prompt: String,
    owed: Option<Arc<OwedReply>>,
    turn: &Arc<ForegroundTurn>,
) -> Option<Result<(String, Session), String>> {
    // On its own task, so a panic still ends the run.
    let input = turn.clone();
    let mut task = tokio::spawn(async move {
        let reply = session
            .send_steerable(prompt, Vec::new(), &work, &*input)
            .await?;
        let reply = match owed {
            Some(owed) if !owed.is_settled() => {
                session
                    .send_steerable(
                        agent_transcript::UNREPLIED.to_string(),
                        Vec::new(),
                        &work,
                        &*input,
                    )
                    .await?
            }
            _ => reply,
        };
        Ok((reply, session))
    });
    tokio::select! {
        sent = &mut task => Some(ended(sent)),
        () = messenger.closed() => {
            turn.stop();
            _ = task.await;
            None
        }
    }
}

/// The final reply a fork's turn gave, empty when it gave none, with its
/// session, or why the turn failed.
fn ended(
    sent: Result<Result<(String, Session), IntelligenceError>, tokio::task::JoinError>,
) -> Result<(String, Session), String> {
    match sent {
        Ok(Ok(ended)) => Ok(ended),
        Ok(Err(IntelligenceError::Stopped)) => Err("The user stopped it.".to_string()),
        Ok(Err(e)) => Err(AgentError::Intelligence(e).to_string()),
        Err(_) => Err(AgentError::Panicked.to_string()),
    }
}

/// A fork's session as it opens, with its prompt.
struct Opened {
    session: Session,
    prompt: String,
    /// What the thread's shared opening holds, when the fork started a
    /// thread of its own, for later forks to copy once it has run.
    opening: Option<SharedOpening>,
    /// The mark of the main thread the fork copied, when it copied it.
    copied: Option<ThreadMark>,
}

/// The recent conversation a fork that started a thread of its own opened
/// it with: the oldest and newest messages of the conversation it holds,
/// and the memories its system prompt lists.
struct SharedOpening {
    extended_from: i64,
    heard_through: i64,
    heard: HeardMemories,
}

/// What a fork's prompt is built from, whichever thread it opens on.
struct Brief<'a> {
    heading: &'a str,
    /// The opening's text, then what the fork may do.
    text: String,
    /// The message in the conversation that stands for the fork, which the
    /// prompt carries itself.
    held: Option<i64>,
    /// The agent's agenda as the fork's status lists it.
    agenda: String,
    /// The workspace the user is in, as the agent was last told.
    workspace: Option<Workspace>,
    now: Zoned,
}

/// The fork's session and its prompt. A cue's background run starts fresh
/// ([`open_background`]). Any other fork copies the agent's main thread, prepared
/// for it ([`main_thread::prepare`]), so it shares its cached prompt, with
/// `opening` after what the conversation said since that thread last heard and
/// which memories changed since; with none, it opens the main thread with the
/// recent conversation, which it shares, then the time and cues, its system
/// prompt listing the memories as they are now. What the fork adds past what it
/// copied or shared asks for the endpoint's short cache: nothing reads it back
/// once the fork ends.
async fn session(
    messenger: &AgentMessenger,
    agent_id: i64,
    run_id: i64,
    opening: &Opening,
    turn: &Arc<ForegroundTurn>,
) -> Result<Opened, AgentError> {
    let database = messenger.database();
    let model = tier_model(database, opening.start.tier())
        .await
        .map_err(AgentError::SessionConstruction)?;
    let services = InstalledServices::load(database).await?;
    let connected_services = ConnectedServices::load(database).await?;
    let tools = || {
        conversation_session::tools(
            messenger,
            agent_id,
            turn.clone(),
            services.clone(),
            connected_services.clone(),
            Branch::Fork {
                run_id,
                owed: opening.owed.clone(),
            },
        )
    };
    let brief = Brief {
        heading: &opening.heading,
        text: format!(
            "{}\n\n{}",
            opening.text,
            agent_transcript::fork_rights(turn.trace().initiative())
        ),
        held: opening.held,
        agenda: turn.agenda().hear(&agenda::load(database, agent_id).await?),
        workspace: conversation_session::user_workspace(database, agent_id).await?,
        now: Zoned::now(),
    };

    if let Start::Background(_) = opening.start {
        return open_background(messenger, agent_id, model, tools(), &brief).await;
    }
    let reason = match main_thread::prepare(messenger, agent_id, &model, None).await? {
        MainThread::Ready(mark) => {
            let forked = conversation_session::fork_session(
                messenger,
                agent_id,
                model.clone(),
                &mark,
                tools(),
            )
            .await;
            match forked {
                Ok(session) => return carry_on(database, agent_id, session, mark, &brief).await,
                Err(e) => {
                    log_error!(
                        Module::Intelligence,
                        "Agent {agent_id}: could not fork its main thread, so the fork starts a \
                         new one: {e:?}"
                    );
                    ContextRebuildReason::NoThread
                }
            }
        }
        MainThread::Rebuild(reason) => reason,
    };
    open_new(messenger, agent_id, model, tools(), reason, &brief).await
}

/// The fork carrying on `session`, a copy of the agent's main thread as
/// `mark` has it, with the prompt `brief` says after what that thread
/// hasn't heard: the conversation's messages since, and which memories
/// changed, opening with the time and cues, as whatever the thread opened with
/// may be old.
async fn carry_on(
    database: &DatabaseContext,
    agent_id: i64,
    mut session: Session,
    mark: ThreadMark,
    brief: &Brief<'_>,
) -> Result<Opened, AgentError> {
    session.share_items();
    session.cache_tail(CacheLength::Short);
    let memories = conversation_session::agent_memories(database, agent_id).await?;
    let memories = session.hear_memories(memories);
    let news: Vec<_> = conversation_session::recent(database, agent_id, None)
        .await?
        .into_iter()
        .filter(|news| news.id > mark.heard_through && Some(news.id) != brief.held)
        .collect();
    let prompt = agent_transcript::catch_up(
        memories.as_deref(),
        &news,
        &brief.text,
        Some(brief.heading),
        &brief.now,
    );
    let cues = database.send(ListAgentCuesCommand { agent_id }).await?;
    let skills = conversation_session::usable_skills(database, agent_id).await?;
    let prompt = format!(
        "{}\n\n{prompt}",
        agent_transcript::status(
            &cues,
            &skills,
            &brief.agenda,
            brief.workspace.as_ref(),
            &brief.now,
        )
    );
    Ok(Opened {
        session,
        prompt,
        opening: None,
        copied: Some(mark),
    })
}

/// A cue's background run on a new thread of `model`, offering `tools`: only the
/// time and the prompt `brief` says after its system prompt, all of it on the
/// short cache. It shares nothing and leaves no mark.
async fn open_background(
    messenger: &AgentMessenger,
    agent_id: i64,
    model: TierModel,
    tools: Vec<Box<dyn SessionTool>>,
    brief: &Brief<'_>,
) -> Result<Opened, AgentError> {
    let agent = messenger
        .database()
        .send(GetIntelligenceAgentCommand { agent_id })
        .await?;
    let mut session = conversation_session::new_session(messenger, &agent, model, tools).await?;
    session.cache_only_short();
    Ok(Opened {
        session,
        prompt: format!(
            "{}\n{}\n\n# {}\n{}",
            agent_transcript::time(&brief.now),
            brief.agenda,
            brief.heading,
            brief.text
        ),
        opening: None,
        copied: None,
    })
}

/// The fork opening the agent's main thread on `model`, offering `tools`: its
/// first item the recent conversation, which it shares for later starts, then
/// the time, the cues and the prompt `brief` says. When it replaces a thread,
/// the feed says it was rebuilt, for `reason`.
async fn open_new(
    messenger: &AgentMessenger,
    agent_id: i64,
    model: TierModel,
    tools: Vec<Box<dyn SessionTool>>,
    reason: ContextRebuildReason,
    brief: &Brief<'_>,
) -> Result<Opened, AgentError> {
    let database = messenger.database();
    let agent = database
        .send(GetIntelligenceAgentCommand { agent_id })
        .await?;
    let mut session = conversation_session::new_session(messenger, &agent, model, tools).await?;
    session.cache_tail(CacheLength::Short);
    let heard = session.heard().clone();
    let mut recent = relevant_set::load(database, agent_id, None).await?;
    // The message the fork stands for is its prompt's own.
    recent.retain(|message| Some(message.id) != brief.held);
    let heard_through = recent.last().map_or(0, |newest| newest.id);
    let extended_from = recent.first().map_or(heard_through + 1, |oldest| oldest.id);
    session
        .open_shared(agent_transcript::recent_conversation(&recent, &brief.now))
        .await
        .map_err(AgentError::Intelligence)?;
    if reason != ContextRebuildReason::NoThread {
        conversation_session::note_rebuilt(database, agent_id, reason).await?;
    }
    let cues = database.send(ListAgentCuesCommand { agent_id }).await?;
    let skills = conversation_session::usable_skills(database, agent_id).await?;
    Ok(Opened {
        session,
        prompt: format!(
            "{}\n\n# {}\n{}",
            agent_transcript::status(
                &cues,
                &skills,
                &brief.agenda,
                brief.workspace.as_ref(),
                &brief.now,
            ),
            brief.heading,
            brief.text
        ),
        opening: Some(SharedOpening {
            extended_from,
            heard_through,
            heard,
        }),
        copied: None,
    })
}

#[cfg(test)]
mod tests;
