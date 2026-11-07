use cohesion_core::protocol::{AboutRequest, ListWorkspacesRequest};

use crate::{cli_context::Context, cli_error::CliError};

/// Where this client's configuration lives, which cohesiond it reaches, and what it acts in.
/// Fails, after printing what it can, when cohesiond is unreachable.
pub async fn status(ctx: &Context) -> Result<(), CliError> {
    println!("Config     {}", ctx.config_root().display());
    println!("Server     {}", ctx.target());
    let about = match ctx.call(AboutRequest).await {
        Ok(about) => about,
        Err(err) => {
            println!("cohesiond  unreachable");
            return Err(err);
        }
    };
    println!("cohesiond  {} on {}", about.version, about.hostname);

    let workspaces = ctx.call(ListWorkspacesRequest).await?;
    let workspace = match ctx.current_workspace(&workspaces)? {
        None => "None. Select one with `cohesion switch <workspace>`.".to_string(),
        Some(id) => match workspaces.iter().find(|w| w.id == id) {
            Some(w) => format!("{} ({})", w.title, w.id),
            None => format!("{id} no longer exists. Run `cohesion switch <workspace>`."),
        },
    };
    println!("Workspace  {workspace}");
    Ok(())
}
