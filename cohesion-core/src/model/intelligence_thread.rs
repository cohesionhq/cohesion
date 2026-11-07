use cohesion_macros::{ColumnGroup, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::ImageBytes;
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Row, RowError, Value, ValueError, read_column};

/// A system prompt, stored once however many threads open with it: every
/// fork of an agent's conversation shares its thread's.
#[derive(Debug, Clone, Model)]
#[model(table = "thread_prompts", backup = Included)]
pub struct ThreadPrompt {
    pub id: i64,
    pub prompt: String,
}

/// The messages sent to a model and what it answered, for an agent's
/// conversation or a fork of it.
#[derive(Debug, Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_threads", backup = Included)]
#[model(join = ThreadPrompt, on = "thread_prompts.id = intelligence_threads.prompt_id")]
pub struct IntelligenceThread {
    pub id: i64,
    pub created_at: i64,
    pub prompt_id: i64,
    /// The system prompt, from `prompt_id`.
    #[model(expr = "thread_prompts.prompt")]
    pub system_prompt: String,
    /// Whether it started empty or carries on another thread's messages.
    #[model(group)]
    pub origin: ThreadOrigin,
    /// The provider's own copy of the conversation, when it keeps one.
    #[model(group)]
    pub remote: Option<RemoteChain>,
}

/// Where a thread's first messages come from.
///
/// - `"root"`: it holds all its own.
/// - `"fork"`: it carries on another thread: every message of `parent_thread_id`
///   through `inherited_through_id` comes before its own, read from the parent
///   rather than copied, and there are `inherited_items` of them, counting
///   what the parent itself inherited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ThreadOrigin {
    Root,
    Fork {
        parent_thread_id: i64,
        inherited_through_id: i64,
        inherited_items: i64,
    },
}

/// The conversation a provider keeps for a thread, and how many of the
/// thread's messages it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TsWire)]
pub struct RemoteChain {
    pub remote_id: String,
    pub remote_cursor: i64,
}

/// A thread's remote chain as its two columns, both `NULL` until a provider
/// reports one; the table's check keeps them together.
impl ColumnGroup for Option<RemoteChain> {
    const COLUMNS: &'static [&'static str] = &["remote_id", "remote_cursor"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let remote_id: Option<String> = read_column(row, "remote_id")?;
        let remote_cursor: Option<i64> = read_column(row, "remote_cursor")?;
        Ok(remote_id
            .zip(remote_cursor)
            .map(|(remote_id, remote_cursor)| RemoteChain {
                remote_id,
                remote_cursor,
            }))
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Some(chain) => vec![
                chain.remote_id.into_value(),
                chain.remote_cursor.into_value(),
            ],
            None => vec![Value::Null, Value::Null],
        }
    }
}

/// One message of a thread. A fork's own messages come after those it
/// inherits, and ids order them all.
#[derive(Debug, Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_thread_messages", backup = Included)]
pub struct IntelligenceThreadMessage {
    // Used for sequencing.
    pub id: i64,
    // Must be linked to a thread.
    pub thread_id: i64,
    pub created_at: i64,
    /// What it says.
    #[model(group)]
    pub item: ThreadItem,
}

/// Which kind of item a thread message holds; stored as its number, which
/// the table's checks read to say which columns each kind sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TextEnum)]
#[text_enum(store = "integer")]
pub enum ThreadItemKind {
    UserMessage = 0,
    AssistantMessage = 1,
    Thought = 2,
    RedactedThought = 3,
    ToolCallRequest = 4,
    ToolCallResponse = 5,
}

/// What a thread message says, tagged by `kind`.
///
/// - `"userMessage"`: what the model was told.
/// - `"assistantMessage"`: what the model said.
/// - `"thought"`: the model's reasoning. `signature` is an opaque token some
///   providers require back with it; `in_content` says it came inside the
///   message content, which is how it goes back.
/// - `"redactedThought"`: reasoning the provider withheld: an opaque `trace`,
///   and a summary if it gave one.
/// - `"toolCallRequest"`: a tool the model called; `arguments` is what the
///   model sent, usually JSON.
/// - `"toolCallResponse"`: what the call returned, or why it failed.
///
/// An old message is emptied but stays this shape, so a request still pairs
/// with its response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ThreadItem {
    UserMessage {
        text: String,
    },
    AssistantMessage {
        text: String,
    },
    Thought {
        text: String,
        signature: Option<String>,
        in_content: bool,
    },
    RedactedThought {
        trace: String,
        summary: Option<String>,
    },
    ToolCallRequest {
        call_identifier: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallResponse {
        call_identifier: String,
        succeeded: bool,
        text: String,
    },
}

