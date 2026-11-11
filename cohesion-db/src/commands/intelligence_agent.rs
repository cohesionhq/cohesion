use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{
        AgentThreadMark, IntelligenceAgent, IntelligenceAgentColor, IntelligenceAgentPersonality,
        IntelligenceAgentRun, IntelligenceAgentRunOutcome, IntelligenceAgentRunState,
        IntelligenceAgentVerbosity,
    },
    protocol::{
        CreateIntelligenceAgentRequest, DeleteIntelligenceAgentRequest,
        GetIntelligenceAgentRequest, ListIntelligenceAgentsRequest, MarkAgentReadRequest,
        ReorderIntelligenceAgentsRequest, SetAgentPinnedRequest, UpdateIntelligenceAgentRequest,
    },
};

use super::{agent_name::agent_name, emoji::emoji};
use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write,
};

/// Creates an agent, on team `team_id` if given. A blank or absent name
/// picks one no other agent uses.
#[derive(FromRequest)]
#[request(CreateIntelligenceAgentRequest)]
pub struct CreateIntelligenceAgentCommand {
    pub name: Option<String>,
    pub color: IntelligenceAgentColor,
    pub emoji: Option<String>,
    pub personality: Option<IntelligenceAgentPersonality>,
    pub verbosity: Option<IntelligenceAgentVerbosity>,
    pub role: Option<String>,
    pub directive: Option<String>,
    pub team_id: Option<i64>,
}
impl DatabaseCommand for CreateIntelligenceAgentCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let name = match self.name.map(|n| n.trim().to_string()) {
            Some(name) if !name.is_empty() => name,
            _ => {
                let taken: Vec<String> = ListIntelligenceAgentsCommand {}
                    .handle(&mut ctx.as_read())?
                    .into_iter()
                    .map(|agent| agent.agent_name)
                    .collect();
                agent_name(&taken, ctx.now)
            }
        };
        let agent: IntelligenceAgent = ctx
            .db
            .insert()
            .set(IntelligenceAgent::AGENT_NAME, name)
            .set(IntelligenceAgent::AGENT_COLOR, self.color)
            .set(IntelligenceAgent::AGENT_EMOJI, emoji(self.emoji)?)
            .set(
                IntelligenceAgent::AGENT_PERSONALITY,
                self.personality.unwrap_or_default(),
            )
            .set(
                IntelligenceAgent::AGENT_VERBOSITY,
                self.verbosity.unwrap_or_default(),
            )
            .set(IntelligenceAgent::AGENT_ROLE, trimmed(self.role))
            .set(IntelligenceAgent::AGENT_DIRECTIVE, trimmed(self.directive))
            .set(IntelligenceAgent::TEAM_ID, self.team_id)
            .set_max_incremented(IntelligenceAgent::SORT_ORDER)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

/// `text` without the whitespace around it; `""` when absent.
fn trimmed(text: Option<String>) -> String {
    text.as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

/// Every agent: the pinned ones in their sort order, then the rest by their
/// latest message (or, with none, when they were created), newest first.
#[derive(FromRequest)]
#[request(ListIntelligenceAgentsRequest)]
pub struct ListIntelligenceAgentsCommand {}
impl DatabaseCommand for ListIntelligenceAgentsCommand {
    type Response = Vec<IntelligenceAgent>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .order(
                "pinned DESC, CASE WHEN pinned THEN sort_order END, \
                 COALESCE(last_message_at, created_at) DESC, id DESC",
            )
            .all()?)
    }
}

/// Gives each pinned agent in `agent_ids` its position in the list as its
/// sort order.
#[derive(FromRequest)]
#[request(ReorderIntelligenceAgentsRequest)]
pub struct ReorderIntelligenceAgentsCommand {
    pub agent_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderIntelligenceAgentsCommand {
    type Response = Vec<IntelligenceAgent>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let mut keys = Vec::new();
        for (position, agent_id) in self.agent_ids.into_iter().enumerate() {
            ctx.db
                .update::<IntelligenceAgent>()
                .set(IntelligenceAgent::SORT_ORDER, position as i64)
                .filter_eq(IntelligenceAgent::ID, agent_id)
                .returning_one()
                .found()?;
            keys.push(Invalidation::Agent(agent_id));
        }
        Ok(ListIntelligenceAgentsCommand {}
            .handle(&mut ctx.as_read())?
            .invalidating(keys))
    }
}

