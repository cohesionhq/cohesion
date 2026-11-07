//! How an actor session message's content is stored: who said it, what kind it is
//! and the columns that kind sets. The wire keeps [`ActorSessionMessageContent`]'s
//! tagged shape. Only the nested request and answer lists stay JSON, in
//! `details`: they are read and written whole, and a permission's tool input is
//! arbitrary JSON anyway.

use cohesion_macros::TextEnum;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{
    ActorMessageContent, ActorPermissionDecision, ActorSessionMessageContent, ActorToolKind,
    ActorUserContent,
};
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Json, Row, RowError, Value, ValueError, read_column};

/// Who said an actor session message.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorMessageSender {
    Actor,
    User,
}

/// What kind of thing an actor session message says.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorMessageKind {
    Text,
    Thinking,
    ToolUse,
    Question,
    Permission,
    Plan,
    Answers,
}

/// `details` parsed as `T`.
fn details<T: DeserializeOwned>(row: &dyn Row) -> Result<T, RowError> {
    Ok(read_column::<Json<T>>(row, "details")?.0)
}

/// `value` as `details` holds it.
fn to_details<T: Serialize>(value: T) -> Option<Value> {
    Some(Json(value).into_value())
}

/// The columns one message sets; the rest stay `NULL`.
struct Columns {
    sender: ActorMessageSender,
    kind: ActorMessageKind,
    text: Option<String>,
    ended_at: Option<i64>,
    tool: Option<String>,
    category: Option<ActorToolKind>,
    request_id: Option<i64>,
    decision: Option<ActorPermissionDecision>,
    approved: Option<bool>,
    details: Option<Value>,
}

impl Columns {
    fn of(sender: ActorMessageSender, kind: ActorMessageKind) -> Self {
        Self {
            sender,
            kind,
            text: None,
            ended_at: None,
            tool: None,
            category: None,
            request_id: None,
            decision: None,
            approved: None,
            details: None,
        }
    }
}

impl ColumnGroup for ActorSessionMessageContent {
    const COLUMNS: &'static [&'static str] = &[
        "sender",
        "kind",
        "text",
        "ended_at",
        "tool",
        "category",
        "request_id",
        "decision",
        "approved",
        "details",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        use ActorMessageKind as Kind;
        let kind: Kind = read_column(row, "kind")?;
        let mismatch = || RowError::Value {
            column: "kind".to_string(),
            error: ValueError::Invalid("this sender doesn't say this kind".to_string()),
        };
        Ok(match read_column::<ActorMessageSender>(row, "sender")? {
            ActorMessageSender::Actor => Self::Actor(match kind {
                Kind::Text => ActorMessageContent::Text {
                    text: read_column(row, "text")?,
                },
                Kind::Thinking => ActorMessageContent::Thinking {
                    text: read_column(row, "text")?,
                    ended_at: read_column(row, "ended_at")?,
                },
                Kind::ToolUse => ActorMessageContent::ToolUse {
                    tool: read_column(row, "tool")?,
                    category: read_column(row, "category")?,
                    summary: read_column(row, "text")?,
                    ended_at: read_column(row, "ended_at")?,
                },
                Kind::Question => ActorMessageContent::Question {
                    questions: details(row)?,
                },
                Kind::Permission => ActorMessageContent::Permission(details(row)?),
                Kind::Plan => ActorMessageContent::Plan {
                    plan: read_column(row, "text")?,
                },
                Kind::Answers => return Err(mismatch()),
            }),
            ActorMessageSender::User => Self::User(match kind {
                Kind::Text => ActorUserContent::Text {
                    text: read_column(row, "text")?,
                },
                Kind::Answers => ActorUserContent::Answers {
                    request_id: read_column(row, "request_id")?,
                    answers: details(row)?,
                },
                Kind::Permission => ActorUserContent::Permission {
                    request_id: read_column(row, "request_id")?,
                    decision: read_column(row, "decision")?,
                    message: read_column(row, "text")?,
                },
                Kind::Plan => ActorUserContent::Plan {
                    request_id: read_column(row, "request_id")?,
                    approved: read_column(row, "approved")?,
                    feedback: read_column(row, "text")?,
                },
                Kind::Thinking | Kind::ToolUse | Kind::Question => return Err(mismatch()),
            }),
        })
    }

    fn into_values(self) -> Vec<Value> {
        use ActorMessageKind as Kind;
        use ActorMessageSender::{Actor, User};
        let columns = match self {
            Self::Actor(content) => match content {
                ActorMessageContent::Text { text } => Columns {
                    text: Some(text),
                    ..Columns::of(Actor, Kind::Text)
                },
                ActorMessageContent::Thinking { text, ended_at } => Columns {
                    text: Some(text),
                    ended_at,
                    ..Columns::of(Actor, Kind::Thinking)
                },
                ActorMessageContent::ToolUse {
                    tool,
                    category,
                    summary,
                    ended_at,
                } => Columns {
                    text: Some(summary),
                    ended_at,
                    tool: Some(tool),
                    category: Some(category),
                    ..Columns::of(Actor, Kind::ToolUse)
                },
                ActorMessageContent::Question { questions } => Columns {
                    details: to_details(questions),
                    ..Columns::of(Actor, Kind::Question)
                },
                ActorMessageContent::Permission(request) => Columns {
                    details: to_details(request),
                    ..Columns::of(Actor, Kind::Permission)
                },
                ActorMessageContent::Plan { plan } => Columns {
                    text: Some(plan),
                    ..Columns::of(Actor, Kind::Plan)
                },
            },
            Self::User(content) => match content {
                ActorUserContent::Text { text } => Columns {
                    text: Some(text),
                    ..Columns::of(User, Kind::Text)
                },
                ActorUserContent::Answers {
                    request_id,
                    answers,
                } => Columns {
                    request_id: Some(request_id),
                    details: to_details(answers),
                    ..Columns::of(User, Kind::Answers)
                },
                ActorUserContent::Permission {
                    request_id,
                    decision,
                    message,
                } => Columns {
                    text: message,
                    request_id: Some(request_id),
                    decision: Some(decision),
                    ..Columns::of(User, Kind::Permission)
                },
                ActorUserContent::Plan {
                    request_id,
                    approved,
                    feedback,
                } => Columns {
                    text: feedback,
                    request_id: Some(request_id),
                    approved: Some(approved),
                    ..Columns::of(User, Kind::Plan)
                },
            },
        };
        vec![
            columns.sender.into_value(),
            columns.kind.into_value(),
            columns.text.into_value(),
            columns.ended_at.into_value(),
            columns.tool.into_value(),
            columns.category.into_value(),
            columns.request_id.into_value(),
            columns.decision.into_value(),
            columns.approved.into_value(),
            columns.details.into_value(),
        ]
    }
}
