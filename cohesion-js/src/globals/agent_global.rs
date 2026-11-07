use cohesion_db::GuardedDatabaseContext;
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};

use cohesion_macros::js_global;

use super::agent_effect::{ForegroundEffect, push_effect};
use super::agent_options::AgentCueOptions;
use super::global::Global;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct AgentGlobal {}
/// Exists only in an agent's own sandbox, where the user is talking to it: how
/// the agent sets and removes its cues.
///
/// Calls take effect once the script finishes, and only if it does not throw.
#[js_global("agent")]
#[rquickjs::methods]
impl AgentGlobal {
    /// Sets a cue that wakes you, replacing the cue you already have with the
    /// same `name`; cues with other names stay.
    ///
    /// Each time `hook` fires, `code` is called with the hook's subject; a
    /// truthy return wakes a background run of you, without your conversation, that reads
    /// `instructions` and what `code` returned (the whole subject for `true`),
    /// on a model as capable as `effort` asks. The user didn't start it, so it
    /// only reads, and asks the user before changing anything. An
    /// actor session you dispatch needs no cue: you follow it (see `actors`).
    pub fn cue<'js>(&self, ctx: Ctx<'js>, options: AgentCueOptions) -> rquickjs::Result<()> {
        let name = options.name.trim();
        if name.is_empty() {
            return Err(Exception::throw_type(&ctx, "A cue needs a name."));
        }
        push_effect(
            &ctx,
            ForegroundEffect::Cue {
                name: name.to_string(),
                hook: options.hook,
                code: options.code,
                instructions: options.instructions,
                effort: options.effort,
            },
        )
    }

    /// Removes the cue named `name`, so its hook no longer wakes you.
    #[qjs(rename = "removeCue")]
    pub fn remove_cue<'js>(&self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        push_effect(&ctx, ForegroundEffect::RemoveCue(name.trim().to_string()))
    }
}
impl Global for AgentGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        _database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set("agent", AgentGlobal {})?;
        Ok(())
    }
}
