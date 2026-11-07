//! The clock behind `day:start`, `day:end` and `timer`, in the daemon's
//! local time, and the `task:start` and `task:due` that follow `day:start`.

use std::sync::Arc;
use std::time::Duration;

use cohesion_core::model::{DayFilter, HookKind, TagFilter};
use cohesion_core::{TaskFolderFilter, TaskSort, log::Module, log_error};
use cohesion_db::{DatabaseContext, DatabaseEventTrace, ListTasksCommand, TaskListing};
use jiff::Zoned;
use jiff::civil::{Date, Time, time};

use super::HookManager;
use super::hook_events::{
    DayEndHook, DayStartHook, HookEvent, TaskDueHook, TaskStartHook, TimerHook,
};

/// When `onDayStart` runs.
const DAY_START: Time = time(6, 0, 0, 0);
/// When `onDayEnd` runs.
const DAY_END: Time = time(23, 0, 0, 0);
/// How often the wall clock is read.
const TICK: Duration = Duration::from_secs(60);

impl HookManager {
    /// Reads the wall clock just after each minute starts and queues each
    /// clock hook whose time has passed since the last reading. The clock is
    /// read after each sleep rather than trusted, because a sleeping computer
    /// pauses tokio's timers. A time passed while the computer
    /// slept fires once on waking, `timer` only for the latest minute; one
    /// passed while the daemon was stopped, or shutting down, doesn't fire.
    /// Each `day:start` is followed by `task:start` and `task:due` for the
    /// tasks in `database` that start or are due that day.
    pub(crate) fn start_day_clock(&self, database: DatabaseContext) {
        let manager = self.clone();
        self.runtime.spawn(async move {
            let mut last = Zoned::now();
            loop {
                tokio::time::sleep(until_next_minute(&Zoned::now())).await;
                // The tick is work, so the hooks it queues are begun while
                // shutdown still sees it, and none is queued once closed.
                let tick = manager.work.run_new("day clock", async {
                    let now = Zoned::now();
                    for hook in passed(&last, &now) {
                        let day_start = hook.kind() == HookKind::DayStart;
                        manager.queue(hook, None, DatabaseEventTrace::default());
                        if day_start {
                            for hook in dated(&database, latest(&now, DAY_START).date()).await {
                                manager.queue(hook, None, DatabaseEventTrace::default());
                            }
                        }
                    }
                    last = now;
                });
                if tick.await.is_err() {
                    return;
                }
            }
        });
    }
}

/// The clock hooks whose most recent time is after `since` and no later
/// than `now`, oldest first.
fn passed(since: &Zoned, now: &Zoned) -> Vec<Arc<dyn HookEvent>> {
    let minute = minute_start(now);
    let timer = TimerHook {
        time: minute.timestamp().as_millisecond(),
        hour: minute.hour(),
        minute: minute.minute(),
        since: minute_start(since).timestamp().as_millisecond(),
    };
    let mut passed: Vec<(Zoned, Arc<dyn HookEvent>)> = [
        (
            latest(now, DAY_START),
            Arc::new(DayStartHook {}) as Arc<dyn HookEvent>,
        ),
        (latest(now, DAY_END), Arc::new(DayEndHook {})),
        (minute, Arc::new(timer)),
    ]
    .into_iter()
    .filter(|(at, _)| at > since)
    .collect();
    passed.sort_by(|(a, _), (b, _)| a.cmp(b));
    passed.into_iter().map(|(_, hook)| hook).collect()
}

/// `task:start` for each incomplete task that starts on `day`, then
/// `task:due` for each one due on it, oldest task first.
async fn dated(database: &DatabaseContext, day: Date) -> Vec<Arc<dyn HookEvent>> {
    let day = day.to_string();
    let on = |start_date, due_date| {
        ListTasksCommand(TaskListing {
            ids: None,
            workspace_id: None,
            folder: TaskFolderFilter::Any,
            priorities: None,
            sizes: None,
            start_date,
            due_date,
            assignee: None,
            tags: TagFilter::default(),
            statuses: None,
            metadata: None,
            has_metadata: None,
            sort: TaskSort::Created,
            descending: false,
        })
    };
    let starting = database.send(on(Some(DayFilter::Day(day.clone())), None));
    let due = database.send(on(None, Some(DayFilter::Day(day))));
    let (starting, due) = match (starting.await, due.await) {
        (Ok(starting), Ok(due)) => (starting, due),
        (Err(e), _) | (_, Err(e)) => {
            log_error!(Module::Extensions, "Failed to list today's tasks: {e:?}.");
            return Vec::new();
        }
    };
    let starting = starting.into_iter().map(|task| {
        Arc::new(TaskStartHook {
            task_id: task.id,
            workspace_id: task.workspace_id,
        }) as Arc<dyn HookEvent>
    });
    let due = due.into_iter().map(|task| {
        Arc::new(TaskDueHook {
            task_id: task.id,
            workspace_id: task.workspace_id,
        }) as Arc<dyn HookEvent>
    });
    starting.chain(due).collect()
}

/// The last moment at or before `now` the local clock read `time`. A time a
/// daylight saving change skips lands just after the gap.
fn latest(now: &Zoned, time: Time) -> Zoned {
    let today = now.with().time(time).build().expect("today is in range");
    if today <= *now {
        return today;
    }
    let yesterday = now.yesterday().expect("yesterday is in range");
    yesterday
        .with()
        .time(time)
        .build()
        .expect("yesterday is in range")
}

/// How long from `now` until the next minute starts, so the reading after
/// the sleep lands in that minute rather than drifting past one.
fn until_next_minute(now: &Zoned) -> Duration {
    TICK - Duration::new(now.second() as u64, now.subsec_nanosecond() as u32)
}

/// The start of the local minute `now` is in.
fn minute_start(now: &Zoned) -> Zoned {
    now.with()
        .second(0)
        .subsec_nanosecond(0)
        .build()
        .expect("the minute's start is in range")
}

#[cfg(test)]
mod tests;
