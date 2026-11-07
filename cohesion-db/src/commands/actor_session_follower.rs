use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{ActorSession, ActorSessionFollower, Initiative};

use super::actor::{GetActorSessionCommand, recounted_agents};
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

#[cfg(test)]
mod tests;
