//! `cohesion://` links: how the user, agents and clients name an item in
//! text, as a Markdown link's target.

use std::fmt;
use std::str::FromStr;

/// What a `cohesion://` link names.
///
/// - `cohesion://task/<workspace>/<id>` and `cohesion://document/<workspace>/<id>`
///   (a journal entry is a document); `cohesion://task/<id>` and
///   `cohesion://document/<id>`, without the workspace, as links were first
///   written, still name one, since their ids are unique across workspaces.
/// - `cohesion://resource/<workspace>/<id>`.
/// - `cohesion://agent/<id>`, `cohesion://actor-session/<id>` and
///   `cohesion://article/<id>`, which belong to no workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemLink {
    Task { workspace_id: Option<i64>, id: i64 },
    Document { workspace_id: Option<i64>, id: i64 },
    Resource { workspace_id: i64, id: i64 },
    Agent { id: i64 },
    ActorSession { id: i64 },
    Article { id: i64 },
}

/// Text that isn't a `cohesion://` link to an item.
#[derive(Debug, PartialEq, Eq)]
pub struct NotAnItemLink;

impl fmt::Display for NotAnItemLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("That isn’t a link to a Cohesion item, such as cohesion://task/3/42.")
    }
}

impl FromStr for ItemLink {
    type Err = NotAnItemLink;

    fn from_str(link: &str) -> Result<Self, Self::Err> {
        let path = link.strip_prefix("cohesion://").ok_or(NotAnItemLink)?;
        let mut parts = path.split('/');
        let kind = parts.next().ok_or(NotAnItemLink)?;
        let ids = parts.map(id).collect::<Result<Vec<_>, _>>()?;
        Ok(match (kind, ids.as_slice()) {
            ("task", &[id]) => ItemLink::Task {
                workspace_id: None,
                id,
            },
            ("task", &[workspace_id, id]) => ItemLink::Task {
                workspace_id: Some(workspace_id),
                id,
            },
            ("document", &[id]) => ItemLink::Document {
                workspace_id: None,
                id,
            },
            ("document", &[workspace_id, id]) => ItemLink::Document {
                workspace_id: Some(workspace_id),
                id,
            },
            ("resource", &[workspace_id, id]) => ItemLink::Resource { workspace_id, id },
            ("agent", &[id]) => ItemLink::Agent { id },
            ("actor-session", &[id]) => ItemLink::ActorSession { id },
            ("article", &[id]) => ItemLink::Article { id },
            _ => return Err(NotAnItemLink),
        })
    }
}

/// One id of a link: digits only, so `+3`, `-3` and `03x` aren't ids.
fn id(part: &str) -> Result<i64, NotAnItemLink> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(NotAnItemLink);
    }
    part.parse().map_err(|_| NotAnItemLink)
}

/// The link as written, with its workspace when it has one.
impl fmt::Display for ItemLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ItemLink::Task {
                workspace_id: Some(ws),
                id,
            } => write!(f, "cohesion://task/{ws}/{id}"),
            ItemLink::Task {
                workspace_id: None,
                id,
            } => write!(f, "cohesion://task/{id}"),
            ItemLink::Document {
                workspace_id: Some(ws),
                id,
            } => write!(f, "cohesion://document/{ws}/{id}"),
            ItemLink::Document {
                workspace_id: None,
                id,
            } => write!(f, "cohesion://document/{id}"),
            ItemLink::Resource { workspace_id, id } => {
                write!(f, "cohesion://resource/{workspace_id}/{id}")
            }
            ItemLink::Agent { id } => write!(f, "cohesion://agent/{id}"),
            ItemLink::ActorSession { id } => write!(f, "cohesion://actor-session/{id}"),
            ItemLink::Article { id } => write!(f, "cohesion://article/{id}"),
        }
    }
}

#[cfg(test)]
mod tests;
