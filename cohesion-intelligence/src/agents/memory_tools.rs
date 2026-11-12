//! The tools an agent keeps its memories with: `remember` and `forget`,
//! which only its conversation with the user has, and `memories`, which its
//! forks have too.

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{
    AgentMemoryChange, AgentMessageContent, IntelligenceAgentMessageSender, MemoryFilter,
    MemoryScope,
};
use cohesion_db::{
    CommandError, CreateAgentMessageCommand, CreateMemoryCommand, DatabaseContext,
    DeleteMemoryCommand, GetIntelligenceAgentCommand, ListMemoriesCommand,
};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use crate::activity::Activity;
use crate::system_prompt::memory_lines;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::ToolFuture;
use crate::transcript_item::ToolCallStatus;

/// Whose a new memory is.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Scope {
    /// The agent's own.
    Personal,
    /// The agents' on its team.
    Team,
    /// Every agent's.
    Global,
}

#[derive(Deserialize)]
struct RememberCall {
    memory: String,
    scope: Scope,
}

/// What a turn whose role may not change memories (one an event started)
/// hears when it tries.
const REFUSAL: &str = "A turn your cue started can't change memories: note where the work \
                             stands with `send_note`, or ask the user with `ask_choice`.";

#[derive(Deserialize)]
struct ForgetCall {
    memory_id: i64,
}

/// Lets the agent's conversation keep a memory, its own, its team's or
/// every agent's, and shows it in the feed, marked with the fork's run when
/// a fork keeps it.
pub(super) struct RememberTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl RememberTool {
    /// The team the agent is on now, which may have changed since the
    /// session began.
    async fn team(&self) -> Result<Option<i64>, CommandError> {
        let agent = self
            .database
            .send(GetIntelligenceAgentCommand {
                agent_id: self.agent_id,
            })
            .await?;
        Ok(agent.team_id)
    }
}

impl AgentTool for RememberTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "remember",
            description: "Keeps a memory for later conversations: a preference, a decision, a \
                 fact about the user, something you were asked to remember. Every memory you see \
                 is under `# Memories` in your instructions, and each scope keeps a limited number. \
                 It stays until you forget it, so forget one this replaces in the same turn."
                .into(),
            fields: vec![
                Field {
                    name: "memory",
                    description: "One plain sentence in your own words, not a copy of the \
                        message, using the names and plain words a later message about it \
                        would use (\"groceries\", \"the Home workspace\"), never \"this\" or \
                        \"it\".",
                    kind: FieldKind::String,
                    required: true,
                },
                Field {
                    name: "scope",
                    description: "`personal` for your own work; `team` for what your \
                        teammates would act on too, when you are on a team; `global` for what \
                        every agent should know, such as who the user is.",
                    kind: FieldKind::String,
                    required: true,
                },
            ],
        }
    }

    fn activity(&self, _arguments: &str) -> Option<Activity> {
        Some(Activity::Remembering)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: RememberCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            let scope = match call.scope {
                Scope::Personal => MemoryScope::Agent {
                    agent_id: self.agent_id,
                },
                Scope::Global => MemoryScope::Global,
                Scope::Team => match self.team().await {
                    Ok(Some(team_id)) => MemoryScope::Team { team_id },
                    Ok(None) => {
                        return ToolCallStatus::Failure(
                            "You're on no team; remember it as personal or global.".to_string(),
                        );
                    }
                    Err(e) => return ToolCallStatus::Failure(format!("could not remember: {e:?}")),
                },
            };
            let result = seat
                .database(&self.database)
                .send_write(CreateMemoryCommand {
                    scope,
                    memory: call.memory,
                })
                .await;
            let Some(result) = result else {
                return ToolCallStatus::Failure(REFUSAL.to_string());
            };
            match result {
                Ok(memory) => {
                    let id = memory.id;
                    post(
                        &self.database,
                        self.agent_id,
                        seat.fork_run_id(),
                        memory.memory,
                        AgentMemoryChange::Remembered,
                    )
                    .await;
                    ToolCallStatus::Success(format!("Remembered [m{id}]."))
                }
                Err(CommandError::Validation(e)) => ToolCallStatus::Failure(e.message),
                Err(e) => ToolCallStatus::Failure(format!("could not remember: {e:?}")),
            }
        })
    }
}

