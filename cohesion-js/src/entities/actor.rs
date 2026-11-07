//! `Actor` in scripts.

use cohesion_core::model::Actor;
use cohesion_macros::js_entity;
use rquickjs::{Ctx, Value};

use crate::actor_access::current_actors;
use crate::conversion::plain;

/// An actor: an agent on one of the user's devices that Cohesion hands
/// sessions to, such as Claude Code in one directory, from `actors.list()` or
/// `actors.actor(id)`.
///
/// `title` reads as "Claude Code on Ada's MacBook Pro", `scope` says
/// where it works (a directory, say), and `modes` and `models` list how it
/// can run a session and on what, the first of each its default; either is
/// empty when the actor has one way only or picks for itself. A model's
/// `efforts` are least first. Hand it work with `actors.dispatch(id, ...)`.
#[js_entity(Actor)]
impl JsActor {
    /// The limits of the plan the actor runs on, such as a five-hour and a
    /// weekly one, as its device last reported them: `used` is from 0 to 1,
    /// `resets_at` in Unix seconds or `null` when unknown. Actors drawing
    /// on one account share its limits, so this is the latest any of them
    /// reported. Empty when its device is offline or reported none. Reads
    /// what the daemon holds, not the device.
    #[qjs(rename = "usageLimits")]
    #[ts(returns = "PlanUsage[]")]
    pub fn usage_limits<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let usage = current_actors(&ctx)?.usage(self.inner.client_id, &self.inner.identifier);
        plain(&ctx, &usage.plan)
    }
}
