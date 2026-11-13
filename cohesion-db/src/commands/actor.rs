use cohesion_core::{
    FromRequest, Projection,
    invalidation::Invalidation,
    model::{
        Actor, ActorDefinition, ActorMessageContent, ActorPermissionDecision, ActorSession,
        ActorSessionMessage, ActorSessionMessageContent, ActorSessionMove, ActorSessionSearchHit,
        ActorSessionStatus, ActorUserContent, Attachment, AttachmentMessage, ContextUsage,
    },
    protocol::{
        GetActorSessionRequest, ImageUpload, ListActorSessionAncestorsRequest,
        ListActorSessionMessageAttachmentsRequest, ListActorSessionMessagesRequest,
        ListActorSessionsRequest, ListActorsRequest, MarkActorSessionReadRequest,
        SearchActorSessionsRequest,
    },
    value::{IntoValue, Json},
};

use super::image_upload::{insert_attachments, validate_images};
use super::title::ensure_title;
use crate::changed::{Changed, Invalidates};
use crate::command_error::searched;
use crate::database_context::unix_now;
use crate::retention::days_in_secs;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, ValidationError, Write,
};

/// The longest a session's title runs, in characters, before it is cut short.
const TITLE_LIMIT: usize = 80;

/// Makes client `client_id`'s actors what `actors` lists: an actor offered
/// for the first time is created, one offered again is rewritten and
/// restored, and one no longer offered is soft-deleted, keeping its sessions.
/// Offering what is already stored writes nothing and raises nothing.
pub(super) fn sync_actors(
    ctx: &mut CommandContext<'_>,
    client_id: i64,
    actors: Vec<ActorDefinition>,
) -> DatabaseCommandResult<Changed<()>> {
    let stored: Vec<Actor> = ctx
        .db
        .select::<Actor>()
        .filter_eq(Actor::CLIENT_ID, client_id)
        .all()?;
    let mut changed = false;
    for actor in &stored {
        if actor.deleted
            || actors
                .iter()
                .any(|offered| offered.identifier == actor.identifier)
        {
            continue;
        }
        ctx.db
            .update::<Actor>()
            .set_true(Actor::DELETED)
            .filter_eq(Actor::ID, actor.id)
            .execute()?;
        changed = true;
    }
    for offered in actors {
        let unchanged = stored.iter().any(|actor| {
            !actor.deleted
                && actor.identifier == offered.identifier
                && actor.product == offered.product
                && actor.scope == offered.scope
                && actor.modes == offered.modes
                && actor.models == offered.models
        });
        if unchanged {
            continue;
        }
        ctx.db
            .insert::<Actor>()
            .set(Actor::CLIENT_ID, client_id)
            .set(Actor::IDENTIFIER, offered.identifier)
            .set(Actor::PRODUCT, offered.product)
            .set(Actor::SCOPE, offered.scope)
            .set(Actor::MODES, Json(offered.modes))
            .set(Actor::MODELS, Json(offered.models))
            .set_false(Actor::DELETED)
            .on_conflict_update(&["client_id", "identifier"])
            .execute()?;
        changed = true;
    }
    Ok(().invalidating(changed.then_some(Invalidation::Actors)))
}

/// Every actor a client offers, by client, product and scope.
#[derive(FromRequest)]
#[request(ListActorsRequest)]
pub struct ListActorsCommand;
impl DatabaseCommand for ListActorsCommand {
    type Response = Vec<Actor>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select_live::<Actor>()
            .order("clients.client_name COLLATE NOCASE, actors.product, actors.scope")
            .all()?)
    }
}

/// Live actor `id`.
pub struct GetActorCommand {
    pub id: i64,
}
impl DatabaseCommand for GetActorCommand {
    type Response = Actor;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select_live::<Actor>()
            .filter_eq(Actor::ID, self.id)
            .one()
            .found()
    }
}

