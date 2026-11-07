//! `ActorSession` in scripts.

use cohesion_core::model::ActorSession;
use cohesion_db::MarkActorSessionReadCommand;
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use crate::actor_access::current_actors;
use crate::connector_access::throw_server_error;
use crate::sandbox::ScriptDeadline;
use crate::traced_command::ensure_writable;

/// A session an actor works on, such as one Claude Code session, from
/// `actors.dispatch()`, `actors.session(id)` or `actors.sessions()`, as it stood
/// when read.
///
/// `status` is where it stands, as its actor last reported: `"working"`,
/// `"needsAttention"` (waiting on the user, such as for an answer),
/// `"idle"` (done, until someone says more) or `"failed"`; or
/// `"resolved"`, once the user is done with it (`resolve()`). A session stays
/// in `actors.sessions()` until resolved, and none ends for good:
/// `actors.send` starts one that is idle, failed or resolved again. `mode`,
/// `model` and `effort` are `null` for the actor's defaults; `startedAt`
/// and `lastActivityAt` are in Unix seconds. `unread` is whether the user
/// has yet to see where the session stands since its actor last finished a
/// stretch of work; `markRead()` dismisses it for them.
///
/// Link one in a message as `[Fix flaky test](cohesion://actor-session/143)`,
/// its title and `id`; the user opens it with a tap.
#[js_entity(ActorSession)]
impl JsActorSession {
    /// Id of the agent following this session, the last to tell it something,
    /// which hears each time it stops on its own; `null` when none does.
    #[qjs(get, rename = "followerAgentId")]
    pub fn follower_agent_id(&self) -> Option<i64> {
        self.inner.follower.map(|follower| follower.agent_id)
    }

    /// Marks the session read, as the user opening it does: `unread` clears
    /// and `readAt` is now, until its actor next finishes a stretch of
    /// work. Not available in read-only contexts.
    #[qjs(rename = "markRead")]
    pub fn mark_read<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            MarkActorSessionReadCommand {
                session_id: self.inner.id,
            },
        )?;
        Ok(())
    }

    /// Resolves the session for the user, who is done with it: one going on
    /// is stopped first. It leaves `actors.sessions()` and the user's lists,
    /// and is deleted once it has sat resolved for the retention the user
    /// set; sending it more starts it again. Resolve only a session the user
    /// is done with, as they said. Not available in read-only contexts.
    pub fn resolve<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        ensure_writable(&ctx, &self.db)?;
        self.inner = current_actors(&ctx)?
            .resolve(self.inner.id, ScriptDeadline::remaining_until(&ctx)?)
            .map_err(|e| throw_server_error(&ctx, e))?;
        Ok(())
    }
}
