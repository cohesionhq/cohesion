use std::io::{self, Read};

use cohesion_core::protocol::Command;

use crate::{cli_context::Context, cli_error::CliError};

/// Sends one raw protocol command and prints cohesiond's reply as JSON.
///
/// `request` is the JSON of a `Command`, e.g.
/// `{"type":"createWorkspace","args":{"title":"Home"}}`; `None` or `-` reads it from stdin.
pub async fn run(ctx: &Context, request: Option<String>) -> Result<(), CliError> {
    let request = match request {
        Some(request) if request != "-" => request,
        _ => read_stdin()?,
    };
    let command = parse_request(&request)?;
    // Sent untyped: the raw outcome, errors included, is what gets printed.
    let outcome = ctx.send(command).await?;

    // Only the reply goes to stdout, so output can be piped into jq.
    println!(
        "{}",
        serde_json::to_string_pretty(&outcome).expect("reply data always serializes")
    );
    Ok(())
}

/// Reads the whole request from stdin.
fn read_stdin() -> Result<String, CliError> {
    let mut request = String::new();
    io::stdin()
        .read_to_string(&mut request)
        .map_err(CliError::CannotReadStdin)?;
    Ok(request)
}

/// Parses the JSON of a protocol `Command`.
fn parse_request(request: &str) -> Result<Command, CliError> {
    serde_json::from_str(request).map_err(CliError::InvalidRequest)
}