/// Actor `id`, even once its client no longer offers it: a session outlives
/// its actor.
pub struct GetActorIncludingDeletedCommand {
    pub id: i64,
}
impl DatabaseCommand for GetActorIncludingDeletedCommand {
    type Response = Actor;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select::<Actor>()
            .filter_eq(Actor::ID, self.id)
            .one()
            .found()
    }
}

/// Actor sessions, latest status change first: actor `actor_id`'s, or every
/// actor's. A resolved session, and a subagent, is left out; each is still
/// there by id.
#[derive(FromRequest)]
#[request(ListActorSessionsRequest)]
pub struct ListActorSessionsCommand {
    pub actor_id: Option<i64>,
}
impl DatabaseCommand for ListActorSessionsCommand {
    type Response = Vec<ActorSession>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let select = ctx
            .db
            .select::<ActorSession>()
            .filter("status <> ? AND parent_session_id IS NULL")
            .bind(ActorSessionStatus::Resolved);
        let select = match self.actor_id {
            Some(actor_id) => select.filter_eq(ActorSession::ACTOR_ID, actor_id),
            None => select,
        };
        Ok(select.order("status_changed_at DESC, id DESC").all()?)
    }
}

/// Actor session `id`.
#[derive(FromRequest)]
#[request(GetActorSessionRequest)]
pub struct GetActorSessionCommand {
    pub id: i64,
}
impl DatabaseCommand for GetActorSessionCommand {
    type Response = ActorSession;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select::<ActorSession>()
            .filter_eq(ActorSession::ID, self.id)
            .one()
            .found()
    }
}

/// Actor sessions whose title, or something the user or the actor said in
/// them, matches FTS5 `query`, best first: a title match outweighs one in
/// the transcript, and a session is as good as its best match. Subagents are
/// left out.
#[derive(FromRequest)]
#[request(SearchActorSessionsRequest)]
pub struct SearchActorSessionsCommand {
    pub query: String,
}
impl DatabaseCommand for SearchActorSessionsCommand {
    type Response = Vec<ActorSessionSearchHit>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        // `MIN` picks each session's best match, and SQLite reads the bare
        // `excerpt` from the row it picked.
        let query = format!(
            "SELECT {} FROM actor_sessions
            INNER JOIN (
                SELECT session_id, MIN(score) AS score, excerpt FROM (
                    SELECT rowid AS session_id, 5.0 * bm25(actor_sessions_fts) AS score,
                        NULL AS excerpt
                    FROM actor_sessions_fts WHERE actor_sessions_fts MATCH ?
                    UNION ALL
                    SELECT m.session_id, bm25(actor_session_messages_fts),
                        snippet(actor_session_messages_fts, 0, '', '', '…', 24)
                    FROM actor_session_messages_fts
                    INNER JOIN actor_session_messages AS m ON m.id = actor_session_messages_fts.rowid
                    WHERE actor_session_messages_fts MATCH ?
                ) GROUP BY session_id
            ) AS hit ON hit.session_id = actor_sessions.id
            WHERE actor_sessions.parent_session_id IS NULL
            ORDER BY hit.score, actor_sessions.status_changed_at DESC",
            ActorSessionSearchHit::columns()
        );
        searched(ctx.db.container().query_rows(
            &query,
            &[self.query.clone().into_value(), self.query.into_value()],
            ActorSessionSearchHit::from_row,
        ))
    }
}

/// Marks actor session `session_id` read, now; the next stretch of work its actor
/// finishes marks it unread again (a trigger on `activities`).
#[derive(FromRequest)]
#[request(MarkActorSessionReadRequest)]
pub struct MarkActorSessionReadCommand {
    pub session_id: i64,
}
impl DatabaseCommand for MarkActorSessionReadCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let session: ActorSession = ctx
            .db
            .update()
            .set_false(ActorSession::UNREAD)
            .set_now(ActorSession::READ_AT)
            .filter_eq(ActorSession::ID, self.session_id)
            .returning_one()
            .found()?;
        let keys = session_keys(&mut ctx.as_read(), &session)?;
        Ok(session.invalidating(keys))
    }
}

