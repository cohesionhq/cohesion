//! Requests about tags: names a workspace puts on its tasks.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Lists workspace `workspace_id`'s tags, in their order.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListTagsRequest {
    pub workspace_id: i64,
}

/// Creates a tag in workspace `workspace_id`. A workspace has one tag per
/// name, ignoring case.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateTagRequest {
    pub workspace_id: i64,
    pub name: String,
}

/// Renames tag `tag_id` of workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameTagRequest {
    pub workspace_id: i64,
    pub tag_id: i64,
    pub name: String,
}

/// Puts workspace `workspace_id`'s tags in the order of `tag_ids`, first to
/// last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderTagsRequest {
    pub workspace_id: i64,
    /// Every tag in the workspace, in its new order.
    pub tag_ids: Vec<i64>,
}

/// Deletes tag `tag_id` of workspace `workspace_id`, taking it off every
/// task it was on.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteTagRequest {
    pub workspace_id: i64,
    pub tag_id: i64,
}

/// Puts tag `tag_id` of workspace `workspace_id` on the workspace's task
/// `task_id`, or takes it off.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetTaggedRequest {
    pub workspace_id: i64,
    pub tag_id: i64,
    pub task_id: i64,
    pub tagged: bool,
}
