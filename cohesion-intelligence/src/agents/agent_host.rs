use std::sync::{Arc, Mutex};

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{
    AgentMessageContent, Attachment, ContextRebuildReason, IntelligenceAgent,
    IntelligenceAgentMessage, IntelligenceAgentMessageSender, IntelligenceTier,
};
use cohesion_db::{
    CommandError, DatabaseContext, GetCandidateAgentMessagesCommand, GetIntelligenceAgentCommand,
    ListAgentCuesCommand, ListAgentMessagesCommand, TierModel,
};
use jiff::Zoned;

use super::agent_activity::Work;
use super::agent_error::AgentError;
use super::agent_messenger::AgentMessenger;
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
use crate::tools::SessionTool;
use crate::transcript_item::PromptImage;

/// One agent's foreground: its conversation with the user, held in the agent's
/// main thread ([`main_thread`]), which is opened from a compact transcript
/// when there is none to carry on.
pub(super) struct AgentHost {
    database: DatabaseContext,
    /// What the agent's sessions are built from, which carries its
    /// questions to other agents.
    messenger: AgentMessenger,
    /// The running turn, as the user reaches it while it works.
    foreground: Arc<ForegroundTurn>,
    agent: IntelligenceAgent,
    /// The messages the model's context is about, oldest first.
    working_set: Vec<IntelligenceAgentMessage>,
    session: Option<Session>,
    /// Why the next fresh thread starts, when a failed turn dropped the
    /// last one; a host with neither has none to rebuild.
    dropped_by: Option<ContextRebuildReason>,
    /// The oldest message the live thread holds: it opened with a summary
    /// from here, and what came after is its own turns.
    extended_from: i64,
    /// Forks' messages up to this id are in the live thread already.
    seen_through: i64,
    /// What the thread's `send_message` tool sent during the current turn.
    sent: Arc<Mutex<Vec<IntelligenceAgentMessage>>>,
    /// The user's message the thread's `react` tool last reacted to during
    /// the current turn.
    reacted: Arc<Mutex<Option<IntelligenceAgentMessage>>>,
}

/// What starts a foreground turn, and so how its prompt reads.
enum Trigger {
    /// The user's saved message, last in the working set: a fresh thread's
    /// status block ends with it, and a live thread gets its text.
    Message(i64),
    /// The user's pick of a choice, which posts no message: the answered
    /// choice stands last in the working set.
    Pick,
    /// The user's reaction to one of the agent's messages: the reacted
    /// message stands last in the working set.
    Reaction,
}
impl Trigger {
    /// The heading the prompt's text goes under; a message's needs none.
    fn heading(&self) -> Option<&str> {
        match self {
            Self::Message(_) => None,
            Self::Pick => Some("User's pick"),
            Self::Reaction => Some("User's reaction"),
        }
    }

    /// The user's message the turn answers: messages sent after it wait
    /// their own turns.
    fn message(&self) -> Option<i64> {
        match self {
            Self::Message(id) => Some(*id),
            Self::Pick | Self::Reaction => None,
        }
    }

    /// Whether the turn answers the user, and so must message or react to
    /// them: a reaction is feedback.
    fn answers_user(&self) -> bool {
        match self {
            Self::Message(_) | Self::Pick => true,
            Self::Reaction => false,
        }
    }
}
impl AgentHost {
    pub(super) async fn new(
        agent_id: i64,
        messenger: AgentMessenger,
        foreground: Arc<ForegroundTurn>,
    ) -> Result<Self, CommandError> {
        let database = messenger.database().clone();
        let agent = database
            .send(GetIntelligenceAgentCommand { agent_id })
            .await?;
        Ok(Self {
            agent,
            database,
            messenger,
            foreground,
            working_set: Vec::new(),
            session: None,
            dropped_by: None,
            extended_from: 0,
            seen_through: 0,
            sent: Arc::default(),
            reacted: Arc::default(),
        })
    }

    /// Runs the agent's turn on the user's saved `message`, which sees the
    /// images attached to it. A turn that fails drops the thread, so the
    /// next message rebuilds the context, and posts an `error` message saying
    /// why.
    pub(super) async fn answer_message(
        &mut self,
        message: IntelligenceAgentMessage,
        attachments: Vec<Attachment>,
        work: &Work,
    ) -> Result<(), AgentError> {
        self.working_set.push(message.clone());
        let images = attachments.into_iter().map(PromptImage::from).collect();
        let text = agent_transcript::user_text(&message);
        self.answer(Trigger::Message(message.id), &text, images, work)
            .await
    }

