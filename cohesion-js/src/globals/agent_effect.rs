//! What an agent's script asks of its agent, collected while the script runs
//! and acted on by the caller once it has finished.

use std::cell::RefCell;

use cohesion_core::model::{HookKind, IntelligenceEffort};
use cohesion_core::protocol::ImageUpload;
use rquickjs::{Ctx, Exception, JsLifetime};

/// One request a foreground agent's script recorded through the `agent`
/// global, in the order the script made it.
#[derive(Clone, Debug, PartialEq)]
pub enum ForegroundEffect {
    /// `agent.cue()`: wake a fork of the agent's conversation, on a model of
    /// `effort`'s tier, when
    /// `code`, called with `hook`'s subject, returns something truthy;
    /// replaces the cue named `name`.
    Cue {
        name: String,
        hook: HookKind,
        code: String,
        instructions: String,
        effort: IntelligenceEffort,
    },
    /// `agent.removeCue()`, with the cue's name.
    RemoveCue(String),
    /// `actors.dispatch()` or `actors.send()`: follow the actor session, by
    /// id, the script handed work, so each time it stops on its own a fork of
    /// the agent's conversation wakes.
    Follow(i64),
}

/// The effects a job's script recorded, in call order, and the images it
/// attached. A caller must apply them or
/// explicitly discard them with [`Effects::into_parts`]; a run's effects
/// that are silently dropped are a script's requests that never happened.
#[derive(Debug)]
#[must_use = "effects the script recorded must be applied or explicitly discarded"]
pub struct Effects<E> {
    pub(crate) requests: Vec<E>,
    pub(crate) images: Vec<ImageUpload>,
}

impl<E> Effects<E> {
    /// The effects, in the order the script recorded them, and the images
    /// it attached through `images`, in the order attached, each already
    /// checked as a message's are.
    pub fn into_parts(self) -> (Vec<E>, Vec<ImageUpload>) {
        (self.requests, self.images)
    }
}

/// The effects of the agent job that is running, stored as context userdata
/// by [`Sandbox::collecting_effects`](crate::Sandbox::collecting_effects) for
/// the job's duration. `E` is the pool's effect type.
pub(crate) struct AgentEffects<E: 'static>(pub RefCell<Vec<E>>);

// By hand because the derive would also require `E: JsLifetime`; an `E` that
// is `'static` holds no JS values, so it is the same type whatever the
// lifetime.
unsafe impl<'js, E: 'static> JsLifetime<'js> for AgentEffects<E> {
    type Changed<'to> = AgentEffects<E>;
}

impl<E> Default for AgentEffects<E> {
    fn default() -> Self {
        Self(RefCell::new(Vec::new()))
    }
}

/// The images the agent job that is running attached through `images`,
/// stored as context userdata beside its [`AgentEffects`] for the job's
/// duration, whatever its pool.
#[derive(Default, JsLifetime)]
pub(crate) struct AgentImages(pub RefCell<Vec<ImageUpload>>);

/// Has the running job's agent follow actor session `actor_session_id`, when the job
/// is one of its conversation's; a script of
/// any other pool, or an extension's, follows nothing.
pub(crate) fn follow_actor_session(ctx: &Ctx<'_>, actor_session_id: i64) {
    if let Some(effects) = ctx.userdata::<AgentEffects<ForegroundEffect>>() {
        effects
            .0
            .borrow_mut()
            .push(ForegroundEffect::Follow(actor_session_id));
    }
}

/// Records `effect` for the running agent job; throws when no agent job of
/// that pool is collecting effects.
pub(super) fn push_effect<E: 'static>(ctx: &Ctx<'_>, effect: E) -> rquickjs::Result<()> {
    let effects = ctx.userdata::<AgentEffects<E>>().ok_or_else(|| {
        Exception::throw_internal(
            ctx,
            "Agent actions are only available while an agent's script is running.",
        )
    })?;
    effects.0.borrow_mut().push(effect);
    Ok(())
}
