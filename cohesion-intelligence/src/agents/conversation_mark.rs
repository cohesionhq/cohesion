//! Where each agent's main thread stood when its last turn ended, which
//! every foreground fork copies: a fork never waits on the conversation's lock,
//! and never copies a turn still under way. The marks are kept in the
//! database, so a restarted daemon carries a thread on while its provider
//! still holds the prompt cached rather than paying to write it again.

use std::time::Duration;

use cohesion_core::model::{AgentThreadMark, ContextRebuildReason};
use cohesion_db::{
    CommandError, DatabaseContext, DeleteAgentThreadMarkCommand,
    LookupAgentThreadMarkByAgentCommand, RefreshAgentThreadMarkCommand, SetAgentThreadMarkCommand,
};
use jiff::Timestamp;

use crate::session::{LastCall, Session};
use crate::system_prompt::HeardMemories;

/// The prompt, in tokens as the provider counted them, past which a thread
/// is rebuilt rather than carried on.
const CONTEXT_LIMIT_TOKENS: u64 = 100_000;

/// What a thread is good for at its next turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fitness {
    /// Its cache hasn't expired, so its prompt is there to read back.
    CarryOn,
    /// It can't carry on, for this reason: its cache has expired, its prompt has
    /// grown too long, or its model or memory capacity changed. Its context is
    /// rebuilt from the database.
    Rebuild(ContextRebuildReason),
}

/// An agent's conversation thread as its last turn left it.
#[derive(Clone)]
pub(super) struct ThreadMark {
    pub thread_id: i64,
    /// How many items the thread held then, ending with a whole exchange.
    pub items: usize,
    /// The endpoint model the thread runs on.
    pub model_id: i64,
    /// How many memories each scope keeps, as the thread's system prompt says.
    pub memory_limit: i64,
    /// The oldest message of the agent's conversation the thread holds:
    /// everything from here through `heard_through` is in it, unless relevance
    /// or age left its line out, and earlier messages are out.
    pub extended_from: i64,
    /// The newest message of the agent's conversation the thread had been
    /// told of (never one merely in the database by then, which a turn
    /// running meanwhile may have posted); a fork catches up on those after
    /// it.
    pub heard_through: i64,
    /// The memories the thread last heard of, which a fork hears what
    /// changed since against.
    pub heard: HeardMemories,
    /// The latest request that read the thread's cached prompt back: the
    /// thread's own, or a later fork's that copied it as it stands.
    last_call: LastCall,
    cache_ttl: Duration,
}

impl ThreadMark {
    /// `session`'s thread as it stands, having heard of the conversation's
    /// messages through `heard_through`; `None` before its first model
    /// call, when nothing of it is cached.
    pub(super) fn of(session: &Session, extended_from: i64, heard_through: i64) -> Option<Self> {
        Some(Self {
            thread_id: session.thread(),
            items: session.items(),
            model_id: session.model().id,
            memory_limit: session.memory_limit(),
            extended_from,
            heard_through,
            heard: session.heard().clone(),
            last_call: session.last_call()?,
            cache_ttl: session.cache_ttl(),
        })
    }

    /// The items `session`'s thread shares, the recent conversation a fork
    /// opened it with, which hold the conversation's messages through
    /// `heard_through` and were sent having heard of `heard`. They stay warm as
    /// long as the endpoint's long cache keeps them past the thread's latest
    /// call, each of which read them back; `None` before its first call.
    pub(super) fn shared(
        session: &Session,
        extended_from: i64,
        heard_through: i64,
        heard: HeardMemories,
    ) -> Option<Self> {
        Some(Self {
            thread_id: session.thread(),
            items: session.shared_items(),
            model_id: session.model().id,
            memory_limit: session.memory_limit(),
            extended_from,
            heard_through,
            heard,
            last_call: session.last_call()?,
            cache_ttl: session.shared_cache_ttl(),
        })
    }

