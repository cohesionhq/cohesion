//! Persistent agents. The [`AgentCoordinator`] keeps one `AgentHost` per
//! agent, which serialises the agent's foreground: its conversation with the
//! user, rebuilt from a compact transcript (`agent_transcript`) whenever its
//! thread can't carry on. Everything else the agent does is a fork of that
//! conversation (`agent_fork`): one turn on a copy of the conversation's own
//! thread (never another fork's), which hears what other forks posted as
//! news. A fork's rights follow from who started its chain, the user or an
//! event (`Initiative`, carried on the trace), and `cohesion_js::AgentRole`
//! alone says what each may reach: a turn the user started has everything,
//! one an event started only reads and asks the user with `ask_choice`.
//! Hook events reach the coordinator through `fire`, which evaluates each
//! matching cue and wakes a fork for every one that fires, started by an
//! event (a cue whose function fails says so in the feed, every time:
//! `cue_failures`). A conversation asks another agent with
//! `ask_agent`, through the `agent_messenger`, which wakes a
//! fork of the asked agent's conversation (`agent_fork`) to answer: with
//! `reply_to_agent` (`reply_tool`) to a turn that waits for it, or, for a
//! one-way question, to no one. An actor session
//! the conversation hands work is followed: `fire` wakes a fork of it as
//! the actor_session needs the user or finishes. An actor session someone other than the agent
//! assigns it (the user, a script, another agent's turn) wakes a fork to
//! work on it, through `fire` too; a job's assignment wakes none. Only the
//! conversation and its forks ask or follow, and only a chain the user
//! started assigns, so every fork traces back to the user; each fork is one
//! hop further from the user's message, counted and logged, never
//! limited.

mod actor_exchange;
mod agenda;
mod agent_activity;
mod agent_code_call;
mod agent_coordinator;
mod agent_error;
mod agent_fork;
mod agent_host;
mod agent_messenger;
mod agent_tool;
mod agent_transcript;
mod applied_effects;
mod ask_tool;
mod conversation_mark;
mod conversation_session;
mod cue_failures;
mod documentation_tool;
mod foreground_code_tool;
mod foreground_turn;
mod image_tray;
mod installed_services;
mod main_thread;
mod memory_tools;
mod message_tool;
mod react_tool;
mod read_message_tool;
mod relevant_set;
mod reply_tool;
mod wake_description;

pub use agent_coordinator::AgentCoordinator;
pub use agent_error::AgentError;

#[cfg(test)]
mod tests;
