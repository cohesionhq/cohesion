//! Requests about modules: which are on in a workspace, and turning one on
//! or off there.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::Module;

/// Answers every module and whether it is on in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListWorkspaceModulesRequest {
    pub workspace_id: i64,
}

/// Turns `module` on or off in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetModuleEnabledRequest {
    pub workspace_id: i64,
    pub module: Module,
    pub enabled: bool,
}
