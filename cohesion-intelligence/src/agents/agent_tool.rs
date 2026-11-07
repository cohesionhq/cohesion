//! An agent's tools, split in two: what the model sees and what a call
//! does. A tool is built with no session in reach and learns which one it
//! serves only when called, from its [`Seat`], so its definition can't vary
//! between the conversation and a fork: a fork's prompt is the
//! conversation's, and a provider finds it cached.

use std::sync::Arc;

use cohesion_core::model::Initiative;
use cohesion_db::{DatabaseContext, GuardedDatabaseContext};
use cohesion_js::AgentRole;

use super::conversation_session::Branch;
use super::foreground_turn::ForegroundTurn;
use crate::activity::Activity;
use crate::tool_definition::Tool;
use crate::tools::{SessionTool, ToolFuture};

/// Where one of an agent's tool calls runs: the turn, whose stop ends the
/// call and whose trace its writes continue, and the session of the
/// conversation it belongs to.
pub(super) struct Seat {
    pub turn: Arc<ForegroundTurn>,
    pub branch: Branch,
}
impl Seat {
    /// The fork making the call, by its run; `None` in the conversation
    /// itself.
    pub fn fork_run_id(&self) -> Option<i64> {
        match &self.branch {
            Branch::Conversation { .. } => None,
            Branch::Fork { run_id, .. } => Some(*run_id),
        }
    }

    /// Who started the chain the turn is in.
    pub fn initiative(&self) -> Initiative {
        self.turn.trace().initiative()
    }

    /// `database` as the turn's role may write it ([`role_database`]).
    pub fn database(&self, database: &DatabaseContext) -> GuardedDatabaseContext {
        role_database(database, self.initiative())
    }
}

/// `database` narrowed to what a turn whose chain `initiative` started may
/// write, as its [`AgentRole`] says: a turn an event started changes none
/// of the agent's memories or cues, and its writes of them find no area.
pub(super) fn role_database(
    database: &DatabaseContext,
    initiative: Initiative,
) -> GuardedDatabaseContext {
    AgentRole::Turn(initiative).database(&database.read_write())
}

/// One of an agent's tools, the same in every session of its conversation:
/// its definition reads only the tool, and each call learns its session
/// from `seat`.
pub(super) trait AgentTool: Send + Sync {
    fn definition(&self) -> Tool;
    /// Runs one call in `seat`. `arguments` is the model's JSON,
    /// unvalidated.
    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a>;
    /// As [`SessionTool::activity`].
    fn activity(&self, _arguments: &str) -> Option<Activity> {
        None
    }
    /// As [`SessionTool::globals`].
    fn globals(&self) -> Vec<&'static str> {
        Vec::new()
    }
    /// As [`SessionTool::ends_turn`].
    fn ends_turn(&self, _arguments: &str) -> bool {
        false
    }
}

/// `tools` as a session runs them, every call in `seat`.
pub(super) fn seated(tools: Vec<Box<dyn AgentTool>>, seat: Seat) -> Vec<Box<dyn SessionTool>> {
    let seat = Arc::new(seat);
    tools
        .into_iter()
        .map(|tool| {
            Box::new(Seated {
                tool,
                seat: seat.clone(),
            }) as Box<dyn SessionTool>
        })
        .collect()
}

/// An agent's tool in its seat.
struct Seated {
    tool: Box<dyn AgentTool>,
    seat: Arc<Seat>,
}
impl SessionTool for Seated {
    fn definition(&self) -> Tool {
        self.tool.definition()
    }

    fn call<'a>(&'a self, arguments: &'a str) -> ToolFuture<'a> {
        self.tool.call(&self.seat, arguments)
    }

    fn activity(&self, arguments: &str) -> Option<Activity> {
        self.tool.activity(arguments)
    }

    fn globals(&self) -> Vec<&'static str> {
        self.tool.globals()
    }

    fn ends_turn(&self, arguments: &str) -> bool {
        self.tool.ends_turn(arguments)
    }
}
