//! Requests about workspaces as a whole.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Lists every workspace that is not deleted.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListWorkspacesRequest;

/// Creates a workspace with `title`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateWorkspaceRequest {
    pub title: String,
    /// Shown in place of the title's initial; none when absent.
    pub emoji: Option<String>,
}

/// Deletes workspace `id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteWorkspaceRequest {
    pub id: i64,
}

/// Retitles workspace `id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameWorkspaceRequest {
    pub id: i64,
    pub title: String,
}

/// Sets workspace `id`'s emoji, or clears it when `emoji` is absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetWorkspaceEmojiRequest {
    pub id: i64,
    pub emoji: Option<String>,
}

/// Puts the workspaces in the order of `ids`, first to last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderWorkspacesRequest {
    /// Every live workspace, in its new order.
    pub ids: Vec<i64>,
}

/// Search the entire workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SearchWorkspaceRequest {
    pub workspace_id: i64,
    pub query: String,
}