/// What was said in actor session `session_id`, oldest first.
#[derive(FromRequest)]
#[request(ListActorSessionMessagesRequest)]
pub struct ListActorSessionMessagesCommand {
    pub session_id: i64,
    pub ids: Option<Vec<i64>>,
}
impl DatabaseCommand for ListActorSessionMessagesCommand {
    type Response = Vec<ActorSessionMessage>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(ctx)?;
        let query = ctx
            .db
            .select::<ActorSessionMessage>()
            .filter_eq(ActorSessionMessage::SESSION_ID, self.session_id);
        let query = match self.ids {
            Some(ids) => query.filter_in_values("id", ids),
            None => query,
        };
        Ok(query.order("id").all()?)
    }
}

/// The images attached to each of actor session messages `message_ids`, by
/// message, each message's in the order sent.
#[derive(FromRequest)]
#[request(ListActorSessionMessageAttachmentsRequest)]
pub struct ListActorSessionMessageAttachmentsCommand {
    pub message_ids: Vec<i64>,
}
impl DatabaseCommand for ListActorSessionMessageAttachmentsCommand {
    type Response = Vec<Attachment>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<Attachment>()
            .filter_in_values("actor_session_message_id", self.message_ids)
            .order("actor_session_message_id, id")
            .all()?)
    }
}

/// Starts a session for live actor `actor_id`, working, with `prompt` trimmed,
/// which must not be empty, and the images in `attachments` as its first
/// message, titled `title` (see [`SetActorSessionTitleCommand`]) or, when
/// none is given, by the prompt's first line. The mode, model and effort
/// must be ones the actor offers; an effort is the model's, or the default
/// model's when none is given.
pub struct CreateActorSessionCommand {
    pub actor_id: i64,
    pub prompt: String,
    pub title: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub attachments: Vec<ImageUpload>,
}
impl DatabaseCommand for CreateActorSessionCommand {
    type Response = (ActorSession, ActorSessionMessage);
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let actor = GetActorCommand { id: self.actor_id }.handle(&mut ctx.as_read())?;
        let prompt = self.prompt.trim();
        if prompt.is_empty() {
            return Err(ValidationError::in_field("prompt", "Say what to do.".into()).into());
        }
        let title = match &self.title {
            Some(title) => given_title(title)?,
            None => title(prompt),
        };
        validate_images(&self.attachments)?;
        if let Some(mode) = &self.mode
            && !actor.offers_mode(mode)
        {
            let message = format!("{} has no mode {mode:?}.", actor.title);
            return Err(ValidationError::in_field("mode", message).into());
        }
        let model = match &self.model {
            Some(model) => Some(actor.model(model).ok_or_else(|| {
                let message = format!("{} has no model {model:?}.", actor.title);
                ValidationError::in_field("model", message)
            })?),
            None => actor.models.first(),
        };
        if let Some(effort) = &self.effort
            && !model.is_some_and(|model| model.efforts.iter().any(|e| &e.identifier == effort))
        {
            let message = format!("The model takes no effort {effort:?}.");
            return Err(ValidationError::in_field("effort", message).into());
        }
        let session = ctx
            .db
            .insert::<ActorSession>()
            .set(ActorSession::ACTOR_ID, actor.id)
            .set(ActorSession::TITLE, title)
            .set(ActorSession::MODE, self.mode)
            .set(ActorSession::MODEL, self.model)
            .set(ActorSession::EFFORT, self.effort)
            .set(ActorSession::STATUS, ActorSessionStatus::Working)
            .returning_one()
            .found()?;
        let content = ActorSessionMessageContent::User(ActorUserContent::Text {
            text: prompt.to_string(),
        });
        let message = insert_message(ctx, session.id, content, self.attachments)?;
        Ok((session, message).invalidating([Invalidation::ActorSessions]))
    }
}

