//! Extension hooks: [`HookManager`] queues a [`HookEvent`] (launch, the
//! start or end of the day, the start of an hour, an extension update, or a
//! database event such as a task being created or an agent's turn
//! starting), and the [`HookReceiver`](hook_receiver::HookReceiver) runs it in every extension with the
//! event's hook enabled.

mod day_clock;
mod hook_events;
mod hook_manager;
mod hook_receiver;

pub use hook_events::*;
pub use hook_manager::HookManager;
