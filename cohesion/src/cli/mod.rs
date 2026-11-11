//! Commands that print for people rather than for jq. `cli_command` declares the nouns and
//! dispatches to their modules, which declare their subcommands next to the handlers they
//! dispatch to; `table`, `terminal_io` and `calendar_day` hold what they share.

mod agent;
mod article;
mod calendar_day;
mod cli_command;
mod extension;
mod intelligence;
mod journal;
mod mcp;
mod noun;
mod open;
mod pair;
mod script;
mod search;
mod status;
mod table;
mod tag;
mod task;
mod team;
mod terminal_io;
mod transfer;
mod workspace;

pub use cli_command::{CliCommand, run};
