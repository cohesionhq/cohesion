//! Subagents: sessions an actor launches from inside one Cohesion started,
//! each an `actor_sessions` row one level below another, which its client
//! reports into under the session Cohesion started, naming it by its own
//! key.

use cohesion_core::{
    FromRequest,
    model::{
        ActorMessageContent, ActorSession, ActorSessionMessage, ActorSessionMessageContent,
        ActorSessionMove, ActorSessionStatus, ActorSubagent,
    },
    protocol::{
        ListActorSessionFamilyRequest, ListActorSubagentsRequest,
        ListUnansweredSubagentRequestsRequest, SubagentLaunch,
    },
};

use super::actor::{
    GetActorSessionCommand, SetActorSessionStatusCommand, ancestors, given_title, session_keys,
    started_session, top_session_id,
};
use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// The most subagents one session Cohesion started holds, at every depth.
pub const SUBAGENT_LIMIT: i64 = 256;
/// The deepest a subagent sits below the session Cohesion started.
pub const SUBAGENT_DEPTH_LIMIT: usize = 4;
/// The longest a client's key for a subagent runs, in bytes.
const KEY_LIMIT: usize = 256;
/// The longest a subagent's kind runs, in bytes.
const AGENT_TYPE_LIMIT: usize = 64;
/// The longest the model a subagent runs on runs, in bytes.
const MODEL_LIMIT: usize = 128;

/// The rows below session `?`, at any depth, as a condition on
/// `actor_sessions`.
const BELOW: &str = "actor_sessions.id IN (
    WITH RECURSIVE below(id) AS (
        SELECT id FROM actor_sessions WHERE parent_session_id = ?
        UNION ALL
        SELECT s.id FROM actor_sessions AS s INNER JOIN below ON s.parent_session_id = below.id
    ) SELECT id FROM below)";

/// Session `session_id`, which must be one Cohesion started: a subagent is
/// reported on under its session.
fn top_session(
    ctx: &mut CommandContext<'_, Read>,
    session_id: i64,
) -> DatabaseCommandResult<ActorSession> {
    let session = GetActorSessionCommand { id: session_id }.handle(ctx)?;
    if session.subagent.is_some() {
        let message = "Report on a subagent under the session Cohesion started.".to_string();
        return Err(ValidationError::in_field("session_id", message).into());
    }
    Ok(session)
}

/// The subagent `key` names among `top`'s actor's, if there is one; one
/// under another session is refused.
fn find_subagent(
    ctx: &mut CommandContext<'_, Read>,
    top: &ActorSession,
    key: &str,
) -> DatabaseCommandResult<Option<ActorSession>> {
    let found = ctx
        .db
        .select::<ActorSession>()
        .filter_eq(ActorSession::ACTOR_ID, top.actor_id)
        .filter("subagent_key = ?")
        .bind(key.to_string())
        .one()?;
    if let Some(subagent) = &found
        && top_session_id(ctx, subagent)? != top.id
    {
        let message = format!("Subagent {key:?} belongs to another session.");
        return Err(ValidationError::in_field("subagent", message).into());
    }
    Ok(found)
}

/// `text`, trimmed, at most `limit` bytes, or `None` when blank.
fn short_text(
    text: Option<String>,
    limit: usize,
    field: &'static str,
) -> Result<Option<String>, ValidationError> {
    let Some(text) = text.map(|text| text.trim().to_string()) else {
        return Ok(None);
    };
    if text.len() > limit {
        let message = format!("Keep {field} to {limit} bytes.");
        return Err(ValidationError::in_field(field, message));
    }
    Ok((!text.is_empty()).then_some(text))
}

/// The session Cohesion started that actor session `id` is, or is a subagent
/// of at any depth.
pub struct GetTopActorSessionCommand {
    pub id: i64,
}
impl DatabaseCommand for GetTopActorSessionCommand {
    type Response = ActorSession;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let session = GetActorSessionCommand { id: self.id }.handle(ctx)?;
        started_session(ctx, session)
    }
}

/// Subagent `key` of session `session_id`, one Cohesion started, as its
/// client launched it, at any depth; one never launched is refused.
pub struct GetActorSubagentCommand {
    pub session_id: i64,
    pub key: String,
}
impl DatabaseCommand for GetActorSubagentCommand {
    type Response = ActorSession;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let top = top_session(ctx, self.session_id)?;
        find_subagent(ctx, &top, &self.key)?.ok_or_else(|| {
            let message = format!("Launch subagent {:?} first.", self.key);
            ValidationError::in_field("subagent", message).into()
        })
    }
}

