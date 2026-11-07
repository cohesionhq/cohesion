use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{HookKind, IntelligenceAgentCue, IntelligenceEffort},
    protocol::ListAgentCuesRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::{
    AgentSettings, CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Write,
};

/// Sets one of an agent's cues, replacing the cue it already has by that
/// name; either way the cue counts as set now.
pub struct SetAgentCueCommand {
    pub agent_id: i64,
    pub name: String,
    pub hook: HookKind,
    pub script: String,
    pub instructions: String,
    pub effort: IntelligenceEffort,
}
impl DatabaseCommand for SetAgentCueCommand {
    type Response = IntelligenceAgentCue;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let cue: IntelligenceAgentCue = ctx
            .db
            .insert()
            .set(IntelligenceAgentCue::AGENT_ID, self.agent_id)
            .set(IntelligenceAgentCue::NAME, self.name)
            .set(IntelligenceAgentCue::HOOK, self.hook)
            .set(IntelligenceAgentCue::SCRIPT, self.script)
            .set(IntelligenceAgentCue::INSTRUCTIONS, self.instructions)
            .set(IntelligenceAgentCue::EFFORT, self.effort)
            .set_now(IntelligenceAgentCue::SET_AT)
            .on_conflict_update(&["agent_id", "name"])
            .returning_one()
            .found()?;
        Ok(cue.invalidating([Invalidation::Agent(self.agent_id)]))
    }
}

/// Removes a cue, returning it.
pub struct DeleteAgentCueCommand {
    pub cue_id: i64,
}
impl DatabaseCommand for DeleteAgentCueCommand {
    type Response = IntelligenceAgentCue;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let cue: IntelligenceAgentCue = ctx
            .db
            .delete()
            .filter_eq(IntelligenceAgentCue::ID, self.cue_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(cue.agent_id);
        Ok(cue.invalidating([key]))
    }
}

/// An agent's cues, oldest first.
#[derive(FromRequest)]
#[request(ListAgentCuesRequest)]
pub struct ListAgentCuesCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for ListAgentCuesCommand {
    type Response = Vec<IntelligenceAgentCue>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("agent_id = ?")
            .bind(self.agent_id)
            .order("id")
            .all()?)
    }
}

/// Every cue on `hook`, oldest agent first.
pub struct ListAgentCuesForHookCommand {
    pub hook: HookKind,
}
impl DatabaseCommand for ListAgentCuesForHookCommand {
    type Response = Vec<IntelligenceAgentCue>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("hook = ?")
            .bind(self.hook)
            .order("agent_id, id")
            .all()?)
    }
}

#[cfg(test)]
mod tests;