    /// Runs the agent's turn on the user's pick, recorded in `choice`. The
    /// pick posts no message to the feed; the choice shows it.
    pub(super) async fn hear_pick(
        &mut self,
        choice: IntelligenceAgentMessage,
        work: &Work,
    ) -> Result<(), AgentError> {
        // The pick is no message of the user's: the answered choice stands
        // in for one, last in the working set.
        self.working_set.retain(|held| held.id != choice.id);
        self.working_set.push(choice.clone());
        let AgentMessageContent::Choice(picked) = &choice.message_content else {
            return Err(AgentError::NotAChoice);
        };
        let text = agent_transcript::pick(&choice, picked);
        self.answer(Trigger::Pick, &text, Vec::new(), work).await
    }

    /// Runs the agent's turn on the user's reaction in `message`, which may
    /// post nothing.
    pub(super) async fn hear_reaction(
        &mut self,
        message: IntelligenceAgentMessage,
        work: &Work,
    ) -> Result<(), AgentError> {
        // The reaction is no message of the user's: the reacted message
        // stands in for one, last in the working set.
        self.working_set.retain(|held| held.id != message.id);
        self.working_set.push(message.clone());
        let text = agent_transcript::reaction(&message);
        self.answer(Trigger::Reaction, &text, Vec::new(), work)
            .await
    }

    /// Runs a turn on `text` and `images`. A turn that fails drops the
    /// thread, so the next message rebuilds the context, and posts an `error`
    /// message saying why.
    async fn answer(
        &mut self,
        trigger: Trigger,
        text: &str,
        images: Vec<PromptImage>,
        work: &Work,
    ) -> Result<(), AgentError> {
        let answered = self.answer_turn(trigger, text, images, work).await;
        self.mark().await;
        answered
    }

    /// Records where the conversation's thread stands, now no turn is
    /// running, for a fork to copy and a restarted daemon to carry on. A
    /// failed turn dropped its thread and leaves the mark at the last turn
    /// that ended well.
    async fn mark(&self) {
        if self.session.is_none() {
            return;
        }
        let heard_through = self
            .working_set
            .iter()
            .map(|message| message.id)
            .max()
            .unwrap_or(0)
            .max(self.seen_through);
        let mark = self
            .session
            .as_ref()
            .and_then(|session| ThreadMark::of(session, self.extended_from, heard_through));
        if let Err(e) = self.messenger.marks().set(self.agent.id, mark).await {
            log_error!(
                Module::Intelligence,
                "Agent {}: could not mark its thread: {e:?}",
                self.agent.id
            );
        }
    }