/// A session's title: the first line of `text`, such as its prompt, cut
/// short past [`TITLE_LIMIT`] characters.
fn title(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or_default().trim();
    match line.char_indices().nth(TITLE_LIMIT) {
        Some((end, _)) => format!("{}…", line[..end].trim_end()),
        None => line.to_string(),
    }
}

/// The title a session was given, as [`title`] cuts it; a blank one is
/// refused.
pub(super) fn given_title(text: &str) -> Result<String, ValidationError> {
    let title = title(text);
    ensure_title(&title)?;
    Ok(title)
}

/// Calls actor session `session_id` `title` from now on, cut to its first line
/// and short past [`TITLE_LIMIT`] characters; a blank one is refused.
/// Answers the session as it stands.
pub struct SetActorSessionTitleCommand {
    pub session_id: i64,
    pub title: String,
}
impl DatabaseCommand for SetActorSessionTitleCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let session: ActorSession = ctx
            .db
            .update()
            .set(ActorSession::TITLE, given_title(&self.title)?)
            .filter_eq(ActorSession::ID, self.session_id)
            .returning_one()
            .found()?;
        let keys = session_keys(&mut ctx.as_read(), &session)?;
        Ok(session.invalidating(keys))
    }
}

/// Adds `content` to actor session `session_id`'s transcript, with the images in
/// `attachments`, the user's or the actor's. A response of the user's must
/// answer a request of the actor's in the same session, of its kind and in its
/// shape: an answer per question, picking only its options (and in words
/// only where it allows), or a decision the request offers. The schema
/// refuses a second response to a request.
pub struct AddActorSessionMessageCommand {
    pub session_id: i64,
    pub content: ActorSessionMessageContent,
    pub attachments: Vec<ImageUpload>,
}
impl DatabaseCommand for AddActorSessionMessageCommand {
    type Response = ActorSessionMessage;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let session = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(&mut ctx.as_read())?;
        if let ActorSessionMessageContent::User(content) = &self.content
            && let Some((request_id, kind)) = content.responds_to()
        {
            let request = ctx
                .db
                .select::<ActorSessionMessage>()
                .filter_eq(ActorSessionMessage::ID, request_id)
                .filter_eq(ActorSessionMessage::SESSION_ID, session.id)
                .one()
                .found()?;
            let ActorSessionMessageContent::Actor(request) = request.content else {
                return Err(
                    ValidationError::new("Only the actor's requests take a response.").into(),
                );
            };
            if request.request() != Some(kind) {
                return Err(ValidationError::new("The response doesn't fit the request.").into());
            }
            check_response(&request, content)?;
        }
        validate_images(&self.attachments)?;
        if session.subagent.is_some()
            && let ActorSessionMessageContent::User(said) = &self.content
            && said.responds_to().is_none()
        {
            return Err(
                ValidationError::new("A subagent takes only responses to what it asked.").into(),
            );
        }
        if session.subagent.is_some() {
            let said = ctx
                .db
                .select::<ActorSessionMessage>()
                .filter_eq(ActorSessionMessage::SESSION_ID, session.id)
                .count()?;
            if said >= SUBAGENT_MESSAGE_LIMIT {
                return Err(ValidationError::new("This subagent's transcript is full.").into());
            }
        }
        let asks = match &self.content {
            ActorSessionMessageContent::Actor(said) => said.request().is_some(),
            ActorSessionMessageContent::User(said) => said.responds_to().is_some(),
        };
        let message = insert_message(ctx, session.id, self.content, self.attachments)?;
        let keys = message_keys(&mut ctx.as_read(), &session, message.id, asks)?;
        Ok(message.invalidating(keys))
    }
}

