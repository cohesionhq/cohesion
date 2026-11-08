//! `cohesiond export` and `cohesiond import`: moving this server's data to
//! another server, then exiting.

use std::path::Path;
use std::process::exit;

use cohesionkit::db::{export_database, import_database};

use crate::args::DataCommand;

/// Runs `command` against the database at `database`, says what happened
/// and exits non-zero if it failed.
pub async fn run(command: &DataCommand, database: &Path) {
    let done = match command {
        DataCommand::Export { file } => export_database(database, file).map(|bytes| {
            format!(
                "Exported this server’s data to {} ({:.1} MB). Copy it to the other server and run cohesiond import there.",
                file.display(),
                bytes as f64 / 1_000_000.0
            )
        }),
        DataCommand::Import { file } => import_database(file, database).await.map(|()| {
            format!(
                "Imported {}. This server kept its own key, so start cohesiond and pair each client again with the setup code it prints.",
                file.display()
            )
        }),
    };
    match done {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            exit(1)
        }
    }
}
