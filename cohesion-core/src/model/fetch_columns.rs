//! How an article's snapshot and a feed's status are stored: a state, and the
//! columns that state sets. The wire keeps their tagged shapes; the tables'
//! checks hold each state to its columns.

use cohesion_macros::TextEnum;
use serde::{Deserialize, Serialize};

use super::{ArticleFetchFailure, ArticleSnapshot, FeedStatus};
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Row, RowError, Value, read_column};

/// Which state an article's `snapshot_state` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ArticleSnapshotState {
    Captured,
    Failed,
}

/// Which state a feed's `status_state` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum FeedStatusState {
    Refreshed,
    Failed,
}

/// What a failure's `failure_kind` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum FetchFailureKind {
    Unreachable,
    Status,
    UnsupportedContent,
    TooLarge,
    NotAFeed,
}

fn read_failure(row: &dyn Row) -> Result<ArticleFetchFailure, RowError> {
    Ok(
        match read_column::<FetchFailureKind>(row, "failure_kind")? {
            FetchFailureKind::Unreachable => ArticleFetchFailure::Unreachable {
                message: read_column(row, "failure_text")?,
            },
            FetchFailureKind::Status => ArticleFetchFailure::Status {
                code: read_column(row, "failure_code")?,
            },
            FetchFailureKind::UnsupportedContent => ArticleFetchFailure::UnsupportedContent {
                content_type: read_column(row, "failure_text")?,
            },
            FetchFailureKind::TooLarge => ArticleFetchFailure::TooLarge,
            FetchFailureKind::NotAFeed => ArticleFetchFailure::NotAFeed,
        },
    )
}

/// `failure` as its three columns.
fn failure_values(failure: ArticleFetchFailure) -> [Value; 3] {
    let (kind, text, code) = match failure {
        ArticleFetchFailure::Unreachable { message } => {
            (FetchFailureKind::Unreachable, Some(message), None)
        }
        ArticleFetchFailure::Status { code } => (FetchFailureKind::Status, None, Some(code)),
        ArticleFetchFailure::UnsupportedContent { content_type } => (
            FetchFailureKind::UnsupportedContent,
            Some(content_type),
            None,
        ),
        ArticleFetchFailure::TooLarge => (FetchFailureKind::TooLarge, None, None),
        ArticleFetchFailure::NotAFeed => (FetchFailureKind::NotAFeed, None, None),
    };
    [kind.into_value(), text.into_value(), code.into_value()]
}

impl ColumnGroup for ArticleSnapshot {
    const COLUMNS: &'static [&'static str] = &[
        "snapshot_state",
        "snapshot_at",
        "final_url",
        "page_title",
        "truncated",
        "failure_kind",
        "failure_text",
        "failure_code",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        Ok(
            match read_column::<ArticleSnapshotState>(row, "snapshot_state")? {
                ArticleSnapshotState::Captured => Self::Captured {
                    final_url: read_column(row, "final_url")?,
                    page_title: read_column(row, "page_title")?,
                    fetched_at: read_column(row, "snapshot_at")?,
                    truncated: read_column(row, "truncated")?,
                },
                ArticleSnapshotState::Failed => Self::Failed {
                    failure: read_failure(row)?,
                    attempted_at: read_column(row, "snapshot_at")?,
                },
            },
        )
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Self::Captured {
                final_url,
                page_title,
                fetched_at,
                truncated,
            } => vec![
                ArticleSnapshotState::Captured.into_value(),
                fetched_at.into_value(),
                final_url.into_value(),
                page_title.into_value(),
                truncated.into_value(),
                Value::Null,
                Value::Null,
                Value::Null,
            ],
            Self::Failed {
                failure,
                attempted_at,
            } => {
                let [kind, text, code] = failure_values(failure);
                vec![
                    ArticleSnapshotState::Failed.into_value(),
                    attempted_at.into_value(),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    kind,
                    text,
                    code,
                ]
            }
        }
    }
}

impl ColumnGroup for FeedStatus {
    const COLUMNS: &'static [&'static str] = &[
        "status_state",
        "status_at",
        "added",
        "failure_kind",
        "failure_text",
        "failure_code",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        Ok(match read_column::<FeedStatusState>(row, "status_state")? {
            FeedStatusState::Refreshed => Self::Refreshed {
                at: read_column(row, "status_at")?,
                added: read_column(row, "added")?,
            },
            FeedStatusState::Failed => Self::Failed {
                failure: read_failure(row)?,
                at: read_column(row, "status_at")?,
            },
        })
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Self::Refreshed { at, added } => vec![
                FeedStatusState::Refreshed.into_value(),
                at.into_value(),
                added.into_value(),
                Value::Null,
                Value::Null,
                Value::Null,
            ],
            Self::Failed { failure, at } => {
                let [kind, text, code] = failure_values(failure);
                vec![
                    FeedStatusState::Failed.into_value(),
                    at.into_value(),
                    Value::Null,
                    kind,
                    text,
                    code,
                ]
            }
        }
    }
}