/// Launches subagent `key` of session `session_id`, one Cohesion started, as
/// `launch` says, answering it: the first launch adds it, working, under the
/// session or the subagent `launch.parent` names, at most
/// [`SUBAGENT_DEPTH_LIMIT`] deep and [`SUBAGENT_LIMIT`] in all; a launch again
/// makes it what this one says, under the same parent, its status as it
/// stands. `launch.message_id` must be a tool use in the transcript one level
/// up.
pub struct LaunchActorSubagentCommand {
    pub session_id: i64,
    pub key: String,
    pub launch: SubagentLaunch,
}
impl DatabaseCommand for LaunchActorSubagentCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let SubagentLaunch {
            title,
            parent,
            message_id,
            agent_type,
            model,
            background,
        } = self.launch;
        let key = self.key.trim().to_string();
        if key.is_empty() || key.len() > KEY_LIMIT {
            let message = format!("Name a subagent in 1 to {KEY_LIMIT} bytes.");
            return Err(ValidationError::in_field("subagent", message).into());
        }
        let title = given_title(&title)?;
        let agent_type = short_text(agent_type, AGENT_TYPE_LIMIT, "agent_type")?;
        let model = short_text(model, MODEL_LIMIT, "model")?;
        let (top, above, launched) = {
            let read = &mut ctx.as_read();
            let top = top_session(read, self.session_id)?;
            let above = match &parent {
                None => top.clone(),
                Some(parent) => find_subagent(read, &top, parent)?.ok_or_else(|| {
                    let message = format!("Launch subagent {parent:?} before one under it.");
                    ValidationError::in_field("parent", message)
                })?,
            };
            if let Some(message_id) = message_id {
                let launched_by = read
                    .db
                    .select::<ActorSessionMessage>()
                    .filter_eq(ActorSessionMessage::ID, message_id)
                    .filter_eq(ActorSessionMessage::SESSION_ID, above.id)
                    .one()?;
                if !matches!(
                    launched_by.map(|message| message.content),
                    Some(ActorSessionMessageContent::Actor(
                        ActorMessageContent::ToolUse { .. }
                    ))
                ) {
                    let message = "Name a tool use of the session the subagent runs under.";
                    return Err(ValidationError::in_field("message_id", message.into()).into());
                }
            }
            let launched = find_subagent(read, &top, &key)?;
            if launched.is_none() {
                if ancestors(read, &above)?.len() + 1 > SUBAGENT_DEPTH_LIMIT {
                    let message =
                        format!("A subagent sits at most {SUBAGENT_DEPTH_LIMIT} levels deep.");
                    return Err(ValidationError::in_field("parent", message).into());
                }
                let count = read
                    .db
                    .select::<ActorSession>()
                    .filter(BELOW)
                    .bind(top.id)
                    .count()?;
                if count >= SUBAGENT_LIMIT {
                    let message = format!("A session launches at most {SUBAGENT_LIMIT} subagents.");
                    return Err(ValidationError::in_field("subagent", message).into());
                }
            }
            (top, above, launched)
        };
        let subagent = ActorSubagent {
            parent_session_id: above.id,
            key: key.clone(),
            agent_type,
            message_id,
            background,
        };
        let session: ActorSession = match launched {
            Some(launched) => {
                let moved = launched
                    .subagent
                    .as_ref()
                    .is_some_and(|was| was.parent_session_id != above.id);
                if moved {
                    let message = format!("Subagent {key:?} runs under another session.");
                    return Err(ValidationError::in_field("parent", message).into());
                }
                ctx.db
                    .update()
                    .set(ActorSession::TITLE, title)
                    .set(ActorSession::MODEL, model)
                    .set_group(Some(subagent))
                    .filter_eq(ActorSession::ID, launched.id)
                    .returning_one()
                    .found()?
            }
            None => ctx
                .db
                .insert::<ActorSession>()
                .set(ActorSession::ACTOR_ID, top.actor_id)
                .set(ActorSession::TITLE, title)
                .set(ActorSession::MODEL, model)
                .set(ActorSession::STATUS, ActorSessionStatus::Working)
                .set_group(Some(subagent))
                .returning_one()
                .found()?,
        };
        let keys = session_keys(&mut ctx.as_read(), &session)?;
        Ok(session.invalidating(keys))
    }
}

