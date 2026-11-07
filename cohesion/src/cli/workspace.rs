use clap::Subcommand;
use cohesion_core::{
    Workspace,
    protocol::{
        CreateWorkspaceRequest, DeleteWorkspaceRequest, ListWorkspacesRequest,
        RenameWorkspaceRequest, SetWorkspaceEmojiRequest,
    },
};

use super::{table, terminal_io::confirmed};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Subcommand)]
pub enum WorkspaceCommand {
    /// Create a workspace and switch to it.
    Create {
        title: String,
        /// An emoji shown in place of the title's initial.
        #[arg(long)]
        emoji: Option<String>,
    },
    /// Select the workspace that later commands act in.
    Switch {
        /// Workspace id or title (titles match case-insensitively).
        #[arg(value_name = "WORKSPACE")]
        target: String,
    },
    /// Retitle a workspace.
    Rename {
        /// Workspace id or title.
        #[arg(value_name = "WORKSPACE")]
        target: String,
        /// The new title.
        title: String,
    },
    /// Set a workspace's emoji, or clear it when none is given.
    Emoji {
        /// Workspace id or title.
        #[arg(value_name = "WORKSPACE")]
        target: String,
        /// The new emoji; leave out to clear it.
        emoji: Option<String>,
    },
    /// Delete a workspace, after asking for confirmation.
    Delete {
        /// Workspace id or title.
        #[arg(value_name = "WORKSPACE")]
        target: String,
        /// Don't ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

pub async fn run(ctx: &mut Context, verb: Option<WorkspaceCommand>) -> Result<(), CliError> {
    match verb {
        None => switch(ctx, None).await,
        Some(WorkspaceCommand::Create { title, emoji }) => create(ctx, title, emoji).await,
        Some(WorkspaceCommand::Switch { target }) => switch(ctx, Some(target)).await,
        Some(WorkspaceCommand::Rename { target, title }) => rename(ctx, target, title).await,
        Some(WorkspaceCommand::Emoji { target, emoji }) => set_emoji(ctx, target, emoji).await,
        Some(WorkspaceCommand::Delete { target, yes }) => delete(ctx, target, yes).await,
    }
}

/// Selects `target`, or lists workspaces, marking the current one, when it's `None`.
pub async fn switch(ctx: &mut Context, target: Option<String>) -> Result<(), CliError> {
    // Only select a workspace the server knows, so a typo can't stick around in the config.
    let workspaces = ctx.call(ListWorkspacesRequest).await?;
    let Some(target) = target else {
        let current = ctx.current_workspace(&workspaces)?;
        table::list(
            &workspaces,
            "No workspaces.\nCreate one with `cohesion workspace create <title>`.",
            |w| {
                let marker = if Some(w.id) == current { '*' } else { ' ' };
                row![marker, >w.id, w.title]
            },
        );
        return Ok(());
    };
    let found = find(&workspaces, &target)?;
    ctx.select_workspace(Some(found.id))?;
    println!("Switched to {}.", describe(found));
    Ok(())
}

async fn create(ctx: &mut Context, title: String, emoji: Option<String>) -> Result<(), CliError> {
    let workspace = ctx.call(CreateWorkspaceRequest { title, emoji }).await?;
    ctx.select_workspace(Some(workspace.id))?;
    println!("Created and switched to {}.", describe(&workspace));
    Ok(())
}

async fn rename(ctx: &Context, target: String, title: String) -> Result<(), CliError> {
    let found = ctx.find_workspace(&target).await?;
    let renamed = ctx
        .call(RenameWorkspaceRequest {
            id: found.id,
            title,
        })
        .await?;
    println!(
        "Renamed workspace \"{}\" to \"{}\".",
        found.title, renamed.title
    );
    Ok(())
}

async fn set_emoji(ctx: &Context, target: String, emoji: Option<String>) -> Result<(), CliError> {
    let found = ctx.find_workspace(&target).await?;
    let updated = ctx
        .call(SetWorkspaceEmojiRequest {
            id: found.id,
            emoji,
        })
        .await?;
    match &updated.emoji {
        Some(emoji) => println!("Set the emoji of {} to {emoji}.", describe(&updated)),
        None => println!("Cleared the emoji of {}.", describe(&updated)),
    }
    Ok(())
}

/// Deletes a workspace once confirmed, deselecting it if `switch` had selected it.
async fn delete(ctx: &mut Context, target: String, yes: bool) -> Result<(), CliError> {
    let found = ctx.find_workspace(&target).await?;
    if !confirmed(&format!("Delete {}?", describe(&found)), yes)? {
        return Ok(());
    }
    let deleted = ctx.call(DeleteWorkspaceRequest { id: found.id }).await?;
    if ctx.selected_workspace() == Some(deleted.id) {
        ctx.select_workspace(None)?;
    }
    println!("Deleted {}.", describe(&deleted));
    Ok(())
}

/// `workspace "Title" (id)`, as messages name a workspace.
fn describe(workspace: &Workspace) -> String {
    format!("workspace \"{}\" ({})", workspace.title, workspace.id)
}
