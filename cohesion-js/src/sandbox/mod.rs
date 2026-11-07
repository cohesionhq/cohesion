//! Where scripts run: a [`Sandbox`] is one QuickJS runtime and context with
//! the script API mounted, limited in time and memory by its
//! [`SandboxOptions`].

mod activity;
mod agent_message;
mod caught_error;
mod notification;
mod revoked_client;
mod sandbox_options;
mod script_argument;
mod script_sandbox;
mod strip_types;

pub(crate) use caught_error::INTERRUPTED;

pub use activity::Activity;
pub use agent_message::AgentMessage;
pub use notification::Notification;
pub use revoked_client::RevokedClient;
pub use sandbox_options::SandboxOptions;
pub use script_argument::ScriptArgument;
pub use script_sandbox::Sandbox;
pub(crate) use script_sandbox::ScriptDeadline;
