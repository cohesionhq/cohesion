use cohesion_core::{
    FromRequest, Named,
    invalidation::Invalidation,
    model::{
        AgentForwardedItem, AgentForwardedItemKind, AgentMessageContent, Attachment,
        AttachmentMessage, DocumentType, IntelligenceAgent, IntelligenceAgentMessage,
        IntelligenceAgentMessageSender,
    },
    protocol::{
        AgentForwardTarget, GetAgentMessageRequest, ImageUpload,
        ListAgentMessageAttachmentsRequest, ListAgentMessagesRequest,
    },
    value::IntoValue,
};

use super::agent_skill::SkillsForAgentCommand;
use super::document::GetDocumentCommand;
use super::emoji::emoji;
use super::image_upload::{insert_attachments, validate_images};
use super::intelligence_agent::GetIntelligenceAgentCommand;
use super::task::GetTaskCommand;
use super::workspace::GetWorkspaceCommand;
use crate::changed::{Changed, Invalidates};
use crate::retention::days_in_secs;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, ValidationError, Write,
};

/// How far back a message stays a candidate: a week.
const CANDIDATE_WINDOW_SECS: i64 = 7 * 24 * 60 * 60;

/// An agent's messages from the last week, newest first: those
/// that may still be relevant when its context rolls over.
pub struct GetCandidateAgentMessagesCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for GetCandidateAgentMessagesCommand {
    type Access = Read;
    type Response = Vec<IntelligenceAgentMessage>;

    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("agent_id = ? AND sent_at > unixepoch() - ?")
            .bind(self.agent_id)
            .bind(CANDIDATE_WINDOW_SECS)
            .order("sent_at DESC, id DESC")
            .all()?)
    }
}

/// The `limit` newest messages of an agent with an id below `before` (the
/// newest overall when `None`), oldest first.
#[derive(FromRequest)]
#[request(ListAgentMessagesRequest)]
pub struct ListAgentMessagesCommand {
    pub agent_id: i64,
    pub before: Option<i64>,
    pub limit: i64,
    pub ids: Option<Vec<i64>>,
}
impl DatabaseCommand for ListAgentMessagesCommand {
    type Access = Read;
    type Response = Vec<IntelligenceAgentMessage>;

    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let mut query = ctx.db.select().filter("agent_id = ?").bind(self.agent_id);
        if let Some(before) = self.before {
            query = query.filter("id < ?").bind(before);
        }
        if let Some(ids) = self.ids {
            query = query.filter_in_values("id", ids);
        }
        let mut messages: Vec<IntelligenceAgentMessage> =
            query.order("id DESC").limit(self.limit as u64).all()?;
        messages.reverse();
        Ok(messages)
    }
}

#[derive(FromRequest)]
#[request(GetAgentMessageRequest)]
pub struct GetAgentMessageCommand {
    pub message_id: i64,
}
impl DatabaseCommand for GetAgentMessageCommand {
    type Access = Read;
    type Response = IntelligenceAgentMessage;

    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select()
            .filter("id = ?")
            .bind(self.message_id)
            .one()
            .found()
    }
}

/// Adds a message to an agent's conversation and raises
/// `AgentMessageCreated`, attributed to the agent so its own cues skip it.
pub struct CreateAgentMessageCommand {
    pub agent_id: i64,
    /// The background run that sent it, if any.
    pub run_id: Option<i64>,
    /// What woke that run, e.g. `"Test task" was created`.
    pub run_description: Option<String>,
    pub sender: IntelligenceAgentMessageSender,
    pub content: AgentMessageContent,
    /// The message of the same conversation it replies to, if any.
    pub reply_to: Option<i64>,
}
impl DatabaseCommand for CreateAgentMessageCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        insert_message(ctx, self, Vec::new())
    }
}

