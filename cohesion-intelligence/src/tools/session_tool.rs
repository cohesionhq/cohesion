use std::pin::Pin;

use serde::Deserialize;

use crate::activity::Activity;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::transcript_item::ToolCallStatus;

/// The boxed future returned by [`SessionTool::call`], boxed so the trait
/// stays usable as `dyn SessionTool`.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = ToolCallStatus> + Send + 'a>>;

/// A tool a [`Session`](crate::Session) offers the model. The session
/// advertises each tool's [`definition`](SessionTool::definition) and routes
/// a call to the tool whose definition carries the called name.
pub trait SessionTool: Send + Sync {
    fn definition(&self) -> Tool;
    /// Runs one call. `arguments` is the model's JSON, unvalidated.
    fn call<'a>(&'a self, arguments: &'a str) -> ToolFuture<'a>;
    /// What a call with `arguments` does, shown while it runs; `None` keeps
    /// showing what the session showed before, for a call that is quick.
    fn activity(&self, _arguments: &str) -> Option<Activity> {
        None
    }
    /// The JavaScript globals the scripts this tool runs reach, which a
    /// model may call as tools of their own; the error a call to an
    /// unknown tool answers names them.
    fn globals(&self) -> Vec<&'static str> {
        Vec::new()
    }
    /// Whether a call with `arguments` that succeeds ends the answer: the
    /// session asks the model for nothing more, since the call delivered
    /// all the turn had left to do. A tool that offers [`done_field`] says
    /// so with [`asks_done`].
    fn ends_turn(&self, _arguments: &str) -> bool {
        false
    }
}

/// The `done` field of a tool that delivers what a turn is for (a
/// message, a reply), with which the model ends its turn on the call
/// rather than in one more response nothing reads.
pub fn done_field() -> Field {
    Field {
        name: "done",
        description: "Optional: true when this call's work is the last thing your turn does: \
            it ends once this succeeds, with no final response. Leave it out when more work \
            follows, and never make a call only to set it: a turn can end with a normal final \
            response.",
        kind: FieldKind::Boolean,
        required: false,
    }
}

/// Whether `arguments` set [`done_field`] true.
pub fn asks_done(arguments: &str) -> bool {
    #[derive(Deserialize)]
    struct Done {
        #[serde(default)]
        done: bool,
    }
    serde_json::from_str::<Done>(arguments).is_ok_and(|call| call.done)
}
