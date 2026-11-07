use std::fmt;

use cohesion_core::model::IntelligenceSpend;
use cohesion_db::CommandError;

use crate::spend_limits::limit_text;

#[derive(Debug)]
pub enum IntelligenceError {
    RateLimited,
    InvalidAuthorization,
    ContextLimitExceeded,
    Network(reqwest::Error),
    UnexpectedStatus(u16, String),
    Serialization(serde_json::Error),
    InvalidResponse(serde_json::Error),
    /// The thread could not be saved: an item, or where its provider keeps
    /// it.
    Save(CommandError),
    /// The spend limits could not be read before a model call.
    SpendCheck(CommandError),
    /// The model kept calling tools past the session's limit of rounds.
    TooManyRounds(usize),
    /// The account an endpoint signs in with gave no credential, and why:
    /// its extension couldn't renew it, or the account is no longer
    /// connected.
    AccountCredential(String),
    /// The provider stopped the reply after it began streaming, with its
    /// error's code and message.
    ResponseFailed {
        code: String,
        message: String,
    },
    /// The user stopped the answer.
    Stopped,
    /// A spend limit is reached, which keeps forks from making
    /// model calls: the spend of its period.
    SpendLimitReached(IntelligenceSpend),
}

/// What went wrong, for the user: shown in an agent's feed and run history.
impl fmt::Display for IntelligenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited => write!(f, "The model’s provider is limiting requests."),
            Self::InvalidAuthorization => write!(f, "The model’s provider rejected the API key."),
            Self::ContextLimitExceeded => {
                write!(
                    f,
                    "The conversation no longer fits the model’s context window."
                )
            }
            Self::Network(e) if e.is_timeout() => {
                write!(f, "The model’s provider took too long to answer.")
            }
            Self::Network(e) if e.is_connect() => {
                write!(f, "Couldn’t reach the model’s provider: {e}")
            }
            Self::Network(e) => write!(f, "The request to the model’s provider failed: {e}"),
            Self::UnexpectedStatus(code, body) => {
                let body: String = body.chars().take(500).collect();
                write!(f, "The model’s provider answered {code}: {body}")
            }
            Self::Serialization(e) => write!(f, "Couldn’t encode the request: {e}"),
            Self::InvalidResponse(e) => write!(f, "The model’s reply couldn’t be read: {e}"),
            Self::Save(_) => write!(f, "Couldn’t save the conversation."),
            Self::SpendCheck(_) => write!(f, "Couldn’t check the spend limits."),
            Self::TooManyRounds(rounds) => {
                write!(
                    f,
                    "Stopped after {rounds} rounds of tool calls without an answer."
                )
            }
            Self::AccountCredential(message) => {
                write!(f, "The endpoint’s account couldn’t sign in: {message}")
            }
            Self::ResponseFailed { code, message } => {
                write!(
                    f,
                    "The model’s provider stopped the reply ({code}): {message}"
                )
            }
            Self::Stopped => write!(f, "You stopped the answer."),
            Self::SpendLimitReached(spend) => write!(f, "{}", limit_text(spend)),
        }
    }
}