/// Inserts `message` with `attachments`, so the row, its excerpt and the event
/// say how many from the start.
pub(super) fn insert_message(
    ctx: &mut CommandContext<'_>,
    message: CreateAgentMessageCommand,
    attachments: Vec<ImageUpload>,
) -> DatabaseCommandResult<Changed<IntelligenceAgentMessage>> {
    let message: IntelligenceAgentMessage = ctx
        .db
        .insert()
        .set(IntelligenceAgentMessage::AGENT_ID, message.agent_id)
        .set(IntelligenceAgentMessage::RUN_ID, message.run_id)
        .set(
            IntelligenceAgentMessage::RUN_DESCRIPTION,
            message.run_description,
        )
        .set(IntelligenceAgentMessage::SENDER, message.sender)
        .set_group(message.content)
        .set(IntelligenceAgentMessage::REPLY_TO, message.reply_to)
        .returning_one()
        .found()?;
    let message = if attachments.is_empty() {
        message
    } else {
        insert_attachments(
            ctx,
            AttachmentMessage::Agent {
                agent_message_id: message.id,
            },
            attachments,
        )?;
        ctx.db
            .select()
            .filter_eq(IntelligenceAgentMessage::ID, message.id)
            .one()
            .found()?
    };
    // The agent's own bookkeeping is news to no one else.
    if !message.message_content.is_bookkeeping() {
        let trace = ctx.trace.clone().from_agent(message.agent_id);
        ctx.events
            .send(trace.event(DatabaseEventKind::AgentMessageCreated {
                message_id: message.id,
            }));
    }
    let key = Invalidation::AgentMessage {
        agent_id: message.agent_id,
        message_id: message.id,
    };
    Ok(message.invalidating([key]))
}

/// Empties the results of scripts agents ran more than `days` ago, in the hourly
/// cleanup. The script, its title and how it ended stay. A message already
/// empty is left alone, so a pass rewrites each row once.
pub(crate) struct BlankOldCodeResultsCommand {
    pub days: i64,
}
impl DatabaseCommand for BlankOldCodeResultsCommand {
    type Access = Write;
    type Response = ();
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db.container().execute_statement(
            "UPDATE intelligence_agent_messages SET outcome_text = ''
             WHERE kind = 'codeExecution' AND outcome_text <> '' AND sent_at < ?",
            [&ctx.now.saturating_sub(days_in_secs(self.days)).into_value()],
        )?;
        Ok(().invalidating_nothing())
    }
}

/// Tells the user something is wrong with an agent's background work (a
/// cue that failed, a spend limit holding it back): a background `error`
/// message in its feed, which marks the agent unread.
pub struct ReportAgentErrorCommand {
    pub agent_id: i64,
    pub error: String,
}
impl DatabaseCommand for ReportAgentErrorCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message = ctx.run(CreateAgentMessageCommand {
            agent_id: self.agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::BackgroundAgent,
            content: AgentMessageContent::Error { error: self.error },
            reply_to: None,
        })?;
        Ok(message.invalidating_nothing())
    }
}

/// Replaces a message's content, e.g. to move a proposal through its
/// statuses.
pub struct UpdateAgentMessageContentCommand {
    pub message_id: i64,
    pub content: AgentMessageContent,
}
impl DatabaseCommand for UpdateAgentMessageContentCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message: IntelligenceAgentMessage = ctx
            .db
            .update()
            .set_group(self.content)
            .filter_eq(IntelligenceAgentMessage::ID, self.message_id)
            .returning_one()
            .found()?;
        let key = Invalidation::AgentMessage {
            agent_id: message.agent_id,
            message_id: message.id,
        };
        Ok(message.invalidating([key]))
    }
}

/// Adds the user's message to an agent's conversation, with the images
/// attached to it and, when it forwards an item, that item: a `forward`
/// whose note is `text`; when it runs one of the agent's skills, a `skill`
/// whose note is `text`; otherwise a `text` message.
pub struct CreateUserAgentMessageCommand {
    pub agent_id: i64,
    pub text: String,
    pub attachments: Vec<ImageUpload>,
    pub forward: Option<AgentForwardTarget>,
    pub skill: Option<i64>,
    /// The live workspace the user is in; when it isn't the agent's
    /// `workspace_id`, it becomes that and the message records the switch.
    pub workspace_id: Option<i64>,
}
impl DatabaseCommand for CreateUserAgentMessageCommand {
    type Access = Write;
    type Response = (IntelligenceAgentMessage, Vec<Attachment>);

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        if self.text.trim().is_empty()
            && self.attachments.is_empty()
            && self.forward.is_none()
            && self.skill.is_none()
        {
            return Err(ValidationError::in_field(
                "message",
                "Write a message or attach an image.".to_string(),
            )
            .into());
        }
        validate_images(&self.attachments)?;

