//! [`SandboxKey`], the name of a script owner's sandbox: its persistent
//! scope's and its private store's.

use std::fmt;

/// Whose sandbox a scope or a store belongs to, written
/// `extension:<script_identifier>` or `agent:<agent_id>`. The kind prefix
/// keeps owners apart: an extension identified `agent:1` is
/// `extension:agent:1`, never agent 1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SandboxKey {
    /// The extension with this script identifier.
    Extension(String),
    /// The agent with this id.
    Agent(i64),
}

impl fmt::Display for SandboxKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extension(identifier) => write!(f, "extension:{identifier}"),
            Self::Agent(id) => write!(f, "agent:{id}"),
        }
    }
}
