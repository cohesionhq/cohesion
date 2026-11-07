use std::fmt;

use cohesion_core::model::{
    AgentMessageContent, IntelligenceAgentMessage, IntelligenceAgentMessageSender,
};
use cohesion_db::{CommandError, CreateAgentMessageCommand, DatabaseContext};

use crate::intelligence_error::IntelligenceError;
use crate::session::SessionConstructionError;

/// Why an agent could not answer.
#[derive(Debug)]
pub enum AgentError {
    /// The agent's model could not be set up, e.g. its endpoint is gone.
    SessionConstruction(SessionConstructionError),
    /// The model call failed.
    Intelligence(IntelligenceError),
    Command(CommandError),
    /// The message answered is not a choice.
    NotAChoice,
    /// The agent's work panicked.
    Panicked,
}
impl AgentError {
    /// Saves why agent `agent_id`'s foreground turn failed as an `error`
    /// message in its feed, and returns it.
    pub(super) async fn post(
        &self,
        database: &DatabaseContext,
        agent_id: i64,
    ) -> Result<IntelligenceAgentMessage, CommandError> {
        database
            .send(CreateAgentMessageCommand {
                agent_id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content: AgentMessageContent::Error {
                    error: self.to_string(),
                },
                reply_to: None,
            })
            .await
    }
}

impl From<CommandError> for AgentError {
    fn from(value: CommandError) -> Self {
        Self::Command(value)
    }
}

/// What went wrong, for the user: an `error` message in the agent's feed
/// shows it.
impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionConstruction(SessionConstructionError::NoDefaultModel) => {
                write!(f, "No model is set for agents; choose one in Settings.")
            }
            Self::SessionConstruction(SessionConstructionError::Command(_)) | Self::Command(_) => {
                write!(f, "Couldn’t read or save the conversation.")
            }
            Self::Intelligence(e) => e.fmt(f),
            Self::NotAChoice => write!(f, "That message isn’t a choice."),
            Self::Panicked => write!(f, "The agent stopped unexpectedly."),
        }
    }
}
