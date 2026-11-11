//! What a session of an agent's conversation is built from, the same for
//! the conversation and for a fork of it: the system prompt and the tools,
//! built with no session in reach ([`AgentTool`]), so a fork's prompt is the
//! conversation's, and a provider that caches prompts finds it cached. So
//! every session has `reply_to_agent`, though only a fork owing a reply can
//! use it.

use std::sync::{Arc, Mutex};

use cohesion_core::model::{
    AgentMessageContent, AgentSkill, ContextRebuildReason, IntelligenceAgent,
    IntelligenceAgentMessage, IntelligenceAgentMessageSender, Memory, Workspace,
};
use cohesion_db::{
    CommandError, CreateAgentMessageCommand, DatabaseContext, GetIntelligenceAgentCommand,
    GetProfileCommand, GetStorageSettingsCommand, GetWorkspaceCommand, ListAgentMessagesCommand,
    ListAgentTeamsCommand, ListIntelligenceAgentsCommand, ListMemoriesCommand,
    SetAgentContextTokensCommand, SkillsForAgentCommand, TierModel,
};
use cohesion_js::AgentRole;

use super::agenda::{AgendaTool, AgendaToolKind};
use super::agent_error::AgentError;
use super::agent_messenger::{AgentMessenger, OwedReply};
use super::agent_tool::{AgentTool, Seat, seated};
use super::ask_tool::AskAgentTool;
use super::conversation_mark::ThreadMark;
use super::documentation_tool::DocumentationTool;
use super::foreground_code_tool::ForegroundCodeTool;
use super::foreground_turn::ForegroundTurn;
use super::image_tray::ImageTray;
use super::installed_services::InstalledServices;
use super::memory_tools::{ForgetTool, MemoriesTool, RememberTool};
use super::message_tool::{MessageKind, MessageTool};
use super::react_tool::ReactTool;
use super::read_message_tool::ReadMessageTool;
use super::reply_tool::ReplyTool;
use super::skill_tool::UseSkillTool;
use crate::connected_services::ConnectedServices;
use crate::session::{Session, SessionConstructionError};
use crate::system_prompt::{Roster, SystemPrompt};
use crate::tools::SessionTool;

/// How far back a thread looks for the conversation's messages it hasn't
/// heard.
const CATCH_UP_LIMIT: i64 = 100;

/// Which session of an agent's conversation the tools serve.
pub(super) enum Branch {
    /// The conversation itself: `sent` gathers what `send_message` sends
    /// this turn and `reacted` the message `react` last reacted to, for the
    /// host's working set.
    Conversation {
        sent: Arc<Mutex<Vec<IntelligenceAgentMessage>>>,
        reacted: Arc<Mutex<Option<IntelligenceAgentMessage>>>,
    },
    /// A fork of the conversation, by its run, which marks what it posts
    /// and runs; `owed` is the reply it owes when another agent's question
    /// woke it and waits for one.
    Fork {
        run_id: i64,
        owed: Option<Arc<OwedReply>>,
    },
}

/// The key that groups an agent's conversation and its forks at a provider
/// that routes by it, so a fork finds the conversation's prompt cached.
pub(super) fn cache_key(agent_id: i64) -> String {
    format!("cohesion-agent-{agent_id}-foreground")
}

/// A new thread of `agent`'s conversation on `model`, under its system
/// prompt as it is now, offering `tools`.
pub(super) async fn new_session(
    messenger: &AgentMessenger,
    agent: &IntelligenceAgent,
    model: TierModel,
    tools: Vec<Box<dyn SessionTool>>,
) -> Result<Session, AgentError> {
    let database = messenger.database();
    Session::new(
        database.clone(),
        messenger.credentials().clone(),
        model,
        system_prompt(database, agent).await?,
        tools,
        cache_key(agent.id),
    )
    .await
    .map_err(AgentError::SessionConstruction)
}

/// A copy of the thread `mark` holds of agent `agent_id`'s conversation, on
/// `model`, offering `tools`, which match the thread's, so its provider
/// finds it cached.
pub(super) async fn fork_session(
    messenger: &AgentMessenger,
    agent_id: i64,
    model: TierModel,
    mark: &ThreadMark,
    tools: Vec<Box<dyn SessionTool>>,
) -> Result<Session, SessionConstructionError> {
    Session::fork(
        messenger.database().clone(),
        messenger.credentials().clone(),
        model,
        mark.thread_id,
        mark.items,
        mark.heard.clone(),
        mark.memory_limit,
        tools,
        cache_key(agent_id),
    )
    .await
}

