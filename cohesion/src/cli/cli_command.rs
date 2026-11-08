use std::path::PathBuf;

use clap::Subcommand;

use super::{
    agent, article, extension, intelligence, journal, noun::Noun, open, pair, script, search,
    status::status, tag, task, team, transfer, workspace,
};
use crate::{cli_context::Context, cli_error::CliError};

// A doc comment here would replace `--help`'s about text.
#[derive(Subcommand)]
pub enum CliCommand {
    /// Show which cohesiond this client talks to, and the workspace it uses.
    Status,
    /// Run script files on cohesiond, or create new ones.
    #[command(subcommand)]
    Script(script::ScriptCommand),
    /// Run a script file on cohesiond and print its result; short for `script run`.
    Run {
        /// Path to the script.
        file: PathBuf,
    },
    #[command(subcommand)]
    Extension(extension::ExtensionCommand),
    /// Manage workspaces, or list them, marking the selected one with `*`, when no
    /// subcommand is given.
    Workspace(Noun<workspace::WorkspaceCommand>),
    /// Select the workspace that later commands act in, or list them when none is given.
    Switch {
        /// Workspace id or title (titles match case-insensitively).
        // Named apart from the global `--workspace`, which clap would otherwise treat as the same arg.
        #[arg(value_name = "WORKSPACE")]
        target: Option<String>,
    },
    /// Read, edit or add to the current workspace's journal. With no subcommand, opens it
    /// in your editor, or appends text piped to stdin.
    Journal(Noun<journal::JournalCommand>),
    /// List and add tasks in the current workspace; lists open tasks when no subcommand is
    /// given.
    Task(task::TaskArgs),
    /// Read what your feeds brought in and follow feeds; lists the articles when no subcommand
    /// is given.
    Article(Noun<article::ArticleCommand>),
    /// Manage the tags tasks are grouped by in the current workspace.
    #[command(subcommand)]
    Tag(tag::TagCommand),
    /// Search the current workspace's documents, tasks and resources.
    Search {
        /// What to search for; several words are joined with spaces.
        #[arg(required = true)]
        query: Vec<String>,
    },
    /// Manage the endpoints, their models and the tier defaults agents use, or show them all
    /// when no subcommand is given.
    #[command(visible_alias = "ai")]
    Intelligence(Noun<intelligence::IntelligenceCommand>),
    /// Manage agents and talk to them, or list them when no subcommand is given.
    Agent(Noun<agent::AgentCommand>),
    /// Manage the teams agents are organized into, or list them when no subcommand is given.
    Team(Noun<team::TeamCommand>),
    /// Print a setup code that pairs another client with this cohesiond.
    Pair,
    /// Copy all data on cohesiond to a new file, for `cohesion import` on another server.
    /// Safe while cohesiond is running.
    ///
    /// The file holds everything cohesiond keeps: workspaces, tasks, documents, agents and
    /// their chats, images, extensions and settings. It doesn't hold the server's key.
    Export {
        /// The file to write. It mustn't exist yet.
        file: PathBuf,
    },
    /// Replace all data on cohesiond with a file from `cohesion export`, then restart it.
    ///
    /// Everything on this cohesiond will be replaced. This can't be undone, so export it
    /// first to keep a copy.
    ///
    /// Only data moves. This cohesiond keeps its own key and the clients paired with it.
    /// Clients paired with the server the file came from need to pair with this one.
    Import {
        /// A file written by `cohesion export`.
        file: PathBuf,
        /// Replace without asking first.
        #[arg(short, long)]
        yes: bool,
    },
    /// Serve the web UI on this machine, open it in your browser, and run until Ctrl-C.
    Open(open::OpenArgs),
}

pub async fn run(ctx: &mut Context, command: CliCommand) -> Result<(), CliError> {
    match command {
        CliCommand::Status => status(ctx).await,
        CliCommand::Script(command) => script::run(ctx, command).await,
        CliCommand::Run { file } => script::run_file(ctx, file).await,
        CliCommand::Workspace(Noun { verb }) => workspace::run(ctx, verb).await,
        CliCommand::Switch { target } => workspace::switch(ctx, target).await,
        CliCommand::Journal(Noun { verb }) => journal::run(ctx, verb).await,
        CliCommand::Task(args) => task::run(ctx, args).await,
        CliCommand::Article(Noun { verb }) => article::run(ctx, verb).await,
        CliCommand::Tag(command) => tag::run(ctx, command).await,
        CliCommand::Search { query } => search::run(ctx, query).await,
        CliCommand::Intelligence(Noun { verb }) => intelligence::run(ctx, verb).await,
        CliCommand::Agent(Noun { verb }) => agent::run(ctx, verb).await,
        CliCommand::Team(Noun { verb }) => team::run(ctx, verb).await,
        CliCommand::Extension(ext) => extension::run(ctx, ext).await,
        CliCommand::Pair => pair::pair(ctx).await,
        CliCommand::Export { file } => transfer::export(ctx, file).await,
        CliCommand::Import { file, yes } => transfer::import(ctx, file, yes).await,
        CliCommand::Open(args) => open::run(ctx, args).await,
    }
}
