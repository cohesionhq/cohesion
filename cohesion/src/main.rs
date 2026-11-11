//! `cohesion`, the command-line client for cohesiond: `setup` registers this client,
//! `api` sends raw protocol commands, the `cli` commands print for people, `open`
//! serves the web UI on loopback through `web_proxy`, which relays the browser's calls
//! over this client's connection, and `mcp` serves MCP on stdio.

mod api;
mod args;
mod cli;
mod cli_context;
mod cli_error;
mod cli_macros;
mod client_config;
mod client_identity;
mod client_storage;
mod client_tls;
mod connection;
mod connection_error;
mod local_connection;
mod name_lookup;
mod remote_connection;
mod setup;
mod web_proxy;

use std::process::ExitCode;

use clap::Parser;

use crate::{
    args::{CohesionArgs, CohesionCommand},
    cli_context::Context,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = CohesionArgs::parse();
    let config_directory = args.config_directory.as_deref();
    let load = || Context::load(config_directory, args.workspace);
    let result = match args.command {
        // Runs before there's a configuration to load.
        CohesionCommand::Setup(setup) => setup::run(config_directory, setup).await,
        CohesionCommand::Api { request } => async { api::run(&load()?, request).await }.await,
        CohesionCommand::Cli(command) => async { cli::run(&mut load()?, command).await }.await,
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
