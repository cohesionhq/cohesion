//! An agent's main thread: the conversation's thread as its [`ThreadMark`]
//! has it, the one thread its turns carry on and its foreground forks copy.
//! Whatever starts next, a chat message or a foreground wake, asks [`prepare`]
//! for it first, so the one decision of what happens to it lives here: reuse it
//! while it is hot, otherwise rebuild the context from the database. A cue's
//! background run never touches it.

use cohesion_core::model::ContextRebuildReason;
use cohesion_db::{CommandError, GetStorageSettingsCommand, TierModel};
use jiff::Timestamp;

use super::agent_messenger::AgentMessenger;
use super::conversation_mark::{Fitness, ThreadMark};

/// What the next start of an agent's conversation or fork has to work from.
pub(super) enum MainThread {
    /// The thread to carry on or copy.
    Ready(ThreadMark),
    /// It can't carry on, for this reason, so the next start builds a new
    /// thread from the database.
    Rebuild(ContextRebuildReason),
}

/// Agent `agent_id`'s main thread for a start on `model`. A turn that just
/// failed on it passes why as `failed`, which rebuilds it however hot it is.
pub(super) async fn prepare(
    messenger: &AgentMessenger,
    agent_id: i64,
    model: &TierModel,
    failed: Option<ContextRebuildReason>,
) -> Result<MainThread, CommandError> {
    let Some(mark) = messenger.marks().get(agent_id).await? else {
        return Ok(MainThread::Rebuild(ContextRebuildReason::NoThread));
    };
    if let Some(reason) = failed {
        return Ok(MainThread::Rebuild(reason));
    }
    let memory_limit = messenger
        .database()
        .send(GetStorageSettingsCommand {})
        .await?
        .memory_limit;
    Ok(
        match mark.fitness(model.model.id, memory_limit, Timestamp::now()) {
            Fitness::CarryOn => MainThread::Ready(mark),
            Fitness::Rebuild(reason) => MainThread::Rebuild(reason),
        },
    )
}
