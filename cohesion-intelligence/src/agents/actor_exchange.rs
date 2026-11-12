//! What an actor session was told and what its actor said back, which the
//! fork that follows it up reads.

use cohesion_core::model::{ActorMessageContent, ActorSessionMessageContent, ActorUserContent};
use cohesion_db::{CommandError, DatabaseContext, ListActorSessionMessagesCommand};
use cohesion_js::globals::ActorSessionMessage;

/// What an agent's conversation told an actor session at its start, and what
/// the actor said since it was last told something.
pub(super) struct SessionExchange {
    /// The session's prompt, its first message, when that is text (a session's
    /// transcript may have been pruned down to the actor's replies).
    pub prompt: Option<String>,
    /// What the actor said since it was last told something, its tool uses
    /// and thinking left out, as JSON for the model.
    pub said: String,
}

/// What session `session_id` was told at its start and what its actor said since
/// it was last told something.
pub(super) async fn exchange(
    database: &DatabaseContext,
    session_id: i64,
) -> Result<SessionExchange, CommandError> {
    let messages = database
        .send(ListActorSessionMessagesCommand {
            session_id,
            ids: None,
        })
        .await?;
    let prompt = match messages.first().map(|message| &message.content) {
        Some(ActorSessionMessageContent::User(ActorUserContent::Text { text })) => {
            Some(text.clone())
        }
        _ => None,
    };
    let said = latest_reply(messages);
    Ok(SessionExchange {
        prompt,
        said: serde_json::to_string(&said).expect("actor session messages serialize"),
    })
}

/// What the actor said after the transcript's last user message, its tool
/// uses and thinking left out: its answer to the prompt or to what it was
/// told since.
fn latest_reply(
    messages: Vec<cohesion_core::model::ActorSessionMessage>,
) -> Vec<ActorSessionMessage> {
    let start = messages
        .iter()
        .rposition(|message| matches!(message.content, ActorSessionMessageContent::User(_)))
        .map_or(0, |last| last + 1);
    messages
        .into_iter()
        .skip(start)
        .filter(|message| {
            !matches!(
                message.content,
                ActorSessionMessageContent::Actor(
                    ActorMessageContent::ToolUse { .. } | ActorMessageContent::Thinking { .. }
                )
            )
        })
        .map(ActorSessionMessage::from)
        .collect()
}

#[cfg(test)]
mod tests;