/// Lets the agent's conversation forget one of the memories it sees, and
/// shows that in the feed, marked with the fork's run when a fork forgets
/// it.
pub(super) struct ForgetTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl AgentTool for ForgetTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "forget",
            description: "Forgets a memory, yours, your team's or a global one, that is \
                wrong, no longer true or replaced by one you just remembered."
                .into(),
            fields: vec![Field {
                name: "memory_id",
                description: "The memory's id, as in `[m12]`.",
                kind: FieldKind::Integer,
                required: true,
            }],
        }
    }

    fn activity(&self, _arguments: &str) -> Option<Activity> {
        Some(Activity::Forgetting)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: ForgetCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            // The model chose the id, so it may name another agent's memory.
            let visible = self
                .database
                .send(ListMemoriesCommand {
                    filter: MemoryFilter::SeenBy(self.agent_id),
                })
                .await;
            let seen = match visible {
                Ok(memories) => memories.iter().any(|memory| memory.id == call.memory_id),
                Err(e) => return ToolCallStatus::Failure(format!("could not forget: {e:?}")),
            };
            if !seen {
                return ToolCallStatus::Failure("no memory of yours has that id".to_string());
            }
            let result = seat
                .database(&self.database)
                .send_write(DeleteMemoryCommand {
                    memory_id: call.memory_id,
                })
                .await;
            let Some(result) = result else {
                return ToolCallStatus::Failure(REFUSAL.to_string());
            };
            match result {
                Ok(memory) => {
                    let id = memory.id;
                    post(
                        &self.database,
                        self.agent_id,
                        seat.fork_run_id(),
                        memory.memory,
                        AgentMemoryChange::Forgotten,
                    )
                    .await;
                    ToolCallStatus::Success(format!("Forgot [m{id}]."))
                }
                Err(e) => ToolCallStatus::Failure(format!("could not forget: {e:?}")),
            }
        })
    }
}

/// Lists every memory the agent sees.
pub(super) struct MemoriesTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl AgentTool for MemoriesTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "memories",
            description: "Lists every memory you see as it is now: yours, then your team's, \
                then the global ones, each after its id."
                .into(),
            fields: Vec::new(),
        }
    }

    fn activity(&self, _arguments: &str) -> Option<Activity> {
        Some(Activity::Recalling)
    }

    fn call<'a>(&'a self, _seat: &'a Seat, _arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let result = self
                .database
                .send(ListMemoriesCommand {
                    filter: MemoryFilter::SeenBy(self.agent_id),
                })
                .await;
            match result {
                Ok(memories) if memories.is_empty() => {
                    ToolCallStatus::Success("You have no memories.".to_string())
                }
                Ok(memories) => ToolCallStatus::Success(memory_lines(&memories)),
                Err(e) => ToolCallStatus::Failure(format!("could not list memories: {e:?}")),
            }
        })
    }
}

/// Posts a `memory` message saying what became of `memory` to the agent's
/// feed, marked with `run_id` when a fork changed it. The memory is already kept or forgotten, so a failure here is only
/// logged.
async fn post(
    database: &DatabaseContext,
    agent_id: i64,
    run_id: Option<i64>,
    memory: String,
    change: AgentMemoryChange,
) {
    let result = database
        .send(CreateAgentMessageCommand {
            agent_id,
            run_id,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::Memory { memory, change },
            reply_to: None,
        })
        .await;
    if let Err(e) = result {
        log_error!(
            Module::Intelligence,
            "Agent {agent_id}: could not post its memory: {e:?}"
        );
    }
}