/// Pins or unpins an agent. A newly pinned agent goes after the other
/// pinned agents; unpinning leaves its sort order, which only pinned agents use.
#[derive(FromRequest)]
#[request(SetAgentPinnedRequest)]
pub struct SetAgentPinnedCommand {
    pub agent_id: i64,
    pub pinned: bool,
}
impl DatabaseCommand for SetAgentPinnedCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agents = ListIntelligenceAgentsCommand {}.handle(&mut ctx.as_read())?;
        let last = agents
            .iter()
            .filter(|agent| agent.pinned && agent.id != self.agent_id)
            .map(|agent| agent.sort_order)
            .max()
            .unwrap_or(0);
        let mut update = ctx
            .db
            .update()
            .set(IntelligenceAgent::PINNED, self.pinned)
            .filter_eq(IntelligenceAgent::ID, self.agent_id);
        if self.pinned {
            update = update.set(IntelligenceAgent::SORT_ORDER, last + 1);
        }
        let agent: IntelligenceAgent = update.returning_one().found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

#[derive(FromRequest)]
#[request(GetIntelligenceAgentRequest)]
pub struct GetIntelligenceAgentCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for GetIntelligenceAgentCommand {
    type Response = IntelligenceAgent;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select()
            .filter("id = ?")
            .bind(self.agent_id)
            .one()
            .found()
    }
}

/// Changes an agent's name, color, emoji, personality or verbosity; `None`
/// leaves a field alone, and a blank emoji clears it.
#[derive(FromRequest)]
#[request(UpdateIntelligenceAgentRequest)]
pub struct UpdateIntelligenceAgentCommand {
    pub agent_id: i64,
    pub name: Option<String>,
    pub color: Option<IntelligenceAgentColor>,
    pub emoji: Option<String>,
    pub personality: Option<IntelligenceAgentPersonality>,
    pub verbosity: Option<IntelligenceAgentVerbosity>,
    pub role: Option<String>,
    pub directive: Option<String>,
}
impl DatabaseCommand for UpdateIntelligenceAgentCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent = GetIntelligenceAgentCommand {
            agent_id: self.agent_id,
        }
        .handle(&mut ctx.as_read())?;
        let agent_emoji = match self.emoji {
            Some(new) => emoji(Some(new))?,
            None => agent.agent_emoji,
        };
        let agent: IntelligenceAgent = ctx
            .db
            .update()
            .set(
                IntelligenceAgent::AGENT_NAME,
                self.name.unwrap_or(agent.agent_name),
            )
            .set(
                IntelligenceAgent::AGENT_COLOR,
                self.color.unwrap_or(agent.agent_color),
            )
            .set(IntelligenceAgent::AGENT_EMOJI, agent_emoji)
            .set(
                IntelligenceAgent::AGENT_PERSONALITY,
                self.personality.unwrap_or(agent.agent_personality),
            )
            .set(
                IntelligenceAgent::AGENT_VERBOSITY,
                self.verbosity.unwrap_or(agent.agent_verbosity),
            )
            .set(
                IntelligenceAgent::AGENT_ROLE,
                self.role
                    .map_or(agent.agent_role, |role| role.trim().to_string()),
            )
            .set(
                IntelligenceAgent::AGENT_DIRECTIVE,
                self.directive.map_or(agent.agent_directive, |directive| {
                    directive.trim().to_string()
                }),
            )
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

/// Deletes an agent; its cues, runs, messages and personal memories go
/// with it.
#[derive(FromRequest)]
#[request(DeleteIntelligenceAgentRequest)]
pub struct DeleteIntelligenceAgentCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for DeleteIntelligenceAgentCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent = GetIntelligenceAgentCommand {
            agent_id: self.agent_id,
        }
        .handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<IntelligenceAgent>()
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .execute()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key, Invalidation::Memories, Invalidation::AgentSkills]))
    }
}

