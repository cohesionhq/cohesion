//! Opens a URL in the user's default browser.

use std::{io, process::Command};

pub fn open(url: &str) -> io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("cmd");
        // `start`'s first quoted argument is the window title.
        command.args(["/C", "start", ""]);
        command
    } else {
        Command::new("xdg-open")
    };
    let status = command.arg(url).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "the launcher exited with {status}"
        )))
    }
}
