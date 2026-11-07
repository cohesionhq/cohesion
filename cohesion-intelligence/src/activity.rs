//! What a session is doing while it works, as its caller shows it: a word
//! while it waits on the model, or what the tool it runs does.

use std::pin::Pin;

/// What a session is doing at one moment of its work.
pub enum Activity {
    /// Waiting on the model's `n`th call of the work; the word rotates
    /// each call.
    Thinking(usize),
    /// Loading a module's declarations.
    Learning,
    Remembering,
    Forgetting,
    /// Looking through memories.
    Recalling,
    /// Running a script, by the title the model gave it.
    Running(String),
    /// Waiting for another agent's answer.
    Asking,
}

/// The words `Thinking` rotates through, one per model call.
const THINKING: [&str; 12] = [
    "Thinking",
    "Working",
    "Reasoning",
    "Pondering",
    "Noodling",
    "Mulling",
    "Musing",
    "Puzzling",
    "Tinkering",
    "Weighing",
    "Brewing",
    "Wondering",
];

impl Activity {
    /// How clients show it.
    pub(crate) fn label(self) -> String {
        match self {
            Self::Thinking(call) => THINKING[call % THINKING.len()].to_string(),
            Self::Learning => "Learning".to_string(),
            Self::Remembering => "Remembering".to_string(),
            Self::Forgetting => "Forgetting".to_string(),
            Self::Recalling => "Recalling".to_string(),
            Self::Running(title) => title,
            Self::Asking => "Asking".to_string(),
        }
    }
}

/// The boxed future returned by [`ShowActivity::show`], boxed so the trait
/// stays usable as `dyn ShowActivity`.
pub type ShowFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Where a [`Session`](crate::session::Session) reports what it is doing.
/// A failure to show it is the implementor's to log; the work goes on.
pub trait ShowActivity: Send + Sync {
    fn show(&self, activity: Activity) -> ShowFuture<'_>;
}
