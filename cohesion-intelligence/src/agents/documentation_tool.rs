use std::borrow::Cow;
use std::fmt::Write;

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{AgentMessageContent, IntelligenceAgentMessageSender};
use cohesion_db::{CreateAgentMessageCommand, DatabaseContext};
use cohesion_js::globals::Mount;
use cohesion_js::script_api::{Load, ScriptModule, modules};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::conversation_session::Branch;
use super::installed_services::{InstalledServices, SERVICES_MODULE};
use crate::activity::Activity;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::ToolFuture;
use crate::transcript_item::ToolCallStatus;

/// The code tool's index of the modules `mounts` load on demand, as
/// comment lines, after a note that those declared up front are brief.
pub(super) fn module_index(mounts: &[Mount]) -> String {
    let upfront: Vec<_> = modules(mounts, Load::Upfront)
        .map(|module| format!("`{}`", module.name))
        .collect();
    let mut text = format!(
        "// The declarations above keep only the first paragraph of each comment; the \
         `documentation` tool returns a module with all of them ({}) when you need the details.\n\
         // More of the API is declared on demand: call the `documentation` tool with a \
         module's name and read its declarations before first writing code against it. They \
         are brief too, and `full` adds the rest of each comment. That \
         only shows you text, and a script reaches the module either way, so never call it \
         for declarations still above in the thread, whichever script, turn or wake this \
         is; only when they aren't (a line saying it was loaded is not them).\n",
        upfront.join(", ")
    );
    for module in modules(mounts, Load::OnDemand) {
        _ = writeln!(text, "// - {}: {}", module.name, module.summary);
    }
    text
}

#[derive(Deserialize)]
struct DocumentationCall {
    module: String,
    #[serde(default)]
    full: bool,
}

/// What `documentation` returns for `module`: brief for one loaded on
/// demand unless `full` asks for more, and always whole for one declared up
/// front, whose brief text the code tool already shows.
fn declarations(module: &ScriptModule, full: bool) -> String {
    match (module.load, full) {
        (Load::OnDemand, false) => module.brief_source(),
        (Load::OnDemand, true) | (Load::Upfront, _) => module.source(),
    }
}

/// Hands an agent the full declarations of one of the modules its
/// sandbox's `mounts` document, those loaded on demand or those its code
/// tool declares in brief, or of the installed services as `services`.
pub(super) struct DocumentationTool {
    /// What the agent's sandbox mounts.
    pub mounts: Cow<'static, [Mount]>,
    /// The services the session's code tool indexes, whose declarations
    /// are offered only when there is one.
    pub services: InstalledServices,
    /// The database and agent whose conversation's feed shows each module
    /// it loads, as "Learning about <module>"; a fork's thread keeps its
    /// lookups as it keeps its scripts.
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl DocumentationTool {
    /// Stores a `documentation` message naming `module` in the feed, when
    /// `seat` is the conversation's.
    async fn record(&self, seat: &Seat, module: &str) {
        let Branch::Conversation { .. } = seat.branch else {
            return;
        };
        let agent_id = self.agent_id;
        if let Err(e) = self
            .database
            .send(CreateAgentMessageCommand {
                agent_id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content: AgentMessageContent::Documentation {
                    module: module.to_string(),
                },
                reply_to: None,
            })
            .await
        {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not record a documentation lookup: {e:?}"
            );
        }
    }
}

impl AgentTool for DocumentationTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "documentation",
            description: "Returns the TypeScript declarations of one module of the script API: \
                one the code tool's description lists as on demand (brief, unless `full`), one \
                it declares in brief (in full), or, as `services`, the installed services it \
                lists. It only \
                returns text: it sets nothing up, and a script reaches the same API without \
                it. Call it for a module before first writing code against it. What it \
                returns stays above for later scripts, turns and wakes, so never call it \
                again while those declarations are above; only when they aren't (a rebuilt \
                thread only lists \"loaded the `x` module\")."
                .into(),
            fields: vec![
                Field {
                    name: "module",
                    description: "The module's name, such as `network`.",
                    kind: FieldKind::String,
                    required: true,
                },
                Field {
                    name: "full",
                    description: "Adds the rest of each comment (caveats and examples), for \
                        when the brief declarations leave a question open.",
                    kind: FieldKind::Boolean,
                    required: false,
                },
            ],
        }
    }

    fn activity(&self, _arguments: &str) -> Option<Activity> {
        Some(Activity::Learning)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: DocumentationCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            if call.module == SERVICES_MODULE
                && let Some(declarations) = self.services.declarations()
            {
                self.record(seat, SERVICES_MODULE).await;
                return ToolCallStatus::Success(declarations.to_string());
            }
            match documented(&self.mounts).find(|module| module.name == call.module) {
                Some(module) => {
                    self.record(seat, module.name).await;
                    ToolCallStatus::Success(declarations(module, call.full))
                }
                None => {
                    let mut names: Vec<_> =
                        documented(&self.mounts).map(|module| module.name).collect();
                    if self.services.declarations().is_some() {
                        names.push(SERVICES_MODULE);
                    }
                    ToolCallStatus::Failure(format!(
                        "no module named `{}`; the modules are {}",
                        call.module,
                        names.join(", ")
                    ))
                }
            }
        })
    }
}

/// Every module `mounts` document, up front then on demand.
fn documented(mounts: &[Mount]) -> impl Iterator<Item = &'static ScriptModule> + '_ {
    modules(mounts, Load::Upfront).chain(modules(mounts, Load::OnDemand))
}

#[cfg(test)]
mod tests;
