use cohesion_core::model::ActorUserContent;
use cohesion_core::protocol::StartActorSessionRequest;
use cohesion_db::{
    GetActorCommand, GetActorSessionCommand, GuardedDatabaseContext,
    ListActorSessionMessagesCommand, ListActorSessionsCommand, ListActorsCommand,
};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, JsLifetime, Value, class::Trace};

use cohesion_macros::js_global;

use super::actor_options::ActorDispatchOptions;
use super::actor_session_message::ActorSessionMessage;
use super::agent_effect::follow_actor_session;
use super::global::Global;
use crate::actor_access::current_actors;
use crate::connector_access::throw_server_error;
use crate::conversion::{FromJsField, plain};
use crate::entities::actor::JsActor;
use crate::entities::actor_session::JsActorSession;
use crate::sandbox::ScriptDeadline;
use crate::traced_command::read;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct ActorsGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// The actors on the user's devices, such as Claude Code in a directory,
/// and the sessions Cohesion hands them: read them, start a session, reply in
/// one, stop one, mark one read for the user with `session.markRead()`, and
/// resolve one they are done with with `session.resolve()`.
///
/// An actor does the work on its own device, so a session takes minutes or
/// hours, not a script's few seconds: hand it work with `dispatch` (or tell
/// it more with `send`) and end your turn. In your conversation you then
/// follow the session: each time it stops on its own (it needs the user,
/// finishes, fails, or its device goes away), a fork of your conversation
/// wakes with where it stands and what the actor said, to carry on the
/// work: tell the user, answer it as they decided, or dispatch the next
/// session, which it follows in turn. A session someone stops wakes no one. No
/// cue is needed:
///
/// ```js
/// actors.dispatch(actorId, { prompt: "Fix the flaky socket test in session.rs." });
/// ```
///
/// The agent last to tell a session something follows it. A script that
/// throws follows nothing, though a session it started still runs. To hear of
/// a session you didn't hand work, such as one the user started, cue on
/// `actor:moved` matching its id.
///
/// Each call reaches the actor's device, which must be online, and throws
/// with why not.
#[js_global("actors")]
#[rquickjs::methods]
impl ActorsGlobal {
    /// Every actor the user's devices offer now, by device, product and
    /// scope.
    pub fn list<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsActor>> {
        let actors = read(&ctx, &self.database, ListActorsCommand)?;
        Ok(actors
            .into_iter()
            .map(|actor| JsActor::new(actor, self.database.clone()))
            .collect())
    }

    /// The actor with this id; throws a `ReferenceError` if no device
    /// offers it now.
    pub fn actor<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsActor> {
        let actor = read(&ctx, &self.database, GetActorCommand { id })?;
        Ok(JsActor::new(actor, self.database.clone()))
    }

    /// Sessions not yet resolved, latest status change first: actor
    /// `actorId`'s, or every actor's.
    pub fn sessions<'js>(
        &self,
        ctx: Ctx<'js>,
        actor_id: Opt<i64>,
    ) -> rquickjs::Result<Vec<JsActorSession>> {
        let sessions = read(
            &ctx,
            &self.database,
            ListActorSessionsCommand {
                actor_id: actor_id.0,
            },
        )?;
        Ok(sessions
            .into_iter()
            .map(|session| JsActorSession::new(session, self.database.clone()))
            .collect())
    }

    /// The session with this id; throws a `ReferenceError` if there is none.
    pub fn session<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsActorSession> {
        let session = read(&ctx, &self.database, GetActorSessionCommand { id })?;
        Ok(JsActorSession::new(session, self.database.clone()))
    }

    /// What was said in session `sessionId`, oldest first: the prompt, then what
    /// the actor and the user said.
    #[ts(returns = "ActorSessionMessage[]")]
    pub fn messages<'js>(&self, ctx: Ctx<'js>, session_id: i64) -> rquickjs::Result<Value<'js>> {
        let messages = read(
            &ctx,
            &self.database,
            ListActorSessionMessagesCommand {
                session_id,
                ids: None,
            },
        )?;
        let messages: Vec<ActorSessionMessage> = messages
            .into_iter()
            .map(ActorSessionMessage::from)
            .collect();
        plain(&ctx, &messages)
    }

    /// Hands actor `actorId` a new session and returns it, working.
    ///
    /// The user follows it among their actor sessions and can reply in it
    /// themselves; in your conversation you follow it too, so its next stop
    /// wakes a fork of the conversation. Throws if the actor's device is
    /// offline or refuses it, leaving no session.
    pub fn dispatch<'js>(
        &self,
        ctx: Ctx<'js>,
        actor_id: i64,
        options: ActorDispatchOptions,
    ) -> rquickjs::Result<JsActorSession> {
        let request = StartActorSessionRequest {
            actor_id,
            prompt: options.prompt,
            title: options.title,
            mode: options.mode,
            model: options.model,
            effort: options.effort,
            attachments: None,
        };
        let session = current_actors(&ctx)?
            .start(request, ScriptDeadline::remaining_until(&ctx)?)
            .map_err(|e| throw_server_error(&ctx, e))?;
        follow_actor_session(&ctx, session.id);
        Ok(JsActorSession::new(session, self.database.clone()))
    }

    /// Says `content` in session `sessionId` and returns the message: text, which
    /// is more to do and starts a session that is idle or failed again, or an
    /// `ActorResponse` to one of the actor's requests. Text starts a
    /// resolved session again too. Answer a request only as the user decided:
    /// in a conversation, once they have said what to answer.
    #[ts(returns = "ActorSessionMessage")]
    pub fn send<'js>(
        &self,
        ctx: Ctx<'js>,
        session_id: i64,
        #[ts(type = "string | ActorResponse")] content: Value<'js>,
    ) -> rquickjs::Result<Value<'js>> {
        let content = match content.as_string() {
            Some(text) => ActorUserContent::Text {
                text: text.to_string()?,
            },
            None => {
                let content = serde_json::Value::from_js_field(&ctx, content)?;
                serde_json::from_value(content)
                    .map_err(|e| Exception::throw_type(&ctx, &format!("Not a response: {e}")))?
            }
        };
        let message = current_actors(&ctx)?
            .send(session_id, content, ScriptDeadline::remaining_until(&ctx)?)
            .map_err(|e| throw_server_error(&ctx, e))?;
        follow_actor_session(&ctx, session_id);
        plain(&ctx, &ActorSessionMessage::from(message))
    }

    /// Stops what session `sessionId` is doing and returns it, idle; sending it
    /// more starts it again.
    pub fn stop<'js>(&self, ctx: Ctx<'js>, session_id: i64) -> rquickjs::Result<JsActorSession> {
        let session = current_actors(&ctx)?
            .stop(session_id, ScriptDeadline::remaining_until(&ctx)?)
            .map_err(|e| throw_server_error(&ctx, e))?;
        Ok(JsActorSession::new(session, self.database.clone()))
    }
}

impl Global for ActorsGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set("actors", ActorsGlobal { database })?;
        Ok(())
    }
}
