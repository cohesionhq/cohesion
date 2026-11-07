//! Where each agent's main thread stood when its last turn ended
//! ([`AgentThreadMark`]), kept across restarts so a thread whose prompt its
//! provider still holds cached is carried on rather than written again.

use cohesion_core::model::{AgentThreadMark, Memory};
use cohesion_core::value::Json;

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Read, ReadableDatabase, Write,
};

/// Records where agent `agent_id`'s main thread stands, in place of where it
/// stood before.
pub struct SetAgentThreadMarkCommand {
    pub agent_id: i64,
    pub thread_id: i64,
    pub items: i64,
    pub endpoint_model_id: i64,
    pub extended_from: i64,
    pub heard_through: i64,
    pub memory_limit: i64,
    pub memories: Vec<Memory>,
    pub last_call_at: i64,
    pub prompt_tokens: i64,
    pub cache_seconds: i64,
}
impl DatabaseCommand for SetAgentThreadMarkCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .insert::<AgentThreadMark>()
            .set(AgentThreadMark::AGENT_ID, self.agent_id)
            .set(AgentThreadMark::THREAD_ID, self.thread_id)
            .set(AgentThreadMark::ITEMS, self.items)
            .set(AgentThreadMark::ENDPOINT_MODEL_ID, self.endpoint_model_id)
            .set(AgentThreadMark::EXTENDED_FROM, self.extended_from)
            .set(AgentThreadMark::HEARD_THROUGH, self.heard_through)
            .set(AgentThreadMark::MEMORY_LIMIT, self.memory_limit)
            .set(AgentThreadMark::MEMORIES, Json(self.memories))
            .set(AgentThreadMark::LAST_CALL_AT, self.last_call_at)
            .set(AgentThreadMark::PROMPT_TOKENS, self.prompt_tokens)
            .set(AgentThreadMark::CACHE_SECONDS, self.cache_seconds)
            .on_conflict_update(&["agent_id"])
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Agent `agent_id`'s thread mark, if it has one.
pub struct LookupAgentThreadMarkByAgentCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for LookupAgentThreadMarkByAgentCommand {
    type Response = Option<AgentThreadMark>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter_eq(AgentThreadMark::AGENT_ID, self.agent_id)
            .one()?)
    }
}

/// Records that a request sent at `read_at` (Unix milliseconds) read agent
/// `agent_id`'s main thread back as its mark has it, `items` items of
/// thread `thread_id`, which keeps the provider's cache of it alive from
/// then. A mark that has moved on since, or was read later, is left alone.
pub struct RefreshAgentThreadMarkCommand {
    pub agent_id: i64,
    pub thread_id: i64,
    pub items: i64,
    pub read_at: i64,
}
impl DatabaseCommand for RefreshAgentThreadMarkCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .update::<AgentThreadMark>()
            .set(AgentThreadMark::LAST_CALL_AT, self.read_at)
            .filter_eq(AgentThreadMark::AGENT_ID, self.agent_id)
            .filter_eq(AgentThreadMark::THREAD_ID, self.thread_id)
            .filter_eq(AgentThreadMark::ITEMS, self.items)
            .filter("last_call_at < ?")
            .bind(self.read_at)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Forgets agent `agent_id`'s thread mark.
pub struct DeleteAgentThreadMarkCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for DeleteAgentThreadMarkCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<AgentThreadMark>()
            .filter_eq(AgentThreadMark::AGENT_ID, self.agent_id)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Forgets every mark a turn the daemon stopped during had
/// moved past, part of [`ShutdownCleanupCommand`](crate::ShutdownCleanupCommand):
/// a turn's end marks its thread, so a thread holding more than its mark
/// says was cut short mid-turn, and carrying on from the mark would leave
/// out what that turn heard and did. The next turn rebuilds the thread
/// from the feed instead. Runs before
/// [`SettleAgentActivityCommand`](crate::SettleAgentActivityCommand), which
/// keeps the context tokens of the agents left with a mark.
pub struct SettleAgentThreadMarksCommand;
impl DatabaseCommand for SettleAgentThreadMarksCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<AgentThreadMark>()
            .filter(
                "items < (SELECT coalesce(thread.inherited_items, 0)
                        FROM intelligence_threads AS thread
                        WHERE thread.id = agent_thread_marks.thread_id)
                    + (SELECT COUNT(*) FROM intelligence_thread_messages AS message
                        WHERE message.thread_id = agent_thread_marks.thread_id)",
            )
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
