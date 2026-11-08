//! cohesiond's command-line arguments.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use cohesionkit::log::Level;

#[derive(Parser)]
#[command(version = cohesionkit::VERSION, about)]
pub struct CohesiondArgs {
    /// Config directory holding the database, server key and config.json,
    /// which can set any other flag.
    #[arg(long = "config", short = 'c', global = true)]
    pub config_directory: Option<String>,

    #[command(subcommand)]
    pub command: Option<DataCommand>,

    /// IP address or DNS name clients should reach this server at, embedded in
    /// the setup code. Defaults to this machine's outbound address.
    #[arg(long)]
    pub host: Option<cohesionkit::Host>,

    /// HTTPS port to listen on. Defaults to 3000.
    #[arg(
        long,
        env = "COHESIOND_PORT",
        value_name = "PORT",
        value_parser = clap::value_parser!(u16).range(1..)
    )]
    pub port: Option<u16>,

    /// Port clients connect to, embedded in the setup code, when it differs
    /// from --port (a forwarded port, say). Defaults to --port.
    #[arg(
        long,
        env = "COHESIOND_PUBLIC_PORT",
        value_name = "PORT",
        value_parser = clap::value_parser!(u16).range(1..)
    )]
    pub public_port: Option<u16>,

    /// Unix socket for local clients. Defaults to the runtime directory on
    /// Linux, or the config directory elsewhere or when --config is given.
    #[arg(long)]
    pub socket: Option<std::path::PathBuf>,

    /// Least severe log level to print: debug, info, warn or error. SQL
    /// statements are logged at debug, so they only print with `debug`.
    /// Defaults to info.
    #[arg(
        long,
        env = "COHESIOND_LOG",
        value_name = "LEVEL",
        value_parser = clap::builder::ValueParser::new(str::parse::<Level>)
    )]
    pub log_level: Option<Level>,

    /// Exit when stdin closes. A parent that runs cohesiond as a child passes
    /// a pipe, so the daemon exits with it even if it is killed outright.
    #[arg(long)]
    pub exit_with_parent: bool,

    /// Address of a peer in front of this daemon that writes a PROXY v2 header
    /// with each client's real address; repeat for several. Connections from it
    /// must open with the header. Name it as this daemon sees it, and name
    /// nothing else: others can't claim an address and are served as themselves.
    /// Off by default.
    #[arg(long, value_name = "ADDRESS")]
    pub proxy_protocol: Vec<std::net::IpAddr>,
}

// Moving this server's data to another server. Without one, cohesiond serves.
// (A `///` here would replace cohesiond's own description in `--help`.)
#[derive(Subcommand)]
pub enum DataCommand {
    /// Copy this server's data to a new file, for `cohesiond import` on
    /// another server. Safe while cohesiond is running.
    ///
    /// The file holds everything in the database: workspaces, tasks,
    /// documents, agents and their chats, images, extensions and settings.
    /// It doesn't hold the server key or config.json.
    Export {
        /// The file to write. It mustn't exist yet.
        file: PathBuf,
    },
    /// Replace this server's data with a file from `cohesiond export`. Stop
    /// cohesiond first, and start it again afterward.
    ///
    /// Everything in this server's database will be replaced. This can't be
    /// undone, so export it first to keep a copy.
    ///
    /// Only data moves. This server keeps its own key and config.json, so
    /// every client needs to pair with it again, using the setup code
    /// cohesiond prints when it starts. Clients paired with the exporting
    /// server are revoked.
    Import {
        /// A file written by `cohesiond export`.
        file: PathBuf,
    },
}