        let content = match (self.forward, self.skill) {
            (Some(_), Some(_)) => {
                return Err(ValidationError::new(
                    "A message can forward an item or use a skill, not both.",
                )
                .into());
            }
            (Some(target), None) => AgentMessageContent::Forward {
                note: self.text.trim().to_string(),
                item: forwarded_item(ctx, target)?,
            },
            (None, Some(skill_id)) => {
                let skill = SkillsForAgentCommand {
                    agent_id: self.agent_id,
                }
                .handle(&mut ctx.as_read())?
                .into_iter()
                .find(|skill| skill.id == skill_id)
                .ok_or_else(|| {
                    ValidationError::in_field(
                        "skill",
                        "This agent doesn't have that skill.".to_string(),
                    )
                })?;
                AgentMessageContent::Skill {
                    name: skill.name,
                    prompt: skill.prompt,
                    note: self.text.trim().to_string(),
                }
            }
            (None, None) => AgentMessageContent::Text { text: self.text },
        };
        let switched = match self.workspace_id {
            Some(workspace_id) => switched_workspace(ctx, self.agent_id, workspace_id)?,
            None => None,
        };
        insert_message(
            ctx,
            CreateAgentMessageCommand {
                agent_id: self.agent_id,
                run_id: None,
                run_description: None,
                sender: IntelligenceAgentMessageSender::User,
                content,
                reply_to: None,
            },
            self.attachments,
        )?
        .and_then(|message| {
            let message = match switched {
                Some(workspace_id) => {
                    ctx.db
                        .update::<IntelligenceAgent>()
                        .set(IntelligenceAgent::WORKSPACE_ID, workspace_id)
                        .filter_eq(IntelligenceAgent::ID, self.agent_id)
                        .execute()?;
                    ctx.db
                        .update::<IntelligenceAgentMessage>()
                        .set(
                            IntelligenceAgentMessage::SWITCHED_WORKSPACE_ID,
                            workspace_id,
                        )
                        .filter_eq(IntelligenceAgentMessage::ID, message.id)
                        .returning_one()
                        .found()?
                }
                None => message,
            };
            let attachments = ListAgentMessageAttachmentsCommand {
                message_ids: vec![message.id],
            }
            .handle(&mut ctx.as_read())?;
            Ok((message, attachments).invalidating_nothing())
        })
    }
}

/// `workspace_id` when the user switched to it since they last told agent
/// `agent_id` where they were (or never had), `None` when it is the same;
/// an error when it is no live workspace.
fn switched_workspace(
    ctx: &mut CommandContext<'_>,
    agent_id: i64,
    workspace_id: i64,
) -> Result<Option<i64>, CommandError> {
    let workspace = GetWorkspaceCommand { id: workspace_id }.handle(&mut ctx.as_read())?;
    let agent = GetIntelligenceAgentCommand { agent_id }.handle(&mut ctx.as_read())?;
    Ok((agent.workspace_id != Some(workspace.id)).then_some(workspace.id))
}

/// Adds `message`, the agent's, with the images in `attachments` it shows
/// the user, which pass the same check as the user's.
pub struct CreateAgentMessageWithImagesCommand {
    pub message: CreateAgentMessageCommand,
    pub attachments: Vec<ImageUpload>,
}
impl DatabaseCommand for CreateAgentMessageWithImagesCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        validate_images(&self.attachments)?;
        insert_message(ctx, self.message, self.attachments)
    }
}

/// The live task or document `target` names, as a forward carries it: a
/// journal is a document, told apart by its type.
fn forwarded_item(
    ctx: &mut CommandContext<'_>,
    target: AgentForwardTarget,
) -> Result<AgentForwardedItem, CommandError> {
    Ok(match target {
        AgentForwardTarget::Task { workspace_id, id } => {
            let task = GetTaskCommand {
                workspace_id,
                task_id: id,
            }
            .handle(&mut ctx.as_read())?;
            AgentForwardedItem {
                kind: AgentForwardedItemKind::Task,
                workspace_id,
                id,
                title: task.title,
            }
        }
        AgentForwardTarget::Document { workspace_id, id } => {
            let document = GetDocumentCommand {
                workspace_id,
                document_id: id,
            }
            .handle(&mut ctx.as_read())?;
            AgentForwardedItem {
                kind: match document.document_type {
                    DocumentType::Normal => AgentForwardedItemKind::Document,
                    DocumentType::Journal => AgentForwardedItemKind::Journal,
                },
                workspace_id,
                id,
                title: document.title,
            }
        }
    })
}

