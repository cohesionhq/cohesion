use std::{fs, path::PathBuf};

use clap::Subcommand;
use cohesion_core::protocol::{CallExtensionRequest, RegisterExtensionRequest};

use crate::{cli_context::Context, cli_error::CliError};

#[derive(Subcommand)]
pub enum ExtensionCommand {
    /// Run a script file on cohesiond and print its result.
    Install {
        /// Path to the script.
        file: PathBuf,
        /// Replace the extension if it's already installed.
        #[arg(short = 'o', long = "overwrite")]
        overwrite: bool,
    },
    /// Send a request to an extension's `serviceRequest` and print what it returns as JSON.
    Call {
        /// The extension's manifest identifier, like `com.cohesion.cloud.ios-companion`.
        identifier: String,
        /// The request's name, as the extension names it.
        name: String,
        /// The request's arguments as JSON; `null` when left out.
        args: Option<String>,
    },
}

pub async fn run(ctx: &Context, command: ExtensionCommand) -> Result<(), CliError> {
    match command {
        ExtensionCommand::Install { file, overwrite } => {
            install_extension(ctx, file, overwrite).await
        }
        ExtensionCommand::Call {
            identifier,
            name,
            args,
        } => call(ctx, identifier, name, args).await,
    }
}

/// Calls `serviceRequest(name, args)` on the extension with manifest
/// `identifier` and prints the result.
async fn call(
    ctx: &Context,
    identifier: String,
    name: String,
    args: Option<String>,
) -> Result<(), CliError> {
    let args = match args {
        Some(args) => serde_json::from_str(&args).map_err(CliError::InvalidRequest)?,
        None => serde_json::Value::Null,
    };
    let called = ctx
        .call(CallExtensionRequest {
            identifier,
            name,
            args,
        })
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&called.result).expect("a JSON value always serializes")
    );
    Ok(())
}

async fn install_extension(ctx: &Context, file: PathBuf, overwrite: bool) -> Result<(), CliError> {
    let script = fs::read_to_string(file).map_err(CliError::CannotReadFile)?;
    ctx.call(RegisterExtensionRequest {
        script,
        allow_overwrite: overwrite,
    })
    .await?;
    Ok(())
}