    /// What the thread is good for at `now` on model `model_id`, with each
    /// scope keeping `memory_limit` memories: rebuilt if either changed,
    /// otherwise as [`fitness_of`] says. The one place a thread's fitness is
    /// decided, for the conversation's own turns and its forks alike.
    pub(super) fn fitness(&self, model_id: i64, memory_limit: i64, now: Timestamp) -> Fitness {
        if self.model_id != model_id {
            return Fitness::Rebuild(ContextRebuildReason::ModelChanged);
        }
        if self.memory_limit != memory_limit {
            return Fitness::Rebuild(ContextRebuildReason::MemoryCapacityChanged);
        }
        fitness_of(self.last_call, self.cache_ttl, now)
    }

    fn from_row(row: AgentThreadMark) -> Self {
        Self {
            thread_id: row.thread_id,
            items: row.items as usize,
            model_id: row.endpoint_model_id,
            memory_limit: row.memory_limit,
            extended_from: row.extended_from,
            heard_through: row.heard_through,
            heard: HeardMemories(row.memories),
            last_call: LastCall {
                sent_at: Timestamp::from_millisecond(row.last_call_at)
                    .unwrap_or(Timestamp::UNIX_EPOCH),
                prompt_tokens: row.prompt_tokens as u64,
            },
            cache_ttl: Duration::from_secs(row.cache_seconds as u64),
        }
    }

    fn into_command(self, agent_id: i64) -> SetAgentThreadMarkCommand {
        SetAgentThreadMarkCommand {
            agent_id,
            thread_id: self.thread_id,
            items: self.items as i64,
            endpoint_model_id: self.model_id,
            memory_limit: self.memory_limit,
            extended_from: self.extended_from,
            heard_through: self.heard_through,
            memories: self.heard.0,
            last_call_at: self.last_call.sent_at.as_millisecond(),
            prompt_tokens: self.last_call.prompt_tokens as i64,
            cache_seconds: self.cache_ttl.as_secs() as i64,
        }
    }
}

/// What a thread whose latest model call was `call` is good for at `now`:
/// rebuilt once its prompt passed [`CONTEXT_LIMIT_TOKENS`] or its provider has
/// dropped the prompt cached `cache_ttl` past that call.
fn fitness_of(call: LastCall, cache_ttl: Duration, now: Timestamp) -> Fitness {
    if call.prompt_tokens > CONTEXT_LIMIT_TOKENS {
        Fitness::Rebuild(ContextRebuildReason::ContextLimit)
    } else if call.cache_expired(cache_ttl, now) {
        Fitness::Rebuild(ContextRebuildReason::CacheExpired)
    } else {
        Fitness::CarryOn
    }
}

/// Each agent's [`ThreadMark`], kept in the database across hosts and
/// restarts: its main thread ([`main_thread`](super::main_thread)), while it
/// has one.
#[derive(Clone)]
pub(super) struct ConversationMarks {
    database: DatabaseContext,
}

impl ConversationMarks {
    pub(super) fn new(database: DatabaseContext) -> Self {
        Self { database }
    }

    /// Records where the agent's main thread stands, or that it has none.
    pub(super) async fn set(
        &self,
        agent_id: i64,
        mark: Option<ThreadMark>,
    ) -> Result<(), CommandError> {
        match mark {
            Some(mark) => self.database.send(mark.into_command(agent_id)).await,
            None => self.forget(agent_id).await,
        }
    }

    pub(super) async fn get(&self, agent_id: i64) -> Result<Option<ThreadMark>, CommandError> {
        let row = self
            .database
            .send(LookupAgentThreadMarkByAgentCommand { agent_id })
            .await?;
        Ok(row.map(ThreadMark::from_row))
    }

    /// Records that a request sent at `read_at` read the agent's main thread
    /// back as `mark` has it, which kept its cache alive from then, unless the
    /// mark has moved on since.
    pub(super) async fn refresh(
        &self,
        agent_id: i64,
        mark: &ThreadMark,
        read_at: Timestamp,
    ) -> Result<(), CommandError> {
        self.database
            .send(RefreshAgentThreadMarkCommand {
                agent_id,
                thread_id: mark.thread_id,
                items: mark.items as i64,
                read_at: read_at.as_millisecond(),
            })
            .await
    }

    /// Forgets the agent's mark, once it changed.
    pub(super) async fn forget(&self, agent_id: i64) -> Result<(), CommandError> {
        self.database
            .send(DeleteAgentThreadMarkCommand { agent_id })
            .await
    }
}

#[cfg(test)]
mod tests;