/// The subagents launched from actor session `session_id`, one level down,
/// first launched first.
#[derive(FromRequest)]
#[request(ListActorSubagentsRequest)]
pub struct ListActorSubagentsCommand {
    pub session_id: i64,
}
impl DatabaseCommand for ListActorSubagentsCommand {
    type Response = Vec<ActorSession>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(ctx)?;
        Ok(ctx
            .db
            .select::<ActorSession>()
            .filter("parent_session_id = ?")
            .bind(self.session_id)
            .order("started_at, id")
            .all()?)
    }
}

/// Actor session `session_id`'s family: the session Cohesion started that it
/// is, or runs under at any depth, then every subagent beneath that, each
/// right after the one it runs under and its earlier siblings' own, siblings
/// first launched first.
#[derive(FromRequest)]
#[request(ListActorSessionFamilyRequest)]
pub struct ListActorSessionFamilyCommand {
    pub session_id: i64,
}
impl DatabaseCommand for ListActorSessionFamilyCommand {
    type Response = Vec<ActorSession>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let session = GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(ctx)?;
        let top = started_session(ctx, session)?;
        let below: Vec<ActorSession> = ctx
            .db
            .select::<ActorSession>()
            .filter(BELOW)
            .bind(top.id)
            .order("started_at, id")
            .all()?;
        let mut family = vec![top];
        let mut index = 0;
        // Each session is followed by its own subagents before its next sibling.
        while index < family.len() {
            let parent = family[index].id;
            let children: Vec<ActorSession> = below
                .iter()
                .filter(|s| s.subagent.as_ref().map(|s| s.parent_session_id) == Some(parent))
                .cloned()
                .collect();
            family.splice(index + 1..index + 1, children);
            index += 1;
        }
        Ok(family)
    }
}

/// The requests waiting on the user in actor session `session_id`'s
/// subagents, at any depth, oldest first: questions, permissions and plans
/// nothing has responded to, of subagents working or needing attention.
#[derive(FromRequest)]
#[request(ListUnansweredSubagentRequestsRequest)]
pub struct ListUnansweredSubagentRequestsCommand {
    pub session_id: i64,
}
impl DatabaseCommand for ListUnansweredSubagentRequestsCommand {
    type Response = Vec<ActorSessionMessage>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        GetActorSessionCommand {
            id: self.session_id,
        }
        .handle(ctx)?;
        let going = format!(
            "sender = 'actor' AND kind IN ('question', 'permission', 'plan')
            AND NOT {answered}
            AND session_id IN (SELECT actor_sessions.id FROM actor_sessions
                WHERE status IN (?, ?) AND {BELOW})",
            answered = ActorSessionMessage::ANSWERED_SQL,
        );
        Ok(ctx
            .db
            .select::<ActorSessionMessage>()
            .filter(&going)
            .bind(ActorSessionStatus::Working)
            .bind(ActorSessionStatus::NeedsAttention)
            .bind(self.session_id)
            .order("id")
            .all()?)
    }
}

/// Which subagents [`SettleActorSubagentsCommand`] settles.
#[derive(Clone, Copy, Debug)]
pub enum SubagentsOf {
    /// Every subagent of client `.0`'s actors, which went away.
    Client(i64),
    /// Every subagent below session `.0`, at any depth, which was stopped.
    Session(i64),
}

/// Leaves the subagents `of` names that are working, or need attention, idle
/// as `moved` says (interrupted or stopped): nothing reports on them any
/// longer. Like any subagent's move, it raises no event.
pub struct SettleActorSubagentsCommand {
    pub of: SubagentsOf,
    pub moved: ActorSessionMove,
}
impl DatabaseCommand for SettleActorSubagentsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let going = ctx
            .db
            .select::<ActorSession>()
            .filter("parent_session_id IS NOT NULL AND status IN (?, ?)")
            .bind(ActorSessionStatus::Working)
            .bind(ActorSessionStatus::NeedsAttention);
        let going: Vec<ActorSession> = match self.of {
            SubagentsOf::Client(client_id) => going
                .filter("actor_id IN (SELECT id FROM actors WHERE client_id = ?)")
                .bind(client_id),
            SubagentsOf::Session(session_id) => going.filter(BELOW).bind(session_id),
        }
        .all()?;
        // Each move names the keys it touched.
        for session in going {
            ctx.run(SetActorSessionStatusCommand {
                session_id: session.id,
                moved: self.moved,
            })?;
        }
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
