use std::pin::Pin;

use crate::intelligence_error::IntelligenceError;
use crate::transcript::Transcript;
use crate::turn::Turn;

/// The boxed future returned by [`IntelligenceBackend::send`], boxed so the trait
/// stays usable as `dyn IntelligenceBackend`.
pub type BackendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Turn, IntelligenceError>> + Send + 'a>>;

/// A connection to an intelligence endpoint, scoped to one transcript.
pub trait IntelligenceBackend: Send + Sync {
    /// Sends `state` and returns the model's turn, its tool calls among its
    /// outputs.
    fn send<'a>(&'a mut self, state: &'a Transcript) -> BackendFuture<'a>;
}
