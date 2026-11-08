//! Starting cohesiond again onto an import a client finished, once the
//! daemon has shut down and closed its database: the import replaces the
//! database file, and the process runs itself again with the same
//! arguments, so it keeps its process id, its supervisor and its stdin.

use std::process::exit;

use cohesionkit::db::StagedImport;
use cohesionkit::log::Module;
use cohesionkit::{log_error, log_info};

/// Puts `staged` in place of the database and starts cohesiond again.
/// Exits non-zero if either fails, so a supervisor starts it instead.
pub fn onto(staged: StagedImport) -> ! {
    if let Err(error) = staged.replace() {
        log_error!(Module::Server, "Couldn’t put the import in place: {error}");
        exit(1);
    }
    log_info!(Module::Server, "The import is in place; starting again.");
    let error = run_again();
    log_error!(Module::Server, "Couldn’t start again: {error}");
    exit(1)
}

/// Replaces this process with a new run of the same program and arguments;
/// returns only if that fails.
#[cfg(unix)]
fn run_again() -> std::io::Error {
    use std::os::unix::process::CommandExt;
    match std::env::current_exe() {
        Ok(program) => std::process::Command::new(program)
            .args(std::env::args_os().skip(1))
            .exec(),
        Err(error) => error,
    }
}
#[cfg(not(unix))]
fn run_again() -> std::io::Error {
    std::io::Error::other("this platform can’t restart in place")
}