/// The most messages a subagent's transcript holds; past it, its actor's
/// reports are refused.
pub const SUBAGENT_MESSAGE_LIMIT: i64 = 2_000;

/// What message `message_id` added to, changed in or taken from `session`'s
/// transcript touches: that message, and for a subagent's request or
/// response, the session Cohesion started too, which shows it. No list
/// shows anything of a session's messages, so none is touched.
fn message_keys(
    ctx: &mut CommandContext<'_, Read>,
    session: &ActorSession,
    message_id: i64,
    asks: bool,
) -> DatabaseCommandResult<Vec<Invalidation>> {
    let message = Invalidation::ActorSessionMessage {
        session_id: session.id,
        message_id,
    };
    let mut keys = vec![message];
    if session.subagent.is_none() {
        return Ok(keys);
    }
    if asks {
        keys.push(Invalidation::ActorSession(top_session_id(ctx, session)?));
    }
    Ok(keys)
}

/// What a change to `session` itself (its status, title or what it is)
/// touches: the lists and the session, for one Cohesion started; for a
/// subagent, which no list holds, its own view, the session one level up,
/// which lists it, and the session Cohesion started, which shows its
/// requests.
pub(super) fn session_keys(
    ctx: &mut CommandContext<'_, Read>,
    session: &ActorSession,
) -> DatabaseCommandResult<Vec<Invalidation>> {
    let Some(subagent) = &session.subagent else {
        return Ok(vec![
            Invalidation::ActorSessions,
            Invalidation::ActorSession(session.id),
        ]);
    };
    let mut keys = vec![
        Invalidation::ActorSession(session.id),
        Invalidation::ActorSession(subagent.parent_session_id),
    ];
    let top = top_session_id(ctx, session)?;
    if top != subagent.parent_session_id {
        keys.push(Invalidation::ActorSession(top));
    }
    Ok(keys)
}

/// The sessions `session` runs under, the one it is a subagent of first and
/// the session Cohesion started last; none for a session Cohesion started.
pub(super) fn ancestors(
    ctx: &mut CommandContext<'_, Read>,
    session: &ActorSession,
) -> DatabaseCommandResult<Vec<ActorSession>> {
    let mut above = Vec::new();
    let mut parent = session.subagent.as_ref().map(|s| s.parent_session_id);
    while let Some(id) = parent {
        let up = GetActorSessionCommand { id }.handle(ctx)?;
        parent = up.subagent.as_ref().map(|s| s.parent_session_id);
        above.push(up);
    }
    Ok(above)
}

/// The sessions actor session `session_id` runs under, as [`ancestors`]
/// lists them.
#[derive(FromRequest)]
#[request(ListActorSessionAncestorsRequest)]
pub struct ListActorSessionAncestorsCommand {
    pub session_id: i64,
}
impl DatabaseCommand for ListActorSessionAncestorsCommand {
    type Response = Vec<ActorSession>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let session = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(ctx)?;
        ancestors(ctx, &session)
    }
}

/// The id of the session Cohesion started that `session` is, or is a
/// subagent of at any depth.
pub(super) fn top_session_id(
    ctx: &mut CommandContext<'_, Read>,
    session: &ActorSession,
) -> DatabaseCommandResult<i64> {
    let above = ancestors(ctx, session)?;
    Ok(above.last().map_or(session.id, |top| top.id))
}

/// The session Cohesion started that `session` is, or is a subagent of at
/// any depth: `session` itself when it has no parent.
pub(super) fn started_session(
    ctx: &mut CommandContext<'_, Read>,
    session: ActorSession,
) -> DatabaseCommandResult<ActorSession> {
    Ok(ancestors(ctx, &session)?.pop().unwrap_or(session))
}

