//! Terminal input and output the commands share: confirmation prompts, a spinner, piped
//! input and the editor.

use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    process,
    time::Duration,
};

use crate::cli_error::CliError;

/// Whether to go ahead with a deletion. `yes` skips the question; otherwise it's asked on
/// stderr, only `y` or `yes` counts, and a refusal prints `Cancelled.`.
pub fn confirmed(question: &str, yes: bool) -> Result<bool, CliError> {
    if yes {
        return Ok(true);
    }
    eprint!("{question} [y/N] ");
    io::stderr().flush().map_err(CliError::CannotReadTerminal)?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(CliError::CannotReadTerminal)?;
    let confirmed = matches!(answer.trim().to_lowercase().as_str(), "y" | "yes");
    if !confirmed {
        eprintln!("Cancelled.");
    }
    Ok(confirmed)
}

/// Awaits `work` while a spinner and `label` turn on stderr, erased when it finishes. Draws
/// nothing when stderr isn't a terminal, so redirected output stays clean.
pub async fn spin<T>(label: &str, work: impl Future<Output = T>) -> T {
    if !io::stderr().is_terminal() {
        return work.await;
    }
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut frames = FRAMES.iter().cycle();
    let mut ticks = tokio::time::interval(Duration::from_millis(80));
    tokio::pin!(work);
    loop {
        tokio::select! {
            output = &mut work => {
                // Back to the start of the line, then clear it.
                eprint!("\r\x1b[K");
                return output;
            }
            _ = ticks.tick() => eprint!("\r{} {label}", frames.next().expect("cycle never ends")),
        }
    }
}

/// `words` joined with spaces, followed by whatever is piped to stdin.
pub fn words_and_stdin(words: &[String]) -> Result<String, CliError> {
    let mut text = words.join(" ");
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        let mut piped = String::new();
        stdin
            .lock()
            .read_to_string(&mut piped)
            .map_err(CliError::CannotReadStdin)?;
        if !text.is_empty() && !piped.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&piped);
    }
    Ok(text)
}

/// Opens `contents` in $VISUAL or $EDITOR (else `vi`) and returns the text it was saved
/// with. The temporary file is removed afterwards.
pub fn edit_in_editor(contents: &str) -> Result<String, CliError> {
    let path = env::temp_dir().join(format!("cohesion-{}.md", process::id()));
    fs::write(&path, contents).map_err(CliError::Tempfile)?;

    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .expect("EDITOR cannot be blank because it falls back to vi.");

    let status = process::Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .map_err(CliError::CannotLaunchEditor)?;
    if !status.success() {
        return Err(CliError::EditorFailed);
    }
    let edited = fs::read_to_string(&path).map_err(CliError::Tempfile)?;
    // Best effort: a leftover file in the temp directory is harmless.
    let _ = fs::remove_file(&path);
    Ok(edited)
}
