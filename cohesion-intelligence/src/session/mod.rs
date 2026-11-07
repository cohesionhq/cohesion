//! Conversations: a [`Session`] runs a thread's turns against a backend and
//! answers its tool calls with the [`SessionTool`](crate::SessionTool)s it
//! was given.

mod intelligence_session;
mod tier_model;

pub use intelligence_session::{LastCall, Session};
pub use tier_model::{SessionConstructionError, tier_model};
