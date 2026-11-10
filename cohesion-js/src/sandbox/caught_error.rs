//! Turning what QuickJS reports into a [`ScriptError`].

use rquickjs::convert::Coerced;
use rquickjs::{CaughtError, Ctx, Exception, FromJs};

use crate::script_error::ScriptError;

/// What QuickJS throws, uncatchably, when the interrupt handler stops a
/// script at its deadline.
pub(crate) const INTERRUPTED: (&str, &str) = ("InternalError", "interrupted");

/// A script failure: whatever the script threw becomes its message, and an
/// error object's stack comes along for the logs.
pub(crate) fn caught<'js>(ctx: &Ctx<'js>, error: CaughtError<'js>) -> ScriptError {
    let thrown = match error {
        CaughtError::Exception(exception) => return thrown_error(&exception),
        CaughtError::Value(value) => value,
        CaughtError::Error(error) => return internal(error),
    };
    match Coerced::<String>::from_js(ctx, thrown) {
        Ok(Coerced(message)) => ScriptError::Exception {
            message,
            stack: None,
        },
        Err(error) => internal(error),
    }
}

/// An `Error` object: its message, prefixed with its name (`TypeError: …`),
/// and its stack.
fn thrown_error(exception: &Exception<'_>) -> ScriptError {
    let name = exception
        .get::<_, Option<Coerced<String>>>("name")
        .ok()
        .flatten()
        .map(|Coerced(name)| name);
    let message = exception.message().unwrap_or_default();
    let stack = exception
        .stack()
        .map(|stack| stack.trim_end().to_string())
        .filter(|stack| !stack.is_empty());
    let message = match name.as_deref() {
        Some(name) if (name, message.as_str()) == INTERRUPTED => {
            "script ran past its time limit and was stopped".to_string()
        }
        Some(name) if !name.is_empty() => format!("{name}: {message}"),
        _ => message,
    };
    ScriptError::Exception { message, stack }
}

pub(super) fn internal(error: rquickjs::Error) -> ScriptError {
    ScriptError::InternalError(error.to_string())
}
