use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::{IntelligenceAgentColor, ListSymbol};

/// A named group of tasks in a workspace.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "task_folders", backup = Included)]
pub struct TaskFolder {
    pub id: i64,
    pub workspace_id: i64,
    pub title: String,
    /// Where the folder sits among its workspace's folders; lower comes first.
    pub sort_order: i64,
    pub deleted: bool,
    /// Whether the folder is pinned to the sidebar, under Tasks.
    pub pinned: bool,
    /// The color the folder is drawn in, from the agents' palette, or none.
    pub color: Option<IntelligenceAgentColor>,
    /// The symbol the folder is drawn with in place of a folder, or none.
    pub symbol: Option<ListSymbol>,
}