/// Whether `response` is shaped as `request`, of its kind, asks.
fn check_response(
    request: &ActorMessageContent,
    response: &ActorUserContent,
) -> Result<(), ValidationError> {
    match (request, response) {
        (
            ActorMessageContent::Question { questions },
            ActorUserContent::Answers { answers, .. },
        ) => {
            if questions.len() != answers.len() {
                return Err(ValidationError::new("Answer every question."));
            }
            for (question, answer) in questions.iter().zip(answers) {
                let picked_known = answer
                    .selected
                    .iter()
                    .all(|label| question.options.iter().any(|option| &option.label == label));
                let other = answer
                    .other
                    .as_deref()
                    .is_some_and(|other| !other.trim().is_empty());
                if !picked_known
                    || (!question.multi_select && answer.selected.len() > 1)
                    || (other && !question.allows_other && !question.options.is_empty())
                    || (answer.selected.is_empty() && !other)
                {
                    return Err(ValidationError::new("An answer doesn't fit its question."));
                }
            }
            Ok(())
        }
        (
            ActorMessageContent::Permission(request),
            ActorUserContent::Permission { decision, .. },
        ) => {
            if request.decisions.contains(decision) {
                Ok(())
            } else {
                Err(ValidationError::new(decision_refused(*decision)))
            }
        }
        _ => Ok(()),
    }
}

fn decision_refused(decision: ActorPermissionDecision) -> &'static str {
    match decision {
        ActorPermissionDecision::Allow => "The actor can't be allowed this.",
        ActorPermissionDecision::AllowForSession => {
            "The actor can't be allowed this for the rest of the session."
        }
        ActorPermissionDecision::Deny => "The actor can't be denied this.",
        ActorPermissionDecision::Abort => "The actor can't be stopped from here.",
    }
}

fn insert_message(
    ctx: &mut CommandContext<'_>,
    session_id: i64,
    content: ActorSessionMessageContent,
    attachments: Vec<ImageUpload>,
) -> DatabaseCommandResult<ActorSessionMessage> {
    let message: ActorSessionMessage = ctx
        .db
        .insert::<ActorSessionMessage>()
        .set(ActorSessionMessage::SESSION_ID, session_id)
        .set_group(content)
        .returning_one()
        .found()?;
    if attachments.is_empty() {
        return Ok(message);
    }
    insert_attachments(
        ctx,
        AttachmentMessage::ActorSession {
            actor_session_message_id: message.id,
        },
        attachments,
    )?;
    ctx.db
        .select()
        .filter_eq(ActorSessionMessage::ID, message.id)
        .one()
        .found()
}

/// Stamps the actor's message `message_id`, in session `session_id`, as ended
/// now: a tool it ran or its thinking. One that has already ended keeps its
/// end; any other message is refused.
pub struct EndActorSessionMessageCommand {
    pub session_id: i64,
    pub message_id: i64,
}
impl DatabaseCommand for EndActorSessionMessageCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let mut message = ctx
            .db
            .select::<ActorSessionMessage>()
            .filter_eq(ActorSessionMessage::ID, self.message_id)
            .filter_eq(ActorSessionMessage::SESSION_ID, self.session_id)
            .one()
            .found()?;
        let ActorSessionMessageContent::Actor(said) = &mut message.content else {
            return Err(ValidationError::new("Only a tool or thinking ends.").into());
        };
        let Some(ended_at) = said.ended_at_mut() else {
            return Err(ValidationError::new("Only a tool or thinking ends.").into());
        };
        if ended_at.is_some() {
            return Ok(().invalidating_nothing());
        }
        *ended_at = Some(unix_now());
        ctx.db
            .update::<ActorSessionMessage>()
            .set_group(message.content)
            .filter_eq(ActorSessionMessage::ID, message.id)
            .execute()?;
        Ok(().invalidating([Invalidation::ActorSessionMessage {
            session_id: self.session_id,
            message_id: message.id,
        }]))
    }
}

