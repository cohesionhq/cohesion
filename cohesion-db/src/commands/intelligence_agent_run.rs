use cohesion_core::{
    FromRequest, Projection,
    invalidation::Invalidation,
    model::{HookKind, IntelligenceAgentRun, IntelligenceAgentRunState, IntelligenceCostEvent},
    protocol::ListAgentRunsRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Write,
};

/// Starts recording a background run of an agent's cue.
pub struct CreateAgentRunCommand {
    pub agent_id: i64,
    pub hook: HookKind,
    pub cue_name: String,
    pub thread_id: Option<i64>,
}
impl DatabaseCommand for CreateAgentRunCommand {
    type Response = IntelligenceAgentRun;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let run: IntelligenceAgentRun = ctx
            .db
            .insert()
            .set(IntelligenceAgentRun::AGENT_ID, self.agent_id)
            .set(IntelligenceAgentRun::HOOK, self.hook)
            .set(IntelligenceAgentRun::CUE_NAME, self.cue_name)
            .set(IntelligenceAgentRun::THREAD_ID, self.thread_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(run.agent_id);
        Ok(run.invalidating([key]))
    }
}

/// Starts recording a fork of agent `agent_id`'s conversation that `hook`
/// woke rather than a cue or a question: `actor:moved` for an actor session it
/// follows, `task:assigned` for a task assigned to it. A run with no cue,
/// not yet tied to a thread.
pub struct CreateAgentForkRunCommand {
    pub agent_id: i64,
    pub hook: HookKind,
}
impl DatabaseCommand for CreateAgentForkRunCommand {
    type Response = IntelligenceAgentRun;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let run: IntelligenceAgentRun = ctx
            .db
            .insert()
            .set(IntelligenceAgentRun::AGENT_ID, self.agent_id)
            .set(IntelligenceAgentRun::HOOK, self.hook)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(run.agent_id);
        Ok(run.invalidating([key]))
    }
}

/// Ties run `run_id`, recorded before its session existed, to the session's
/// thread, whose cost events its finish adds up.
pub struct SetAgentRunThreadCommand {
    pub run_id: i64,
    pub thread_id: i64,
}
impl DatabaseCommand for SetAgentRunThreadCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .update::<IntelligenceAgentRun>()
            .set(IntelligenceAgentRun::THREAD_ID, Some(self.thread_id))
            .filter_eq(IntelligenceAgentRun::ID, self.run_id)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// What a thread's cost events add up to: no row's worth of tokens is zero,
/// and a thread without events has no cost.
#[derive(Projection)]
#[projection(of = IntelligenceCostEvent)]
struct ThreadCost {
    #[projection(coalesce("SUM(input_tokens)", "0"))]
    input_tokens: i64,
    #[projection(coalesce("SUM(output_tokens)", "0"))]
    output_tokens: i64,
    #[projection(coalesce("SUM(cache_read_tokens)", "0"))]
    cache_read_tokens: i64,
    #[projection(coalesce("SUM(cache_write_tokens)", "0"))]
    cache_write_tokens: i64,
    #[projection(expr = "SUM(total_cost)")]
    cost: Option<f64>,
}

/// Ends a run, failed on `error` if it has one, totting up the tokens and cost its thread's
/// cost events recorded (`cost` stays `None` when there are none).
pub struct FinishAgentRunCommand {
    pub run_id: i64,
    pub error: Option<String>,
}
impl DatabaseCommand for FinishAgentRunCommand {
    type Response = IntelligenceAgentRun;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let started: IntelligenceAgentRun = ctx
            .db
            .select()
            .filter_eq(IntelligenceAgentRun::ID, self.run_id)
            .one()
            .found()?;
        let spent: ThreadCost = ctx
            .db
            .select_as()
            .filter("thread_id = ?")
            .bind(started.thread_id)
            .one()
            .found()?;
        let run: IntelligenceAgentRun = ctx
            .db
            .update()
            .set_group(match self.error {
                Some(error) => IntelligenceAgentRunState::Failed {
                    ended_at: ctx.now,
                    error,
                },
                None => IntelligenceAgentRunState::Completed { ended_at: ctx.now },
            })
            .set(IntelligenceAgentRun::INPUT_TOKENS, spent.input_tokens)
            .set(IntelligenceAgentRun::OUTPUT_TOKENS, spent.output_tokens)
            .set(
                IntelligenceAgentRun::CACHE_READ_TOKENS,
                spent.cache_read_tokens,
            )
            .set(
                IntelligenceAgentRun::CACHE_WRITE_TOKENS,
                spent.cache_write_tokens,
            )
            .set(IntelligenceAgentRun::COST, spent.cost)
            .filter_eq(IntelligenceAgentRun::ID, self.run_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(run.agent_id);
        Ok(run.invalidating([key]))
    }
}
/// An agent's `limit` most recent runs, newest first.
#[derive(FromRequest)]
#[request(ListAgentRunsRequest)]
pub struct ListAgentRunsCommand {
    pub agent_id: i64,
    pub limit: i64,
}
impl DatabaseCommand for ListAgentRunsCommand {
    type Response = Vec<IntelligenceAgentRun>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("agent_id = ?")
            .bind(self.agent_id)
            .order("id DESC")
            .limit(self.limit as u64)
            .all()?)
    }
}

#[cfg(test)]
mod tests;
