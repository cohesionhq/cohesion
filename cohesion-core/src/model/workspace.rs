use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// A top-level container for documents and tasks.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "workspaces", backup = Included)]
pub struct Workspace {
    /// Unique, stable identifier.
    pub id: i64,
    /// Display name.
    ///
    /// Not guaranteed unique; looking a workspace up by title is an exact,
    /// case-sensitive match.
    pub title: String,
    /// An emoji shown in place of the title's initial; none when unset.
    pub emoji: Option<String>,
    /// Whether the workspace has been soft-deleted.
    ///
    /// Lookups and listings skip deleted workspaces, so this is normally
    /// `false`.
    pub deleted: bool,
}
