use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{ActorSession, ActorSessionFollower, Initiative};
use cohesion_core::protocol::UnfollowActorSessionRequest;

use super::actor::{GetActorSessionCommand, recounted_agents, session_keys};
use crate::changed::{Changed, Invalidates};
use crate::{CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Write};

/// Has agent `agent_id` follow actor session `session_id`, in place of whoever
/// followed it before, as the agent last to tell it something: each time
/// the session stops on its own, a fork of its conversation wakes, with
/// `initiative`, that of the chain that told it.
pub struct FollowActorSessionCommand {
    pub session_id: i64,
    pub agent_id: i64,
    pub initiative: Initiative,
}
impl DatabaseCommand for FollowActorSessionCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let get = || GetActorSessionCommand {
            id: self.session_id,
        };
        let before = get().handle(&mut ctx.as_read())?;
        ctx.db
            .update::<ActorSession>()
            .set_group(Some(ActorSessionFollower {
                agent_id: self.agent_id,
                initiative: self.initiative,
            }))
            .filter_eq(ActorSession::ID, self.session_id)
            .execute()?;
        let after = get().handle(&mut ctx.as_read())?;
        Ok(().invalidating(
            [
                Invalidation::ActorSessions,
                Invalidation::ActorSession(self.session_id),
            ]
            .into_iter()
            .chain(recounted_agents(&before, &after)),
        ))
    }
}

/// Has agent `agent_id` stop following actor session `session_id`, so no fork
/// of it wakes when the session stops on its own, until it tells the session
/// something again. A session another agent follows, or none does, is left
/// as it is: the agent has nothing to stop.
#[derive(FromRequest)]
#[request(UnfollowActorSessionRequest)]
pub struct UnfollowActorSessionCommand {
    pub session_id: i64,
    pub agent_id: i64,
}
impl DatabaseCommand for UnfollowActorSessionCommand {
    type Response = ActorSession;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let get = || GetActorSessionCommand {
            id: self.session_id,
        };
        let before = get().handle(&mut ctx.as_read())?;
        let changed = ctx
            .db
            .update::<ActorSession>()
            .set_group(None::<ActorSessionFollower>)
            .filter_eq(ActorSession::ID, self.session_id)
            .filter("follower_agent_id = ?")
            .bind(self.agent_id)
            .execute()?;
        let after = get().handle(&mut ctx.as_read())?;
        if changed == 0 {
            return Ok(after.invalidating_nothing());
        }
        let keys = session_keys(&mut ctx.as_read(), &after)?
            .into_iter()
            .chain(recounted_agents(&before, &after))
            .collect::<Vec<_>>();
        Ok(after.invalidating(keys))
    }
}

#[cfg(test)]
mod tests;
