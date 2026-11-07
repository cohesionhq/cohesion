use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use tokio::sync::oneshot;

use crate::sandbox::Sandbox;
use crate::script_error::ScriptError;

pub type ScriptJob = Box<dyn FnOnce(Result<&Sandbox, ScriptError>) + Send>;

/// Wraps `job` so its result reaches the returned receiver. A panic in
/// `job` arrives there as [`ScriptError::Panicked`] instead of unwinding
/// the worker.
pub(super) fn replying<T: Send + 'static>(
    job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
) -> (ScriptJob, oneshot::Receiver<Result<T, ScriptError>>) {
    let (reply, reply_rx) = oneshot::channel();
    let job: ScriptJob = Box::new(move |sandbox| {
        let result = catch_unwind(AssertUnwindSafe(|| sandbox.and_then(job)))
            .unwrap_or_else(|payload| Err(ScriptError::Panicked(panic_message(&*payload))));
        let _ = reply.send(result);
    });
    (job, reply_rx)
}

/// The message a panic carried, for the common string payloads.
pub(super) fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}