    /// Runs the turn [`answer`](Self::answer) describes.
    async fn answer_turn(
        &mut self,
        trigger: Trigger,
        text: &str,
        images: Vec<PromptImage>,
        work: &Work,
    ) -> Result<(), AgentError> {
        let turned = self.turn(trigger, text, images, work).await;
        // What the turn posted, however it ended, shows in the feed.
        self.take_sent();
        self.take_reacted();
        match turned {
            // The thread stays whole for the next message.
            Ok(()) | Err(AgentError::Intelligence(IntelligenceError::Stopped)) => Ok(()),
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Agent {}: the turn failed: {e:?}",
                    self.agent.id
                );
                // A thread that never answered may be why, so forks don't copy
                // it either.
                let answered = self.session.as_ref().and_then(Session::last_call);
                if answered.is_none() {
                    self.messenger.marks().forget(self.agent.id).await?;
                }
                self.session = None;
                self.dropped_by = Some(ContextRebuildReason::TurnFailed);
                conversation_session::forget_thread(&self.database, self.agent.id).await?;
                let error = e.post(&self.database, self.agent.id).await?;
                self.working_set.push(error);
                Ok(())
            }
        }
    }

    /// Runs the agent's turn on `text`, with `images` after it. The user
    /// sees only what the turn posts with its tools (`send_message`,
    /// `ask_choice`, `react`); the model's final reply stays in its thread.
    /// A turn that answers the user (or took in a message they sent while it
    /// worked) and ends having posted nothing is told so once and goes on,
    /// so an answer left in the final reply still reaches them.
    async fn turn(
        &mut self,
        trigger: Trigger,
        text: &str,
        images: Vec<PromptImage>,
        work: &Work,
    ) -> Result<(), AgentError> {
        // The main thread decides: this turn carries it on as it stands, or
        // builds a new one from the database.
        let model = tier_model(&self.database, IntelligenceTier::Agent)
            .await
            .map_err(AgentError::SessionConstruction)?;
        let failed = self.dropped_by.take();
        let main = main_thread::prepare(&self.messenger, self.agent.id, &model, failed).await?;
        let (rebuilt, moved) = match main {
            MainThread::Rebuild(reason) => {
                self.session = None;
                (Some(failed.unwrap_or(reason)), false)
            }
            MainThread::Ready(mark) => {
                let moved = !self.holds(&mark);
                if moved {
                    self.session = None;
                }
                let rebuilt = match moved {
                    true => self.resume(mark, &model).await?,
                    false => None,
                };
                (rebuilt, moved)
            }
        };

        // Messages sent since this one wait their own turns.
        let through = trigger.message();
        if self.session.is_none() {
            self.working_set = relevant_set::load(&self.database, self.agent.id, through).await?;
        }
        let heading = trigger.heading().map(str::to_string);
        let prompt = match (rebuilt, heading) {
            (Some(reason), None) => self.start_session(model, through, reason).await?,
            (Some(reason), Some(heading)) => format!(
                "{}\n\n# {heading}\n{text}",
                self.start_session(model, None, reason).await?
            ),
            (None, heading) => {
                // The thread's system prompt stays as it began; it hears
                // what was remembered and forgotten since it last heard.
                let memories =
                    conversation_session::agent_memories(&self.database, self.agent.id).await?;
                let changed = self
                    .session
                    .as_mut()
                    .expect("a live thread")
                    .hear_memories(memories);
                // A failed turn's work is in the feed, not the thread.
                let prompt = self
                    .catch_up(&trigger, changed.as_deref(), text, heading.as_deref())
                    .await?;
                if moved {
                    // Its opening's status may be days old.
                    let cues = self
                        .database
                        .send(ListAgentCuesCommand {
                            agent_id: self.agent.id,
                        })
                        .await?;
                    format!(
                        "{}\n\n{prompt}",
                        agent_transcript::status(&cues, &Zoned::now())
                    )
                } else {
                    prompt
                }
            }
        };

        let session = self.session.as_mut().expect("a thread was just started");
        session
            .send_steerable(prompt, images, work, &*self.foreground)
            .await
            .map_err(AgentError::Intelligence)?;
        let answers_user = trigger.answers_user() || self.foreground.has_joined();
        let posted = !self.sent.lock().expect("sent is never poisoned").is_empty()
            || self
                .reacted
                .lock()
                .expect("reacted is never poisoned")
                .is_some();
        if answers_user && !posted {
            session
                .send_steerable(
                    agent_transcript::UNANSWERED.to_string(),
                    Vec::new(),
                    work,
                    &*self.foreground,
                )
                .await
                .map_err(AgentError::Intelligence)?;
        }
        Ok(())
    }

    /// Whether the live thread is the main thread as `mark` has it.
    fn holds(&self, mark: &ThreadMark) -> bool {
        self.session.as_ref().is_some_and(|session| {
            session.thread() == mark.thread_id && session.items() == mark.items
        })
    }

    /// Carries the conversation on from the main thread as `mark` has it, as
    /// a new host does after a restart or a host whose thread a fork moved on,
    /// in a copy of what the mark holds (so nothing a turn the daemon stopped
    /// during added comes with it). Returns why a fresh thread starts instead,
    /// if the copy can't be made.
    async fn resume(
        &mut self,
        mark: ThreadMark,
        model: &TierModel,
    ) -> Result<Option<ContextRebuildReason>, AgentError> {
        let agent_id = self.agent.id;
        let tools = self.tools().await?;
        let session = conversation_session::fork_session(
            &self.messenger,
            agent_id,
            model.clone(),
            &mark,
            tools,
        )
        .await;
        let session = match session {
            Ok(session) => session.reporting_to(agent_id),
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not carry on its marked thread, so it starts a \
                     new one: {e:?}"
                );
                return Ok(Some(ContextRebuildReason::NoThread));
            }
        };
        self.session = Some(session);
        self.seen_through = mark.heard_through;
        self.extended_from = mark.extended_from;
        // The thread holds the conversation from its extended context
        // through what it last heard; the trigger stays last.
        let trigger = self.working_set.pop();
        let mut held = self
            .database
            .send(GetCandidateAgentMessagesCommand { agent_id })
            .await?;
        held.retain(|message| {
            (mark.extended_from..=mark.heard_through).contains(&message.id)
                && Some(message.id) != trigger.as_ref().map(|trigger| trigger.id)
        });
        held.sort_by_key(|message| message.id);
        held.extend(trigger);
        self.working_set = held;
        Ok(None)
    }

    /// The conversation's tools, as every thread of it is sent them.
    async fn tools(&self) -> Result<Vec<Box<dyn SessionTool>>, AgentError> {
        Ok(conversation_session::tools(
            &self.messenger,
            self.agent.id,
            self.foreground.clone(),
            InstalledServices::load(&self.database).await?,
            ConnectedServices::load(&self.database).await?,
            Branch::Conversation {
                sent: self.sent.clone(),
                reacted: self.reacted.clone(),
            },
        ))
    }

    /// Starts a new thread on `model` and returns its first message: the
    /// status message over the working set, ending with how to answer the
    /// user's message `answering`, if the turn answers one. When the
    /// summary holds more than the turn's own trigger, the conversation is
    /// being rebuilt rather than begun, and a `rebuiltContext` line in
    /// the feed says so.
    async fn start_session(
        &mut self,
        model: TierModel,
        answering: Option<i64>,
        reason: ContextRebuildReason,
    ) -> Result<String, AgentError> {
        let agent_id = self.agent.id;
        self.agent = self
            .database
            .send(GetIntelligenceAgentCommand { agent_id })
            .await?;
        let cues = self
            .database
            .send(ListAgentCuesCommand { agent_id })
            .await?;

        let tools = self.tools().await?;
        let session = conversation_session::new_session(&self.messenger, &self.agent, model, tools)
            .await?
            .reporting_to(agent_id);
        self.session = Some(session);
        self.seen_through = self.working_set.last().map_or(0, |message| message.id);
        self.begin_thread().await?;
        if self.working_set.len() > 1 {
            conversation_session::note_rebuilt(&self.database, agent_id, reason).await?;
        }

        Ok(agent_transcript::status_message(
            &cues,
            &self.working_set,
            answering,
            &Zoned::now(),
        ))
    }

    /// Notes that the thread just started holds the working set as its
    /// status message's lines, and every message after the newest of them,
    /// which a later prompt brings, as its own turns; the thread has no
    /// token count until its first request is answered.
    async fn begin_thread(&mut self) -> Result<(), CommandError> {
        let agent_id = self.agent.id;
        self.extended_from = match self.working_set.first() {
            Some(oldest) => oldest.id,
            // A status message with no lines holds nothing sent so far.
            None => {
                self.database
                    .send(ListAgentMessagesCommand {
                        agent_id,
                        before: None,
                        limit: 1,
                    })
                    .await?
                    .last()
                    .map_or(0, |newest| newest.id)
                    + 1
            }
        };
        conversation_session::forget_thread(&self.database, agent_id).await
    }

    /// The prompt for a live thread: `text`, under `heading` when it is no
    /// message of the user's, after `memories`, which memories changed, and
    /// anything forks posted since the
    /// thread last heard from the user, up to the message `trigger` answers (or
    /// now, for any other trigger).
    async fn catch_up(
        &mut self,
        trigger: &Trigger,
        memories: Option<&str>,
        text: &str,
        heading: Option<&str>,
    ) -> Result<String, AgentError> {
        let before = trigger.message();
        let recent = conversation_session::recent(&self.database, self.agent.id, before).await?;
        let newest = recent.last().map_or(self.seen_through, |recent| recent.id);
        // The trigger, last in the working set, is the prompt's own text.
        let held = self.working_set.last().map(|held| held.id);
        let news: Vec<_> = recent
            .into_iter()
            .filter(|recent| {
                recent.id > self.seen_through && Some(recent.id) != held && is_news(recent)
            })
            .collect();
        self.seen_through = before.unwrap_or(newest).max(self.seen_through);

        // The news goes before the trigger.
        let at = self.working_set.len() - usize::from(held.is_some());
        self.working_set.splice(at..at, news.iter().cloned());
        Ok(agent_transcript::catch_up(
            memories,
            &news,
            text,
            heading,
            &Zoned::now(),
        ))
    }

    /// The user's message the agent reacted to this turn, which replaces its
    /// copy in the working set.
    fn take_reacted(&mut self) -> Option<IntelligenceAgentMessage> {
        let reacted = self
            .reacted
            .lock()
            .expect("reacted is never poisoned")
            .take()?;
        for held in &mut self.working_set {
            if held.id == reacted.id {
                *held = reacted.clone();
            }
        }
        Some(reacted)
    }

    /// Moves the messages `send_message` sent this turn, and the user's the
    /// turn took in as it ran, into the working set in the order they came.
    fn take_sent(&mut self) {
        let mut came = std::mem::take(&mut *self.sent.lock().expect("sent is never poisoned"));
        came.extend(self.foreground.take_joined());
        came.sort_by_key(|message| message.id);
        self.working_set.extend(came);
    }
}

/// Whether a live thread hears of `message` in its next catch-up: what a
/// fork posted (any message with a run, and an older cue run's); the rest
/// the thread said or heard itself.
pub(super) fn is_news(message: &IntelligenceAgentMessage) -> bool {
    message.sender == IntelligenceAgentMessageSender::BackgroundAgent || message.run_id.is_some()
}

#[cfg(test)]
mod tests;
