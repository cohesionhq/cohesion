//! The services an agent's scripts reach through `services.use`: an
//! "Installed services" index in the code tool's description, and their
//! declarations, which the `documentation` tool hands out as `services`.

use std::fmt::Write;

use cohesion_core::model::InstalledService;
use cohesion_db::{CommandError, DatabaseContext, ListServicesCommand};
use cohesion_js::service_declarations;

/// What `documentation` is asked for to get the services' declarations.
pub(super) const SERVICES_MODULE: &str = "services";

/// The installed services, loaded when a session is built. Every session
/// sees the same view; a read-only caller finds out at call time which
/// functions it cannot call, as their declarations say.
#[derive(Clone, Default)]
pub(super) struct InstalledServices {
    section: String,
    declarations: String,
}

impl InstalledServices {
    /// The services installed now.
    pub async fn load(database: &DatabaseContext) -> Result<Self, CommandError> {
        let services = database.send(ListServicesCommand {}).await?;
        Ok(Self {
            section: section(&services),
            declarations: service_declarations(&services),
        })
    }

    /// The code tool's "Installed services" comment lines, ending in a
    /// newline; empty when there is no service.
    pub fn section(&self) -> &str {
        &self.section
    }

    /// The services' `.d.ts`, or `None` when there is no service.
    pub fn declarations(&self) -> Option<&str> {
        (!self.declarations.is_empty()).then_some(self.declarations.as_str())
    }
}

/// A header naming the `documentation` lookup, then one line per service.
fn section(services: &[InstalledService]) -> String {
    if services.is_empty() {
        return String::new();
    }
    let mut text = format!(
        "// Installed services: functions extensions offer, called as \
         `services.use(identifier).<function>(args)`. Call the `documentation` tool with \
         `{SERVICES_MODULE}` for their declarations before calling one.\n"
    );
    for service in services {
        let (identifier, name) = (&service.script_identifier, &service.name);
        _ = match service.description.as_str() {
            "" => writeln!(text, "// - services.use(\"{identifier}\"): {name}"),
            description => writeln!(
                text,
                "// - services.use(\"{identifier}\"): {name} — {description}"
            ),
        };
    }
    text
}
