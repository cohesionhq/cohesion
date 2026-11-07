use clap::{Parser, Subcommand};

use crate::{cli::CliCommand, setup::SetupArgs};

#[derive(Parser)]
#[command(version = cohesion_core::VERSION, about)]
pub struct CohesionArgs {
    /// Directory holding this client's identity and settings, instead of the default location.
    #[arg(long = "config", short = 'c', global = true)]
    pub config_directory: Option<String>,

    /// Workspace id or title to act in, overriding the one selected with `switch`.
    #[arg(long, short = 'w', global = true)]
    pub workspace: Option<String>,

    #[command(subcommand)]
    pub command: CohesionCommand,
}

#[derive(Subcommand)]
pub enum CohesionCommand {
    /// Create this client's identity and register it with cohesiond using its setup code.
    Setup(SetupArgs),
    /// Send one raw protocol command to cohesiond and print its JSON reply.
    Api {
        /// The command as JSON, e.g. '{"type":"createWorkspace","args":{"title":"Home"}}'.
        /// Read from stdin when omitted or `-`.
        #[arg(value_name = "REQUEST")]
        request: Option<String>,
    },
    #[command(flatten)]
    Cli(CliCommand),
}

#[cfg(test)]
mod tests;