/// Every memory agent `agent_id` sees, its own, then its team's, then the
/// global ones, for its system prompt. Each scope keeps few, so all of them
/// fit.
pub(super) async fn agent_memories(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<Vec<Memory>, CommandError> {
    database
        .send(ListMemoriesCommand {
            agent_id: Some(agent_id),
        })
        .await
}

/// The skills agent `agent_id` may use itself, by name: its own and its
/// team's whose `agent_can_use` is set, its own winning a name.
pub(super) async fn usable_skills(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<Vec<AgentSkill>, CommandError> {
    let mut skills = database.send(SkillsForAgentCommand { agent_id }).await?;
    skills.retain(|skill| skill.agent_can_use);
    Ok(skills)
}

/// The workspace the user is in as agent `agent_id` was last told, for its
/// status; `None` when it never was or that workspace is gone.
pub(super) async fn user_workspace(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<Option<Workspace>, CommandError> {
    let agent = database
        .send(GetIntelligenceAgentCommand { agent_id })
        .await?;
    let Some(id) = agent.workspace_id else {
        return Ok(None);
    };
    match database.send(GetWorkspaceCommand { id }).await {
        Ok(workspace) => Ok(Some(workspace)),
        Err(CommandError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The newest of agent `agent_id`'s messages, oldest first, sent before
/// message `before` (or through now), as far back as a thread catching up
/// looks.
pub(super) async fn recent(
    database: &DatabaseContext,
    agent_id: i64,
    before: Option<i64>,
) -> Result<Vec<IntelligenceAgentMessage>, CommandError> {
    database
        .send(ListAgentMessagesCommand {
            agent_id,
            before,
            limit: CATCH_UP_LIMIT,
        })
        .await
}

/// Records that agent `agent_id`'s conversation has no live thread, once
/// its host drops one or starts another, so clients show no count until the
/// next request is answered.
pub(super) async fn forget_thread(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<(), CommandError> {
    database
        .send(SetAgentContextTokensCommand {
            agent_id,
            tokens: None,
        })
        .await
}

/// The system prompt of `agent`'s conversation, which lists its teammates,
/// the other agents it can ask, who the user is, its memories and each
/// scope's capacity as they are now.
pub(super) async fn system_prompt(
    database: &DatabaseContext,
    agent: &IntelligenceAgent,
) -> Result<SystemPrompt, CommandError> {
    let agents = database.send(ListIntelligenceAgentsCommand {}).await?;
    let teams = database.send(ListAgentTeamsCommand {}).await?;
    let roster = Roster::of(agent, &agents, &teams);
    Ok(SystemPrompt {
        profile: agent.into(),
        roster,
        user: database.send(GetProfileCommand {}).await?.into(),
        memories: agent_memories(database, agent.id).await?,
        memory_limit: database
            .send(GetStorageSettingsCommand {})
            .await?
            .memory_limit,
    })
}

/// Says in agent `agent_id`'s feed that its context was just rebuilt, for
/// `reason`.
pub(super) async fn note_rebuilt(
    database: &DatabaseContext,
    agent_id: i64,
    reason: ContextRebuildReason,
) -> Result<(), CommandError> {
    database
        .send(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::RebuiltContext {
                reason: Some(reason),
            },
            reply_to: None,
        })
        .await?;
    Ok(())
}

/// The tools of agent `agent_id`'s conversation, as `branch` uses them,
/// with `turn` stopping its scripts and waits.
pub(super) fn tools(
    messenger: &AgentMessenger,
    agent_id: i64,
    turn: Arc<ForegroundTurn>,
    services: InstalledServices,
    connected_services: ConnectedServices,
    branch: Branch,
) -> Vec<Box<dyn SessionTool>> {
    seated(
        agent_tools(messenger, agent_id, services, connected_services),
        Seat { turn, branch },
    )
}

/// The tools of agent `agent_id`'s conversation, built with no session in
/// reach, so every session's are the same.
fn agent_tools(
    messenger: &AgentMessenger,
    agent_id: i64,
    services: InstalledServices,
    connected_services: ConnectedServices,
) -> Vec<Box<dyn AgentTool>> {
    let database = messenger.database();
    let images = ImageTray::default();
    vec![
        Box::new(ForegroundCodeTool {
            engine: messenger.engine().clone(),
            database: database.clone(),
            agent_id,
            services: services.clone(),
            connected_services,
            images: images.clone(),
        }),
        Box::new(DocumentationTool {
            mounts: AgentRole::DOCUMENTED.mounts(),
            services,
            database: database.clone(),
            agent_id,
        }),
        Box::new(MessageTool {
            database: database.clone(),
            agent_id,
            kind: MessageKind::Text { images },
        }),
        Box::new(MessageTool {
            database: database.clone(),
            agent_id,
            kind: MessageKind::Choice,
        }),
        Box::new(MessageTool {
            database: database.clone(),
            agent_id,
            kind: MessageKind::Note,
        }),
        Box::new(AskAgentTool {
            messenger: messenger.clone(),
            agent_id,
        }),
        Box::new(ReplyTool),
        Box::new(ReactTool {
            database: database.clone(),
            agent_id,
        }),
        Box::new(ReadMessageTool {
            database: database.clone(),
            agent_id,
        }),
        Box::new(RememberTool {
            database: database.clone(),
            agent_id,
        }),
        Box::new(ForgetTool {
            database: database.clone(),
            agent_id,
        }),
        Box::new(MemoriesTool {
            database: database.clone(),
            agent_id,
        }),
        Box::new(AgendaTool {
            database: database.clone(),
            agent_id,
            kind: AgendaToolKind::Append,
        }),
        Box::new(AgendaTool {
            database: database.clone(),
            agent_id,
            kind: AgendaToolKind::Finish,
        }),
        Box::new(AgendaTool {
            database: database.clone(),
            agent_id,
            kind: AgendaToolKind::Reset,
        }),
        Box::new(UseSkillTool {
            database: database.clone(),
            agent_id,
        }),
    ]
}
