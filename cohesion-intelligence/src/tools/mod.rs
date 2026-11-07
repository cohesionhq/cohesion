//! Tools a [`Session`](crate::Session) can offer the model: the
//! [`SessionTool`] trait. Agents implement their own code tools.

mod session_tool;

pub use session_tool::{SessionTool, ToolFuture, asks_done, done_field};
