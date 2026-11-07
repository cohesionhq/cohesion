use clap::Subcommand;
use cohesion_core::model::{Tag, TagIds};
use cohesion_core::protocol::{
    CreateTagRequest, DeleteTagRequest, ListTagsRequest, RenameTagRequest,
};

use super::{table, terminal_io::confirmed};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Subcommand)]
pub enum TagCommand {
    /// List the current workspace's tags.
    List,
    /// Create a tag.
    Create { name: String },
    /// Rename a tag.
    Rename {
        /// Tag id or name.
        tag: String,
        /// What to call it now.
        name: String,
    },
    /// Delete a tag, after asking for confirmation. The tag will also be removed from every task.
    Delete {
        /// Tag id or name.
        tag: String,
        /// Don't ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

pub async fn run(ctx: &Context, command: TagCommand) -> Result<(), CliError> {
    match command {
        TagCommand::List => list(ctx).await,
        TagCommand::Create { name } => create(ctx, name).await,
        TagCommand::Rename { tag, name } => rename(ctx, tag, name).await,
        TagCommand::Delete { tag, yes } => delete(ctx, tag, yes).await,
    }
}

/// Workspace `workspace_id`'s tags.
pub async fn tags(ctx: &Context, workspace_id: i64) -> Result<Vec<Tag>, CliError> {
    ctx.call(ListTagsRequest { workspace_id }).await
}

/// The tag whose id or name is `query`.
pub async fn find_tag(ctx: &Context, workspace_id: i64, query: &str) -> Result<Tag, CliError> {
    find(&tags(ctx, workspace_id).await?, query).cloned()
}

/// The names of the tags `ids` among `tags`, joined with commas, for a listing's column.
pub fn names(tags: &[Tag], ids: &TagIds) -> String {
    tags.iter()
        .filter(|tag| ids.0.contains(&tag.id))
        .map(|tag| tag.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

async fn list(ctx: &Context) -> Result<(), CliError> {
    let tags = tags(ctx, ctx.workspace().await?).await?;
    table::list(
        &tags,
        "No tags.\nCreate one with `cohesion tag create <name>`.",
        |t| row![>t.id, t.name],
    );
    Ok(())
}

async fn create(ctx: &Context, name: String) -> Result<(), CliError> {
    let tag = ctx
        .call(CreateTagRequest {
            workspace_id: ctx.workspace().await?,
            name,
        })
        .await?;
    println!("Created {}.", describe(&tag));
    Ok(())
}

async fn rename(ctx: &Context, tag: String, name: String) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let found = find_tag(ctx, workspace_id, &tag).await?;
    let renamed = ctx
        .call(RenameTagRequest {
            workspace_id,
            tag_id: found.id,
            name,
        })
        .await?;
    println!("Renamed {} to {}.", describe(&found), describe(&renamed));
    Ok(())
}

async fn delete(ctx: &Context, tag: String, yes: bool) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let found = find_tag(ctx, workspace_id, &tag).await?;
    if !confirmed(&format!("Delete {}?", describe(&found)), yes)? {
        return Ok(());
    }
    let deleted = ctx
        .call(DeleteTagRequest {
            workspace_id,
            tag_id: found.id,
        })
        .await?;
    println!("Deleted {}.", describe(&deleted));
    Ok(())
}

/// `tag "name" (id)`, as messages name a tag.
fn describe(tag: &Tag) -> String {
    format!("tag \"{}\" ({})", tag.name, tag.id)
}
