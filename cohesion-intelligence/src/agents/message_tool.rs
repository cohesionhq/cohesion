use cohesion_core::model::{AgentChoice, AgentMessageContent, IntelligenceAgentMessageSender};
use cohesion_core::protocol::ImageUpload;
use cohesion_db::{
    CreateAgentMessageCommand, CreateAgentMessageWithImagesCommand, DatabaseContext,
};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::conversation_session::Branch;
use super::image_tray::ImageTray;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::{ToolFuture, asks_done, done_field};
use crate::transcript_item::ToolCallStatus;

/// The most options a choice offers, lettered `A` to `F`.
const MAX_OPTIONS: usize = 6;

#[derive(Deserialize)]
struct MessageCall {
    text: String,
    reply_to: Option<i64>,
    #[serde(default)]
    images: Vec<usize>,
}

#[derive(Deserialize)]
struct NoteCall {
    note: String,
}

#[derive(Deserialize)]
struct ChoiceCall {
    question: String,
    options: Vec<String>,
    context: Option<String>,
    reply_to: Option<i64>,
}

/// Lets an agent text the user: each call stores a message in the feed at
/// once, so a turn or a fork can say what it has to in several
/// short messages rather than one long one. A foreground turn talks to the
/// user only through it, as its final reply isn't posted; a message shows
/// any of the images the session's scripts attached. As `ask_choice`, it
/// stores a `choice` instead, which the user answers with a button.
pub(super) struct MessageTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
    pub kind: MessageKind,
}

/// Which tool a [`MessageTool`] is.
pub(super) enum MessageKind {
    /// `send_message`, storing a `text` message with the images it names
    /// from `images`, the session's.
    Text { images: ImageTray },
    /// `ask_choice`, storing a `choice`.
    Choice,
    /// `send_note`, storing a `note` nobody is told of.
    Note,
}

/// The longest a note may be, so the news a later turn hears stays short.
const NOTE_LIMIT: usize = 1_000;

impl AgentTool for MessageTool {
    fn definition(&self) -> Tool {
        match &self.kind {
            MessageKind::Text { .. } => Tool {
                name: "send_message",
                description: "Sends the user one short message now, while your turn goes on. \
                    Each call is its own message in the conversation, like a text. \
                    It is the only way to talk to the user (with `ask_choice` and `react`): \
                    your final reply is NEVER shown to them, so text written only as the \
                    final reply is lost. Every turn that answers a message or a pick MUST \
                    call this or `react` at least once. \
                    Otherwise send one only when the user needs to act, decide or know."
                    .into(),
                fields: vec![
                    Field {
                        name: "text",
                        description: "The message, in Markdown: usually a sentence or two.",
                        kind: FieldKind::String,
                        required: true,
                    },
                    reply_to_field(),
                    Field {
                        name: "images",
                        description: "The numbers of images your scripts attached with \
                            `images` (`Attached as image 3.`), to show below the text, in \
                            this order; at most 10. Leave it out to send none.",
                        kind: FieldKind::IntegerList,
                        required: false,
                    },
                    done_field(),
                ],
            },
            MessageKind::Note => Tool {
                name: "send_note",
                description: "Notes for your own later turns where work stands, what you are \
                    waiting on and what comes next, in a sentence or two; the user isn't told \
                    of it and it answers no one. This thread and its forks hear it; a thread \
                    rebuilt over an hour later won't, so restate what is pending in your \
                    newest note and put plans that must outlast this thread in `storage` or \
                    on their task. Never in place of telling the user what they should know."
                    .into(),
                fields: vec![
                    Field {
                        name: "note",
                        description: "The note, at most 1,000 characters.",
                        kind: FieldKind::String,
                        required: true,
                    },
                    done_field(),
                ],
            },
            MessageKind::Choice => Tool {
                name: "ask_choice",
                description: "Asks the user a question with a few answers to pick from, \
                    each a button lettered A, B, C… in the agent's conversation. \
                    Their pick starts a turn of the conversation. Like `send_message`, it \
                    reaches the user; your final reply never does. A fork must give `context`: \
                    the turn the pick starts never sees the fork's thread."
                    .into(),
                fields: vec![
                    question_field(),
                    options_field(),
                    Field {
                        name: "context",
                        description: "Required in a fork; leave it out in the conversation. \
                            Notes for the turn that acts on the pick later without this \
                            fork's transcript: what you found (titles, ids, dates), why you \
                            are asking, and what each answer should lead to, with the code to \
                            run if you have it. The user doesn't see it.",
                        kind: FieldKind::String,
                        required: false,
                    },
                    reply_to_field(),
                    done_field(),
                ],
            },
        }
    }

