use cohesion_macros::{JsOptions, js_global};
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};

use super::global::Global;
use super::timer_registration::{TimerRegistration, TimerSchedule};
use super::timer_unit::TimerUnit;

const MIN_TIMER_SECONDS: u64 = 10;
const MAX_TIMER_SECONDS: u64 = 86_400;

#[derive(JsOptions)]
pub struct TimerOptions {
    pub every: i64,
    pub unit: TimerUnit,
    pub call: String,
}

#[derive(JsLifetime, Trace)]
#[rquickjs::class]
pub struct TimerGlobal {
    #[qjs(skip_trace)]
    schedule: TimerSchedule,
}
#[rquickjs::methods]
#[js_global("timer")]
impl TimerGlobal {
    /// Schedule to call a function on a recurring schedule. If the scheduled work takes
    /// longer than the schedule, the next call will be skipped. Hooks should be configured
    /// to run in the `onLaunch` function, using the `launch` hook. There may be
    /// a use case to run it in another hook or function as well, so the `launch` hook is NOT
    /// required and not automatically given.
    fn every<'js>(&self, ctx: Ctx<'js>, options: TimerOptions) -> rquickjs::Result<()> {
        let Ok(constant @ 1..) = u64::try_from(options.every) else {
            return Err(Exception::throw_range(&ctx, "`every` must be at least 1"));
        };
        let reg = TimerRegistration {
            unit: options.unit,
            constant,
            function_name: options.call,
        };
        if reg.seconds() < MIN_TIMER_SECONDS {
            return Err(Exception::throw_range(
                &ctx,
                &format!("A timer cannot fire more than every {MIN_TIMER_SECONDS} seconds."),
            ));
        } else if reg.seconds() > MAX_TIMER_SECONDS {
            return Err(Exception::throw_range(
                &ctx,
                &format!("A timer cannot fire less than every {MAX_TIMER_SECONDS} seconds."),
            ));
        }
        _ = self.schedule.0.send(reg);
        Ok(())
    }
}

impl Global for TimerGlobal {
    fn install<'a>(
        context: &rquickjs::prelude::Ctx<'a>,
        _database: cohesion_db::GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        let schedule = context
            .userdata::<TimerSchedule>()
            .ok_or_else(|| {
                rquickjs::Exception::throw_message(
                    context,
                    "The timer global isn't available in this context.",
                )
            })?
            .0
            .clone();
        context.globals().set(
            "timer",
            TimerGlobal {
                schedule: TimerSchedule(schedule),
            },
        )?;
        Ok(())
    }
}
