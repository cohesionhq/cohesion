use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{Memory, MemoryScope},
    protocol::{ForgetMemoryRequest, ListMemoriesRequest},
};

use crate::changed::{Changed, Invalidates};
use crate::{
    AgentSettings, CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    GetStorageSettingsCommand, Read, ReadableDatabase, ValidationError, Write,
};

/// Keeps a memory in `scope`, which keeps at most the storage settings'
/// `memory_limit`: past it, remembering another in that scope is refused
/// until one is forgotten.
pub struct CreateMemoryCommand {
    pub scope: MemoryScope,
    pub memory: String,
}
impl DatabaseCommand for CreateMemoryCommand {
    type Response = Memory;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let memory = self.memory.trim();
        if memory.is_empty() {
            return Err(ValidationError::new("A memory needs some text.").into());
        }
        let in_scope = ctx.db.select::<Memory>();
        let kept = match self.scope {
            MemoryScope::Global => in_scope.filter("agent_id IS NULL AND team_id IS NULL"),
            MemoryScope::Team { team_id } => in_scope.filter("team_id = ?").bind(team_id),
            MemoryScope::Agent { agent_id } => in_scope.filter("agent_id = ?").bind(agent_id),
        }
        .count()?;
        let limit = GetStorageSettingsCommand {}
            .handle(&mut ctx.as_read())?
            .memory_limit;
        if kept >= limit {
            let whose = match self.scope {
                MemoryScope::Global => "global",
                MemoryScope::Team { .. } => "team",
                MemoryScope::Agent { .. } => "personal",
            };
            return Err(ValidationError {
                field: None,
                message: format!(
                    "There are already {kept} {whose} memories, and that scope keeps \
                     at most {limit}. Compact them before remembering another: forget memories \
                     about the same thing and remember one sentence holding them all, or \
                     forget one that matters less."
                ),
            }
            .into());
        }
        let memory: Memory = ctx
            .db
            .insert()
            .set_group(self.scope)
            .set(Memory::MEMORY, memory.to_string())
            .returning_one()
            .found()?;
        Ok(memory.invalidating([Invalidation::Memories]))
    }
}

/// Forgets a memory, returning it.
#[derive(FromRequest)]
#[request(ForgetMemoryRequest)]
pub struct DeleteMemoryCommand {
    pub memory_id: i64,
}
impl DatabaseCommand for DeleteMemoryCommand {
    type Response = Memory;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let memory: Memory = ctx
            .db
            .delete()
            .filter_eq(Memory::ID, self.memory_id)
            .returning_one()
            .found()?;
        Ok(memory.invalidating([Invalidation::Memories]))
    }
}

/// The memories agent `agent_id` sees: its own, then its team's, then the
/// global ones, each oldest first. Only the global ones when `None`.
#[derive(FromRequest)]
#[request(ListMemoriesRequest)]
pub struct ListMemoriesCommand {
    pub agent_id: Option<i64>,
}
impl DatabaseCommand for ListMemoriesCommand {
    type Response = Vec<Memory>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let query = match self.agent_id {
            Some(agent_id) => ctx
                .db
                .select()
                .filter(
                    "agent_id = ? OR (agent_id IS NULL AND team_id IS NULL) \
                     OR team_id = (SELECT team_id FROM intelligence_agents WHERE id = ?)",
                )
                .bind(agent_id)
                .bind(agent_id),
            None => ctx
                .db
                .select()
                .filter("agent_id IS NULL AND team_id IS NULL"),
        };
        Ok(query.order("agent_id IS NULL, team_id IS NULL, id").all()?)
    }
}

#[cfg(test)]
mod tests;