    fn ends_turn(&self, arguments: &str) -> bool {
        asks_done(arguments)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            // What the reply adds after the new message's id.
            let (content, reply_to, attachments, after) = match &self.kind {
                MessageKind::Text { images } => match text(arguments, images) {
                    Ok((text, reply_to, attachments)) => (
                        AgentMessageContent::Text { text },
                        reply_to,
                        attachments,
                        "",
                    ),
                    Err(e) => return ToolCallStatus::Failure(e),
                },
                MessageKind::Note => match note(arguments) {
                    Ok(note) => (AgentMessageContent::Note { note }, None, Vec::new(), ""),
                    Err(e) => return ToolCallStatus::Failure(e),
                },
                MessageKind::Choice => match choice(arguments, seat) {
                    Ok((choice, reply_to)) => (
                        AgentMessageContent::Choice(choice),
                        reply_to,
                        Vec::new(),
                        " The user's pick starts a later turn; don't pick for them.",
                    ),
                    Err(e) => return ToolCallStatus::Failure(e),
                },
            };
            let result = self
                .database
                .send_traced(
                    CreateAgentMessageWithImagesCommand {
                        message: CreateAgentMessageCommand {
                            reply_to,
                            agent_id: self.agent_id,
                            run_id: seat.fork_run_id(),
                            run_description: None,
                            sender: IntelligenceAgentMessageSender::Agent,
                            content,
                        },
                        attachments,
                    },
                    seat.turn.trace(),
                )
                .await;
            match result {
                Ok(message) => {
                    let verb = match self.kind {
                        MessageKind::Text { .. } => "Sent",
                        MessageKind::Choice => "Asked",
                        MessageKind::Note => "Noted",
                    };
                    let reply = format!("{verb} as [#{}].{after}", message.id);
                    // A note answers no one, so it doesn't count as the
                    // turn having told the user something.
                    if let (
                        Branch::Conversation { sent, .. },
                        MessageKind::Text { .. } | MessageKind::Choice,
                    ) = (&seat.branch, &self.kind)
                    {
                        sent.lock().expect("sent is never poisoned").push(message);
                    }
                    ToolCallStatus::Success(reply)
                }
                Err(e) => ToolCallStatus::Failure(format!("could not send the message: {e:?}")),
            }
        })
    }
}

/// A `send_message` call's text, the message it replies to, and the images
/// from `images` it shows.
fn text(
    arguments: &str,
    images: &ImageTray,
) -> Result<(String, Option<i64>, Vec<ImageUpload>), String> {
    let call: MessageCall = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
    let text = call.text.trim();
    if text.is_empty() {
        return Err("the message is empty".to_string());
    }
    Ok((text.to_string(), call.reply_to, images.get(&call.images)?))
}

/// A `send_note` call's note, cut to [`NOTE_LIMIT`] characters.
fn note(arguments: &str) -> Result<String, String> {
    let call: NoteCall = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
    let note = call.note.trim();
    if note.is_empty() {
        return Err("the note is empty".to_string());
    }
    Ok(note.chars().take(NOTE_LIMIT).collect())
}

fn question_field() -> Field {
    Field {
        name: "question",
        description: "The question, in Markdown: usually a sentence.",
        kind: FieldKind::String,
        required: true,
    }
}

fn reply_to_field() -> Field {
    Field {
        name: "reply_to",
        description: "The id of a message in the conversation this answers, as in `[#42]`, \
            to show it quoted above yours; leave it out when yours stands alone.",
        kind: FieldKind::Integer,
        required: false,
    }
}

fn options_field() -> Field {
    Field {
        name: "options",
        description: "Two to six short answers, in the order to letter them.",
        kind: FieldKind::StringList,
        required: true,
    }
}

/// An `ask_choice` call's question and options, still to be answered, with
/// the context a fork must give, and the message it replies to.
fn choice(arguments: &str, seat: &Seat) -> Result<(AgentChoice, Option<i64>), String> {
    let call: ChoiceCall = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
    let context = match seat.branch {
        Branch::Conversation { .. } => None,
        Branch::Fork { .. } => match call.context.as_deref().map(str::trim) {
            Some(context) if !context.is_empty() => Some(context.to_string()),
            _ => return Err("a fork's choice needs its context".to_string()),
        },
    };
    let question = call.question.trim();
    if question.is_empty() {
        return Err("the question is empty".to_string());
    }
    let options: Vec<String> = call
        .options
        .iter()
        .map(|option| option.trim().to_string())
        .collect();
    if !(2..=MAX_OPTIONS).contains(&options.len()) {
        return Err(format!(
            "a choice offers 2 to {MAX_OPTIONS} options, not {}",
            options.len()
        ));
    }
    if options.iter().any(String::is_empty) {
        return Err("an option is empty".to_string());
    }
    let choice = AgentChoice {
        question: question.to_string(),
        options,
        context,
        selected: None,
    };
    Ok((choice, call.reply_to))
}
