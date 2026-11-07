//! A script's request to call one of its functions on a schedule, and the
//! channel it travels down to the persistent worker that runs the timers.

use rquickjs::JsLifetime;
use tokio::sync::mpsc;

use super::timer_unit::TimerUnit;

pub struct TimerRegistration {
    pub(super) unit: TimerUnit,
    pub(super) constant: u64,
    pub function_name: String,
}
impl TimerRegistration {
    /// Returns the amount of seconds, or a week if it overflows.
    pub fn seconds(&self) -> u64 {
        (match self.unit {
            TimerUnit::Second => 1u64,
            TimerUnit::Minute => 60u64,
            TimerUnit::Hour => 60u64 * 60u64,
        })
        .saturating_mul(self.constant)
    }
}

#[derive(JsLifetime)]
pub struct TimerSchedule(pub mpsc::UnboundedSender<TimerRegistration>);