/// A column a kind of item always sets; a row without it breaks the table's
/// checks, so this only guards reading past them.
fn required<T>(column: &str, value: Option<T>) -> Result<T, RowError> {
    value.ok_or_else(|| RowError::Value {
        column: column.to_string(),
        error: ValueError::Invalid("this kind of item always sets it".to_string()),
    })
}

/// A thread item as `kind` and the columns that kind sets, which the
/// table's checks hold to. A thought's and a response's text share `text`,
/// as a redacted thought's summary does.
impl ColumnGroup for ThreadItem {
    const COLUMNS: &'static [&'static str] = &[
        "kind",
        "text",
        "signature",
        "in_content",
        "redacted_trace",
        "call_identifier",
        "tool_name",
        "arguments",
        "succeeded",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let text: Option<String> = read_column(row, "text")?;
        let call_identifier: Option<String> = read_column(row, "call_identifier")?;
        Ok(match read_column::<ThreadItemKind>(row, "kind")? {
            ThreadItemKind::UserMessage => Self::UserMessage {
                text: required("text", text)?,
            },
            ThreadItemKind::AssistantMessage => Self::AssistantMessage {
                text: required("text", text)?,
            },
            ThreadItemKind::Thought => Self::Thought {
                text: required("text", text)?,
                signature: read_column(row, "signature")?,
                in_content: read_column::<Option<bool>>(row, "in_content")?.unwrap_or(false),
            },
            ThreadItemKind::RedactedThought => Self::RedactedThought {
                trace: required("redacted_trace", read_column(row, "redacted_trace")?)?,
                summary: text,
            },
            ThreadItemKind::ToolCallRequest => Self::ToolCallRequest {
                call_identifier: required("call_identifier", call_identifier)?,
                tool_name: required("tool_name", read_column(row, "tool_name")?)?,
                arguments: required("arguments", read_column(row, "arguments")?)?,
            },
            ThreadItemKind::ToolCallResponse => Self::ToolCallResponse {
                call_identifier: required("call_identifier", call_identifier)?,
                succeeded: required("succeeded", read_column(row, "succeeded")?)?,
                text: required("text", text)?,
            },
        })
    }

    fn into_values(self) -> Vec<Value> {
        let (kind, text, signature, in_content, trace, call, tool, arguments, succeeded) =
            match self {
                Self::UserMessage { text } => (
                    ThreadItemKind::UserMessage,
                    Some(text),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                Self::AssistantMessage { text } => (
                    ThreadItemKind::AssistantMessage,
                    Some(text),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                Self::Thought {
                    text,
                    signature,
                    in_content,
                } => (
                    ThreadItemKind::Thought,
                    Some(text),
                    signature,
                    Some(in_content),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                Self::RedactedThought { trace, summary } => (
                    ThreadItemKind::RedactedThought,
                    summary,
                    None,
                    None,
                    Some(trace),
                    None,
                    None,
                    None,
                    None,
                ),
                Self::ToolCallRequest {
                    call_identifier,
                    tool_name,
                    arguments,
                } => (
                    ThreadItemKind::ToolCallRequest,
                    None,
                    None,
                    None,
                    None,
                    Some(call_identifier),
                    Some(tool_name),
                    Some(arguments),
                    None,
                ),
                Self::ToolCallResponse {
                    call_identifier,
                    succeeded,
                    text,
                } => (
                    ThreadItemKind::ToolCallResponse,
                    Some(text),
                    None,
                    None,
                    None,
                    Some(call_identifier),
                    None,
                    None,
                    Some(succeeded),
                ),
            };
        vec![
            kind.into_value(),
            text.into_value(),
            signature.into_value(),
            in_content.into_value(),
            trace.into_value(),
            call.into_value(),
            tool.into_value(),
            arguments.into_value(),
            succeeded.into_value(),
        ]
    }
}

/// An image the model saw with a user message. A backup leaves it out, as it
/// does the attachment it came from.
#[derive(Debug, Clone, Model)]
#[model(table = "intelligence_thread_images", backup = LeftOut)]
pub struct IntelligenceThreadImage {
    pub id: i64,
    pub message_id: i64,
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    pub data: ImageBytes,
}
