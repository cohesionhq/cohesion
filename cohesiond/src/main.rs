//! cohesiond: opens the database, builds the [`CohesionContext`], prints the
//! first setup code, and serves clients, clients among them, over HTTPS and a
//! local Unix socket until
//! a signal (or, with `--exit-with-parent`, its parent exiting) shuts it
//! down cleanly. A client finishing an import shuts it down the same way,
//! then it starts again onto the import (`restart`).

mod args;
mod daemon_storage;
mod restart;
mod server;
mod settings;
mod shutdown;

use clap::Parser;
use std::fmt::Display;
use std::process::exit;

use cohesionkit::db::{Container, ListClientsCommand, Secrets, StagedImport, clear_transfers};
use cohesionkit::log::{ConsoleAdapter, Logger, Module};
use cohesionkit::{CohesionContext, ServerAddress, log_info, log_warn};

use crate::{
    args::CohesiondArgs,
    daemon_storage::{ApplicationData, StoragePath},
    settings::Settings,
    shutdown::Signals,
};

#[tokio::main]
async fn main() {
    let args = CohesiondArgs::parse();
    let storage_path = match &args.config_directory {
        None => StoragePath::default(),
        Some(cfg) => StoragePath::resolve(cfg),
    }
    .unwrap_or_else(|err| fatal("could not use the config directory", err));
    let settings = Settings::resolve(args, &storage_path)
        .unwrap_or_else(|err| fatal("could not read the settings", err));
    let _ = Logger::new()
        .with(ConsoleAdapter::new(settings.log_level))
        .install();
    let mut signals = Signals::new();

    let mut container =
        Container::open(storage_path.database_file()).expect("could not open the database");
    container.migrate().expect("could not migrate the database");
    if let Err(err) = clear_transfers(&storage_path.database_file()) {
        log_warn!(
            Module::Database,
            "Couldn’t remove old exports and imports: {err}"
        );
    }

    let storage = ApplicationData::make(&storage_path)
        .unwrap_or_else(|err| fatal("could not load the server key", err));
    let certificate = storage
        .server_key
        .self_signed_certificate("cohesiond")
        .expect("could not create server certificate");
    let address = ServerAddress {
        host: settings.host,
        port: settings.public_port,
        fingerprint: storage.server_key.fingerprint(),
    };
    let context = CohesionContext::new(container, Secrets::new(), address.clone());

    context.start().await;

    open_first_pairing(&context, &address).await;

    // The server keeps answering while the daemon shuts down, refusing every
    // request, so clients hear why rather than finding nothing there.
    let mut server = tokio::spawn(server::start(
        context.clone(),
        certificate,
        settings.port,
        settings.socket,
        settings.proxy_protocol,
    ));
    let stop = tokio::select! {
        ended = &mut server => Stop::Server(ended),
        reason = shutdown::requested(&mut signals, settings.exit_with_parent) => {
            log_info!(Module::Server, "{reason}; shutting down. Press Ctrl-C again to stop now.");
            Stop::Asked
        }
        staged = context.pending_import.requested() => {
            log_info!(Module::Server, "A client imported data; restarting to put it in place.");
            Stop::Import(staged)
        }
    };
    let clean = shutdown::run(&context, &mut signals).await;
    // Dropping the server removes the socket file. One that ended on its
    // own has already dropped, and polling its handle again would panic.
    if !matches!(stop, Stop::Server(_)) {
        server.abort();
        let _ = server.await;
    }
    // A server that could not serve (a port already held, say) exits
    // non-zero, so a supervisor restarts it rather than reading a clean stop.
    let failed = match stop {
        Stop::Import(staged) => restart::onto(staged),
        Stop::Server(ended) => !matches!(ended, Ok(Ok(()))),
        Stop::Asked => false,
    };
    exit(if clean && !failed { 0 } else { 1 });
}

/// Why the daemon stopped serving.
enum Stop {
    /// The server ended on its own, with what it returned.
    Server(Result<std::io::Result<()>, tokio::task::JoinError>),
    /// A signal, or the parent exiting.
    Asked,
    /// A client finished an import, which the daemon restarts onto.
    Import(StagedImport),
}

/// With no clients yet, opens a pairing window and prints its setup code, the
/// way to reach a server no one can ask for one. The code goes to stdout, not
/// the log, which others may read; later codes come from `BeginPairing`.
async fn open_first_pairing(context: &CohesionContext, address: &ServerAddress) {
    let clients = context
        .database
        .send(ListClientsCommand {})
        .await
        .expect("could not list clients");
    if clients.is_empty() {
        let key = context.secrets.begin_pairing();
        println!("Setup code: {}", address.setup_code(key).encode());
        log_info!(Module::Server, "Pairing is open.");
    }
}

/// Prints `context: err` and exits, for startup failures with no one to report to.
fn fatal(context: &str, err: impl Display) -> ! {
    eprintln!("{context}: {err}");
    exit(1)
}