/// The images attached to each of agent messages `message_ids`, by
/// message, each message's in the order sent.
#[derive(FromRequest)]
#[request(ListAgentMessageAttachmentsRequest)]
pub struct ListAgentMessageAttachmentsCommand {
    pub message_ids: Vec<i64>,
}
impl DatabaseCommand for ListAgentMessageAttachmentsCommand {
    type Access = Read;
    type Response = Vec<Attachment>;

    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter_in_values("agent_message_id", self.message_ids)
            .order("agent_message_id, id")
            .all()?)
    }
}

/// The user reacts to one of the agent's messages, or clears the reaction.
pub struct SetAgentMessageReactionCommand {
    pub message_id: i64,
    pub reaction: Option<String>,
}
impl DatabaseCommand for SetAgentMessageReactionCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message = GetAgentMessageCommand {
            message_id: self.message_id,
        }
        .handle(&mut ctx.as_read())?;
        if message.sender == IntelligenceAgentMessageSender::User {
            return Err(
                ValidationError::new("React to the agent’s messages, not your own.").into(),
            );
        }
        react(ctx, &message, self.reaction)
    }
}

/// The agent reacts to one of the user's messages in its conversation, or
/// clears its reaction.
pub struct ReactToUserMessageCommand {
    pub agent_id: i64,
    pub message_id: i64,
    pub reaction: Option<String>,
}
impl DatabaseCommand for ReactToUserMessageCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message = GetAgentMessageCommand {
            message_id: self.message_id,
        }
        .handle(&mut ctx.as_read())?;
        if message.agent_id != self.agent_id {
            return Err(CommandError::NotFound(IntelligenceAgentMessage::NOUN));
        }
        if message.sender != IntelligenceAgentMessageSender::User {
            return Err(ValidationError::new("React to the user’s messages, not your own.").into());
        }
        react(ctx, &message, self.reaction)
    }
}

/// Sets `message`'s reaction, a single emoji (blank clears it), stamping
/// when, and raises `AgentMessageReacted` unless it was cleared. Only a text
/// message or a forward takes a reaction, though one left from before can be
/// cleared.
/// Like a new message, the reaction is attributed to the agent, so its own
/// cues skip it.
fn react(
    ctx: &mut CommandContext<'_>,
    message: &IntelligenceAgentMessage,
    reaction: Option<String>,
) -> DatabaseCommandResult<Changed<IntelligenceAgentMessage>> {
    let reaction = emoji(reaction)?;
    let reacted = reaction.is_some();
    if reacted && !message.message_content.is_reactable() {
        return Err(ValidationError::new("Only a text message takes a reaction.").into());
    }
    let message: IntelligenceAgentMessage = ctx
        .db
        .update()
        .set(IntelligenceAgentMessage::REACTION, reaction)
        .set_now(IntelligenceAgentMessage::REACTED_AT)
        .filter_eq(IntelligenceAgentMessage::ID, message.id)
        .returning_one()
        .found()?;
    if reacted {
        let trace = ctx.trace.clone().from_agent(message.agent_id);
        ctx.events
            .send(trace.event(DatabaseEventKind::AgentMessageReacted {
                message_id: message.id,
            }));
    }
    let key = Invalidation::AgentMessage {
        agent_id: message.agent_id,
        message_id: message.id,
    };
    Ok(message.invalidating([key]))
}

/// Records the user's pick of option `option` in the choice in message
/// `message_id`, which must still be unanswered.
pub struct PickAgentChoiceCommand {
    pub message_id: i64,
    pub option: usize,
}
impl DatabaseCommand for PickAgentChoiceCommand {
    type Access = Write;
    type Response = IntelligenceAgentMessage;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message = GetAgentMessageCommand {
            message_id: self.message_id,
        }
        .handle(&mut ctx.as_read())?;
        let AgentMessageContent::Choice(mut choice) = message.message_content else {
            return Err(ValidationError::new("That message isn’t a choice.").into());
        };
        if choice.selected.is_some() {
            return Err(ValidationError::new("That choice has already been answered.").into());
        }
        if self.option >= choice.options.len() {
            return Err(ValidationError::new("That choice has no such option.").into());
        }
        choice.selected = Some(self.option);
        Ok(ctx
            .run(UpdateAgentMessageContentCommand {
                message_id: message.id,
                content: AgentMessageContent::Choice(choice),
            })?
            .invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
