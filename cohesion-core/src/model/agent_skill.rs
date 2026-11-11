use cohesion_macros::{ColumnGroup, Model, TsWire};
use serde::{Deserialize, Serialize};

/// A prompt the user saved for an agent, or for every agent on a team, to
/// run by name: the user starts it from the composer, and an agent whose
/// skill `agent_can_use` sees it listed and runs it with `use_skill`.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "agent_skills", backup = Included)]
pub struct AgentSkill {
    pub id: i64,
    /// Whose it is: one agent's, or every agent's on one team.
    #[model(group)]
    pub scope: AgentSkillScope,
    /// What it is called, unique in its scope ignoring case.
    pub name: String,
    /// One line saying when to use it, which the agent reads in its list;
    /// `""` when unset.
    pub description: String,
    /// What it asks the agent to do.
    pub prompt: String,
    /// Whether the agent sees it listed and may run it itself. The user may
    /// run any skill.
    pub agent_can_use: bool,
    pub created_at: i64,
}

/// Whose a skill is. The table's check gives every row exactly one of an
/// agent and a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentSkillScope {
    /// The agents' on one team; it goes with the team.
    Team {
        /// The team it belongs to.
        team_id: i64,
    },
    /// One agent's own; it goes with the agent.
    Agent {
        /// The agent it belongs to.
        agent_id: i64,
    },
}
