//! The options object `actors.dispatch()` takes.

use cohesion_macros::JsOptions;

/// Options for `actors.dispatch()`.
#[derive(JsOptions)]
pub struct ActorDispatchOptions {
    /// What the actor is to do, as you would tell it yourself: it can't read
    /// your conversation, so put everything it needs here.
    pub prompt: String,
    /// What the user sees the session called, in a few words, such as
    /// `"Fix the flaky socket test"`; left out, the prompt's first line.
    ///
    /// The actor's device may give the session a title of its own later.
    pub title: Option<String>,
    /// The `identifier` of one of the actor's `modes`; left out, its
    /// default.
    ///
    /// Use the default unless the user asked for another mode.
    pub mode: Option<String>,
    /// The `identifier` of one of the actor's `models`; left out, its
    /// default.
    pub model: Option<String>,
    /// The `identifier` of one of the model's `efforts`; left out, the
    /// model's default.
    pub effort: Option<String>,
}
