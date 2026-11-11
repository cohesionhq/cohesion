//! What reaches a running turn from outside it: the user's messages, which
//! steer it, news its owner adds (an agent's agenda changed by another of
//! its threads), and the user's stop.

use std::future::Future;
use std::pin::Pin;

use crate::transcript_item::Prompt;

/// The boxed future returned by [`TurnInput::stopped`], boxed so the trait
/// stays usable as `dyn TurnInput`.
pub type StoppedFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Where a [`Session`](crate::session::Session)'s turn hears from its user
/// while it works.
pub trait TurnInput: Send + Sync {
    /// The prompts sent since the turn last asked, oldest first, and any
    /// news; they join it before its next model call.
    fn take(&self) -> Vec<Prompt>;

    /// As [`take`](Self::take), except that when none are waiting the turn
    /// takes no more, and later prompts wait for a turn of their own. The
    /// turn asks once it would end.
    fn close(&self) -> Vec<Prompt>;

    /// Whether the user has stopped the turn.
    fn is_stopped(&self) -> bool;

    /// Resolves once the user stops the turn, at once if they have.
    fn stopped(&self) -> StoppedFuture<'_>;
}
