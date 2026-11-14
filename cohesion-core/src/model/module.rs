//! Modules: optional areas of the app, each on or off in each workspace.

use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// An optional area of the app that a workspace can hide. Every module is on
/// in a workspace until it is turned off there. Turning one off hides it in
/// the clients only: the daemon still serves its commands, scripts and
/// agents.
///
/// Add a variant to offer one, with its arms in [`Module::title`] and
/// [`Module::description`]; its place among the variants is the order
/// Settings lists them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum Module {
    Calendar,
}

impl Module {
    /// What Settings calls it.
    pub fn title(self) -> &'static str {
        match self {
            Module::Calendar => "Calendar",
        }
    }

    /// One sentence on what it shows, for Settings.
    pub fn description(self) -> &'static str {
        match self {
            Module::Calendar => "Calendars and their events.",
        }
    }
}

/// Whether a module is on in a workspace, once it has been turned off or on
/// there; a workspace with no row for a module has it on. Unique by
/// `(workspace_id, module_name)`.
#[derive(Clone, Serialize, Deserialize, Model, Fields)]
#[model(table = "enabled_modules", backup = Included)]
pub struct EnabledModule {
    pub id: i64,
    pub workspace_id: i64,
    /// [`Module::as_str`]. Kept as text so the row of a module that no longer
    /// exists still reads, and is ignored.
    pub module_name: String,
    pub enabled: bool,
}

/// A module and whether it is on in a workspace, as Settings › Workspaces
/// lists it.
#[derive(Clone, Debug, Serialize, Deserialize, TsWire)]
pub struct ModuleSetting {
    pub module: Module,
    pub title: String,
    pub description: String,
    pub enabled: bool,
}
