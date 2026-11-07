use cohesion_core::{
    invalidation::Invalidation,
    model::{
        AgentAsk, AgentAskReply, AgentMessageContent, AgentPeer, HookKind, IntelligenceAgent,
        IntelligenceAgentMessage, IntelligenceAgentMessageSender, IntelligenceAgentRun,
    },
};

use super::intelligence_agent_message::{CreateAgentMessageCommand, insert_message};
use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    ReadableDatabase, ValidationError, Write,
};

/// What [`AskAgentCommand`] recorded: the run of the agent asked that
/// answers, and the question in each agent's conversation.
pub struct AgentAskRecord {
    /// The answering agent's run, a fork of its conversation, not yet tied
    /// to a thread.
    pub run: IntelligenceAgentRun,
    /// `askedAgent`, in the asking agent's conversation.
    pub asked: IntelligenceAgentMessage,
    /// `askedByAgent`, in the answering agent's conversation, standing for
    /// `run`.
    pub asked_by: IntelligenceAgentMessage,
    /// The asking agent, as the answering agent's conversation names it.
    pub asker: AgentPeer,
}

/// Records agent `asker_id`'s `question` to agent `recipient_id`, waiting
/// for its answer: the run that answers it, and the question in both
/// agents' conversations. Sent under the trace the answering fork runs
/// under, the asker's one hop further, since a question is a hop. An agent
/// can't ask itself, nor an agent that doesn't exist.
pub struct AskAgentCommand {
    pub asker_id: i64,
    /// The asker's run when a fork of its conversation asks; `None` from the
    /// conversation itself.
    pub asker_run_id: Option<i64>,
    pub recipient_id: i64,
    pub question: String,
}
impl DatabaseCommand for AskAgentCommand {
    type Access = Write;
    type Response = AgentAskRecord;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        if self.recipient_id == self.asker_id {
            return Err(ValidationError::in_field(
                "agent_id",
                "You can't ask yourself; ask another agent.".to_string(),
            )
            .into());
        }
        let asker = peer(ctx, self.asker_id)?;
        let recipient = peer(ctx, self.recipient_id)?;
        let run: IntelligenceAgentRun = ctx
            .db
            .insert()
            .set(IntelligenceAgentRun::AGENT_ID, self.recipient_id)
            .set(IntelligenceAgentRun::HOOK, HookKind::AgentMessage)
            .set(IntelligenceAgentRun::SENDER_AGENT_ID, Some(self.asker_id))
            .returning_one()
            .found()?;
        let asked = insert_message(
            ctx,
            CreateAgentMessageCommand {
                agent_id: self.asker_id,
                run_id: self.asker_run_id,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content: AgentMessageContent::AskedAgent(AgentAsk {
                    agent: recipient,
                    question: self.question.clone(),
                    reply: AgentAskReply::Waiting,
                }),
                reply_to: None,
            },
            Vec::new(),
        )?;
        let asked_by = insert_message(
            ctx,
            CreateAgentMessageCommand {
                agent_id: self.recipient_id,
                run_id: Some(run.id),
                run_description: None,
                sender: IntelligenceAgentMessageSender::BackgroundAgent,
                content: AgentMessageContent::AskedByAgent(AgentAsk {
                    agent: asker.clone(),
                    question: self.question,
                    reply: AgentAskReply::Waiting,
                }),
                reply_to: None,
            },
            Vec::new(),
        )?;
        let run_key = Invalidation::Agent(run.agent_id);
        asked.and_then(|asked| {
            Ok(asked_by
                .map(|asked_by| AgentAskRecord {
                    run,
                    asked,
                    asked_by,
                    asker,
                })
                .and([run_key]))
        })
    }
}

/// Agent `agent_id` as another agent's conversation names it.
fn peer(ctx: &mut CommandContext<'_>, agent_id: i64) -> Result<AgentPeer, CommandError> {
    let agent: IntelligenceAgent = ctx
        .db
        .select()
        .filter_eq(IntelligenceAgent::ID, agent_id)
        .one()
        .found()?;
    Ok(AgentPeer {
        id: agent.id,
        name: agent.agent_name,
    })
}

/// Settles a question still waiting with `reply`, in both of its messages,
/// `asked` (the asker's) and `asked_by` (the answerer's), and returns
/// `asked` as it stands. A question no longer waiting keeps the reply it
/// has.
pub struct AnswerAgentAskCommand {
    pub asked: i64,
    pub asked_by: i64,
    pub reply: AgentAskReply,
}
impl DatabaseCommand for AnswerAgentAskCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let waiting: Vec<IntelligenceAgentMessage> = ctx
            .db
            .select()
            .filter(WAITING)
            .filter("intelligence_agent_messages.id IN (?, ?)")
            .bind(self.asked)
            .bind(self.asked_by)
            .all()?;
        let settled = settle(ctx, waiting, &self.reply)?;
        let asked: IntelligenceAgentMessage = ctx
            .db
            .select()
            .filter_eq(IntelligenceAgentMessage::ID, self.asked)
            .one()
            .found()?;
        Ok(settled.map(|()| asked))
    }
}

/// Why a question went unanswered when the daemon stopped, as both agents'
/// conversations say it and as the asker's waiting call hears it.
pub const ASK_STOPPED_REASON: &str = "Cohesion stopped before it answered.";

/// Settles every question still waiting as unanswered, since no agent is
/// answering it: run as the daemon stops, and as it starts after a crash.
pub struct SettleAgentAsksCommand;
impl DatabaseCommand for SettleAgentAsksCommand {
    type Access = Write;
    type Response = ();

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let waiting: Vec<IntelligenceAgentMessage> = ctx.db.select().filter(WAITING).all()?;
        let reply = AgentAskReply::Unanswered {
            reason: ASK_STOPPED_REASON.to_string(),
        };
        settle(ctx, waiting, &reply)
    }
}

/// The condition for a question's message whose reply is still waiting.
const WAITING: &str = "intelligence_agent_messages.reply = 'waiting'";

/// Gives each of `messages`, questions' messages, `reply`.
fn settle(
    ctx: &mut CommandContext<'_>,
    messages: Vec<IntelligenceAgentMessage>,
    reply: &AgentAskReply,
) -> DatabaseCommandResult<Changed<()>> {
    let mut keys = Vec::new();
    for message in messages {
        let content = match message.message_content {
            AgentMessageContent::AskedAgent(ask) => AgentMessageContent::AskedAgent(AgentAsk {
                reply: reply.clone(),
                ..ask
            }),
            AgentMessageContent::AskedByAgent(ask) => AgentMessageContent::AskedByAgent(AgentAsk {
                reply: reply.clone(),
                ..ask
            }),
            _ => unreachable!("only questions' messages wait for a reply"),
        };
        ctx.db
            .update::<IntelligenceAgentMessage>()
            .set_group(content)
            .filter_eq(IntelligenceAgentMessage::ID, message.id)
            .execute()?;
        keys.push(Invalidation::Agent(message.agent_id));
    }
    Ok(().invalidating(keys))
}

#[cfg(test)]
mod tests;
