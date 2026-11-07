//! The `Sleep` mount: no global, only the mark that lets the sandbox's
//! scripts pause with `cohesion.sleep`.

use std::thread;
use std::time::{Duration, Instant};

use rquickjs::{Ctx, Exception, JsLifetime};

use crate::globals::Global;
use crate::sandbox::ScriptDeadline;
use crate::script_stop::ScriptStop;

/// The longest one `cohesion.sleep` may pause for, in seconds.
const MAX_SLEEP_SECONDS: f64 = 30.0;

/// How often a sleeping script looks for a stop.
const STOP_POLL: Duration = Duration::from_millis(50);

/// Stored as context userdata in a sandbox that mounts `Sleep`;
/// `cohesion.sleep` pauses only when it is there.
#[derive(JsLifetime)]
pub struct SleepMount;

impl Global for SleepMount {
    fn install<'a>(
        context: &Ctx<'a>,
        _database: cohesion_db::GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .store_userdata(Self)
            .expect("no userdata is borrowed while a sandbox is built");
        Ok(())
    }
}

/// Stopped when the scope a persistent sandbox serves shuts down, stored as
/// its context userdata for the sandbox's life. It cuts a sleep short and
/// nothing else: a job already sent to the scope still runs.
#[derive(JsLifetime)]
pub(crate) struct ScopeClosing(pub ScriptStop);

/// What `cohesion.sleep` throws in a sandbox that may not.
const NO_SLEEP_MESSAGE: &str =
    "`cohesion.sleep` isn’t available here: cues can’t sleep. To act later, set a cue.";

/// Blocks the script's own thread for `seconds`, throwing a `RangeError` for
/// a value that isn't a number from 0 to [`MAX_SLEEP_SECONDS`] or that
/// would pass the script's deadline, and an `Error` once the job's stop or
/// its scope's closing cuts it short.
pub(crate) fn sleep(ctx: &Ctx<'_>, seconds: f64) -> rquickjs::Result<()> {
    if ctx.userdata::<SleepMount>().is_none() {
        return Err(Exception::throw_message(ctx, NO_SLEEP_MESSAGE));
    }
    if !(0.0..=MAX_SLEEP_SECONDS).contains(&seconds) {
        return Err(Exception::throw_range(
            ctx,
            &format!("`cohesion.sleep` takes a number of seconds from 0 to {MAX_SLEEP_SECONDS}."),
        ));
    }
    let until = Instant::now() + Duration::from_secs_f64(seconds);
    let deadline = ScriptDeadline::remaining_until(ctx)?;
    if until >= deadline {
        return Err(Exception::throw_range(
            ctx,
            &format!("Sleeping {seconds} seconds would pass this script's time limit."),
        ));
    }
    let stops: Vec<ScriptStop> = ScriptDeadline::stop(ctx)
        .into_iter()
        .chain(
            ctx.userdata::<ScopeClosing>()
                .map(|closing| closing.0.clone()),
        )
        .collect();
    loop {
        if stops.iter().any(ScriptStop::is_stopped) {
            return Err(Exception::throw_message(
                ctx,
                "This script was stopped while it slept.",
            ));
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        thread::sleep(left.min(STOP_POLL));
    }
}