/// Removes message `id`, which its actor never heard: the client was
/// unreachable when the user said it.
pub struct DeleteActorSessionMessageCommand {
    pub id: i64,
}
impl DatabaseCommand for DeleteActorSessionMessageCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message = ctx
            .db
            .select::<ActorSessionMessage>()
            .filter_eq(ActorSessionMessage::ID, self.id)
            .one()
            .found()?;
        ctx.db
            .delete::<ActorSessionMessage>()
            .filter_eq(ActorSessionMessage::ID, self.id)
            .execute()?;
        let session = GetActorSessionCommand {
            id: message.session_id,
        }
        .handle(&mut ctx.as_read())?;
        let asks = match &message.content {
            ActorSessionMessageContent::Actor(said) => said.request().is_some(),
            ActorSessionMessageContent::User(said) => said.responds_to().is_some(),
        };
        let keys = message_keys(&mut ctx.as_read(), &session, message.id, asks)?;
        Ok(().invalidating(keys))
    }
}

/// The agents whose `following_count` a write that took a session from `before`
/// to `after` changed, for those holding the agent list to refetch.
pub(super) fn recounted_agents(
    before: &ActorSession,
    after: &ActorSession,
) -> impl Iterator<Item = Invalidation> {
    let counted = [before.counted_by(), after.counted_by()];
    (counted[0] != counted[1])
        .then_some(counted)
        .into_iter()
        .flatten()
        .flatten()
        .map(Invalidation::Agent)
}

/// Removes actor session `id`, with what was said in it and its activities:
/// its actor never took it.
pub struct DeleteActorSessionCommand {
    pub id: i64,
}
impl DatabaseCommand for DeleteActorSessionCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let session = GetActorSessionCommand { id: self.id }.handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<ActorSession>()
            .filter_eq(ActorSession::ID, self.id)
            .execute()?;
        Ok(().invalidating(
            [Invalidation::ActorSessions, Invalidation::Activities]
                .into_iter()
                .chain(session.counted_by().map(Invalidation::Agent)),
        ))
    }
}

/// Moves actor session `session_id` as `moved` says, into its status, answering
/// the session as it stands, and raises `ActorSessionMoved` with why when that
/// changed the status of a session Cohesion started (a subagent's raises
/// nothing), and `ActorSessionResolved` too when it was resolved. A resolved
/// session stays resolved through a move that
/// [keeps it](ActorSessionMove::keeps_resolved).
pub struct SetActorSessionStatusCommand {
    pub session_id: i64,
    pub moved: ActorSessionMove,
}
impl DatabaseCommand for SetActorSessionStatusCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let status = self.moved.status();
        let before = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(&mut ctx.as_read())?;
        let update = ctx
            .db
            .update::<ActorSession>()
            .set(ActorSession::STATUS, status)
            .filter_eq(ActorSession::ID, self.session_id)
            .filter("status <> ?")
            .bind(status);
        // Its actor ending a turn leaves a session unread, and resolving one
        // reads it. A subagent is never unread.
        let update = match self.moved {
            ActorSessionMove::Finished | ActorSessionMove::Failed if before.subagent.is_none() => {
                update.set_true(ActorSession::UNREAD)
            }
            ActorSessionMove::Resolved => update
                .set_false(ActorSession::UNREAD)
                .set_now(ActorSession::READ_AT),
            _ => update,
        };
        let update = if self.moved.keeps_resolved() {
            update
                .filter("status <> ?")
                .bind(ActorSessionStatus::Resolved)
        } else {
            update
        };
        let changed = update.execute()?;
        let session = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(&mut ctx.as_read())?;
        if changed == 0 {
            return Ok(session.invalidating_nothing());
        }
        // A subagent's moves are its session's business: no hook hears them
        // and it has no follower to wake.
        if session.subagent.is_none() {
            let event = DatabaseEventKind::ActorSessionMoved {
                actor_session: session.clone(),
                moved: self.moved,
            };
            ctx.events.send(ctx.trace.event(event));
            if status == ActorSessionStatus::Resolved {
                let event = DatabaseEventKind::ActorSessionResolved {
                    actor_session: session.clone(),
                    moved: self.moved,
                };
                ctx.events.send(ctx.trace.event(event));
            }
        }
        let keys = session_keys(&mut ctx.as_read(), &session)?
            .into_iter()
            .chain(recounted_agents(&before, &session))
            .collect::<Vec<_>>();
        Ok(session.invalidating(keys))
    }
}