/// Which end of an agent's turn answering the user [`RecordAgentTurnCommand`]
/// records.
pub enum AgentTurnPhase {
    Started,
    Ended,
}

/// Marks agent `agent_id`'s turn as started (its `foreground_started` set to
/// now) or ended (cleared), and raises `AgentTurnStarted` or
/// `AgentTurnEnded` so hooks hear of it. A turn may post nothing, so no
/// other write can raise them.
pub struct RecordAgentTurnCommand {
    pub agent_id: i64,
    pub phase: AgentTurnPhase,
}
impl DatabaseCommand for RecordAgentTurnCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let agent_id = self.agent_id;
        let update = ctx.db.update::<IntelligenceAgent>();
        let (update, event) = match self.phase {
            AgentTurnPhase::Started => (
                update.set_now(IntelligenceAgent::FOREGROUND_STARTED),
                DatabaseEventKind::AgentTurnStarted { agent_id },
            ),
            AgentTurnPhase::Ended => (
                update.set_null(IntelligenceAgent::FOREGROUND_STARTED),
                DatabaseEventKind::AgentTurnEnded { agent_id },
            ),
        };
        update
            .filter_eq(IntelligenceAgent::ID, agent_id)
            .execute()?;
        ctx.events.send(ctx.trace.event(event));
        Ok(().invalidating([Invalidation::Agent(agent_id)]))
    }
}

/// Records how many tokens agent `agent_id`'s live thread held at its latest
/// request, or `None` when it has no live thread (a thread begun or dropped).
pub struct SetAgentContextTokensCommand {
    pub agent_id: i64,
    pub tokens: Option<i64>,
}
impl DatabaseCommand for SetAgentContextTokensCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db
            .update::<IntelligenceAgent>()
            .set(IntelligenceAgent::CONTEXT_TOKENS, self.tokens)
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .execute()?;
        Ok(().invalidating([Invalidation::Agent(self.agent_id)]))
    }
}

/// Marks an agent's conversation read; the next message it sends marks it
/// unread again.
#[derive(FromRequest)]
#[request(MarkAgentReadRequest)]
pub struct MarkAgentReadCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for MarkAgentReadCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent: IntelligenceAgent = ctx
            .db
            .update()
            .set_false(IntelligenceAgent::AGENT_UNREAD)
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

/// Records what the agent is doing while it works; `None` once it is not
/// working.
pub struct SetAgentActivityCommand {
    pub agent_id: i64,
    pub activity: Option<String>,
}
impl DatabaseCommand for SetAgentActivityCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent: IntelligenceAgent = ctx
            .db
            .update()
            .set(IntelligenceAgent::ACTIVITY, self.activity)
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

/// Settles every agent as if no work were running, part of
/// [`ShutdownCleanupCommand`](crate::ShutdownCleanupCommand): work a
/// stopping daemon gave up on, or one that crashed was doing, never ended, so its
/// activity, its turn and `running` runs would otherwise stay. Every
/// activity and turn is cleared and each such run ends `failed`. A thread
/// outlives the daemon while its agent has a conversation
/// [`AgentThreadMark`], which the next daemon carries on from; every other
/// agent's context tokens are cleared.
pub struct SettleAgentActivityCommand;
impl DatabaseCommand for SettleAgentActivityCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .update::<IntelligenceAgent>()
            .set_null(IntelligenceAgent::ACTIVITY)
            .set_null(IntelligenceAgent::FOREGROUND_STARTED)
            .execute()?;
        ctx.db
            .update::<IntelligenceAgent>()
            .set_null(IntelligenceAgent::CONTEXT_TOKENS)
            .filter_not_in(
                "id",
                ctx.db.select_column::<AgentThreadMark, i64>("agent_id"),
            )
            .execute()?;
        ctx.db
            .update::<IntelligenceAgentRun>()
            .set_group(IntelligenceAgentRunState::Failed {
                ended_at: ctx.now,
                error: "The daemon stopped during this run.".to_string(),
            })
            .filter("outcome = ?")
            .bind(IntelligenceAgentRunOutcome::Running)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
