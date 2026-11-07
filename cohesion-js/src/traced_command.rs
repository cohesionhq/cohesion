//! Database commands sent from scripts through their sandbox's guard: reads
//! always, and writes under the running hook's trace and only to the areas
//! the sandbox may write, with failures thrown as JS exceptions.

use rquickjs::{Ctx, Exception, JsLifetime};

use cohesion_core::protocol::ServerError;
use cohesion_db::{
    Area, CommandError, DatabaseCommand, DatabaseEventTrace, GuardedDatabaseContext, Read, Write,
    WriteArea,
};

/// What a write to `area` from a sandbox that may not write it throws.
fn refusal(area: WriteArea) -> &'static str {
    match area {
        WriteArea::UserData => {
            "This script has read-only access to Cohesion; the `cohesion:write` permission is required to change data."
        }
        WriteArea::IntelligenceSettings => {
            "This script can't change intelligence settings; the `intelligence` permission is required."
        }
        WriteArea::AgentSettings => "This script can't change an agent's memories or cues.",
        WriteArea::Notifications => {
            "This script can't send notifications; the `notifications` permission is required."
        }
    }
}

/// The trace of the hook whose job is running, stored as context userdata
/// by [`Sandbox::traced`](crate::Sandbox::traced) for the job's duration.
#[derive(JsLifetime)]
pub(crate) struct CurrentTrace(pub DatabaseEventTrace);

/// Runs a read command from a script, turning any failure into a JS
/// exception carrying the client-safe message.
pub fn read<'js, C: DatabaseCommand<Access = Read>>(
    ctx: &Ctx<'js>,
    db: &GuardedDatabaseContext,
    cmd: C,
) -> rquickjs::Result<C::Response> {
    db.send_read_blocking(cmd)
        .map_err(|error| throw_command_error(ctx, error))
}

/// Runs a write command from a script under the running hook's trace. Throws
/// if the sandbox may not write the command's area, and turns any failure
/// into a JS exception carrying the client-safe message.
pub fn write<'js, A: Area, C: DatabaseCommand<Access = Write<A>>>(
    ctx: &Ctx<'js>,
    db: &GuardedDatabaseContext,
    cmd: C,
) -> rquickjs::Result<C::Response> {
    let trace = ctx
        .userdata::<CurrentTrace>()
        .map(|t| t.0.clone())
        .unwrap_or_default();
    match db.send_write_blocking_traced(cmd, trace) {
        Some(result) => result.map_err(|error| throw_command_error(ctx, error)),
        None => Err(Exception::throw_message(ctx, refusal(A::AREA))),
    }
}

/// Throws what a write of the user's data from a read-only sandbox throws,
/// before a method does work (such as fetching a page) only a write would
/// use.
pub(crate) fn ensure_writable(ctx: &Ctx<'_>, db: &GuardedDatabaseContext) -> rquickjs::Result<()> {
    if db.may_write(WriteArea::UserData) {
        Ok(())
    } else {
        Err(Exception::throw_message(ctx, refusal(WriteArea::UserData)))
    }
}

/// The exception a failed command throws: its type follows the error's kind,
/// its message is the one clients see.
pub(crate) fn throw_command_error(ctx: &Ctx<'_>, error: CommandError) -> rquickjs::Error {
    let throw = match error {
        CommandError::Database(_) => Exception::throw_internal,
        CommandError::NotFound(_) => Exception::throw_reference,
        CommandError::Validation(_) => Exception::throw_syntax,
        CommandError::Unauthorized | CommandError::InvalidReference => Exception::throw_message,
    };
    let ServerError { message, field, .. } = error.into();
    match field {
        Some(field) => throw(ctx, &format!("{message} (field `{field}`)")),
        None => throw(ctx, &message),
    }
}