/// Keeps `context` as how full actor session `session_id`'s context window is,
/// in place of what its actor said before. Only the session's own view hears
/// of it: a list of sessions doesn't show it.
pub struct SetActorSessionContextCommand {
    pub session_id: i64,
    pub context: ContextUsage,
}
impl DatabaseCommand for SetActorSessionContextCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .update::<ActorSession>()
            .set_group(Some(self.context))
            .filter_eq(ActorSession::ID, self.session_id)
            .execute()?;
        Ok(().invalidating([Invalidation::ActorSession(self.session_id)]))
    }
}

/// Runs actor session `session_id` in `mode` from now on, `None` for its actor's
/// default, answering the session as it stands. A mode must be one the actor
/// offers.
pub struct SetActorSessionModeCommand {
    pub session_id: i64,
    pub mode: Option<String>,
}
impl DatabaseCommand for SetActorSessionModeCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let session = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(&mut ctx.as_read())?;
        let actor = GetActorIncludingDeletedCommand {
            id: session.actor_id,
        }
        .handle(&mut ctx.as_read())?;
        if let Some(mode) = &self.mode
            && !actor.offers_mode(mode)
        {
            let message = format!("{} has no mode {mode:?}.", actor.title);
            return Err(ValidationError::in_field("mode", message).into());
        }
        let session: ActorSession = ctx
            .db
            .update()
            .set(ActorSession::MODE, self.mode)
            .filter_eq(ActorSession::ID, self.session_id)
            .returning_one()
            .found()?;
        let keys = session_keys(&mut ctx.as_read(), &session)?;
        Ok(session.invalidating(keys))
    }
}

/// Leaves every working actor session, and every one that needs attention,
/// idle, since a stopped daemon hears from no actor, raising nothing: it
/// runs as the daemon stops, or as it starts before hooks do. Part of
/// [`ShutdownCleanupCommand`](crate::ShutdownCleanupCommand).
pub struct SettleActorSessionsCommand;
impl DatabaseCommand for SettleActorSessionsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let settled = ctx
            .db
            .update::<ActorSession>()
            .set(ActorSession::STATUS, ActorSessionStatus::Idle)
            .filter("status IN (?, ?)")
            .bind(ActorSessionStatus::Working)
            .bind(ActorSessionStatus::NeedsAttention)
            .execute()?;
        Ok(().invalidating(if settled > 0 {
            vec![Invalidation::ActorSessions]
        } else {
            Vec::new()
        }))
    }
}

/// Deletes the actor sessions that have sat resolved for `days`: a session the
/// user hasn't resolved, done or not, stays. Its messages and their images,
/// its activities and its follower go with it (their rows cascade).
pub struct DeleteOldActorSessionsCommand {
    pub days: i64,
}
impl DatabaseCommand for DeleteOldActorSessionsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let finished: Vec<ActorSession> = ctx
            .db
            .select()
            .filter("status = ? AND status_changed_at < ?")
            .bind(ActorSessionStatus::Resolved)
            .bind(ctx.now - days_in_secs(self.days))
            .all()?;
        for session in &finished {
            ctx.run(DeleteActorSessionCommand { id: session.id })?;
        }
        // Lists refetch on the keys `DeleteActorSessionCommand` raises; a session
        // open in a panel refetches on its own.
        let keys = finished
            .iter()
            .map(|session| Invalidation::ActorSession(session.id));
        Ok(().invalidating(keys))
    }
}

#[cfg(test)]
mod tests;
