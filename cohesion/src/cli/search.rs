use cohesion_core::{model::SearchEntityKind, protocol::SearchWorkspaceRequest};

use super::table::{self, truncate};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row};

/// Each hit's kind, id and title, then the first line that mentions a search word.
pub async fn run(ctx: &Context, query: Vec<String>) -> Result<(), CliError> {
    let query = query.join(" ");
    let results = ctx
        .call(SearchWorkspaceRequest {
            workspace_id: ctx.workspace().await?,
            query: query.clone(),
        })
        .await?;

    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    table::list(&results, &format!("No matches for \"{query}\"."), |r| {
        let kind = match r.entity_kind {
            SearchEntityKind::Document => "document",
            SearchEntityKind::Task => "task",
            SearchEntityKind::Resource => "resource",
            SearchEntityKind::Contact => "contact",
            SearchEntityKind::TaskFolder => "task folder",
            SearchEntityKind::DocumentFolder => "document folder",
            SearchEntityKind::Feed => "feed",
        };
        let title = r.title.as_deref().unwrap_or("(untitled)");
        let line = r.contents.lines().map(str::trim).find(|line| {
            let line = line.to_lowercase();
            words.iter().any(|word| line.contains(word))
        });
        row![kind, >r.entity_id, title].detail(line.map(|line| truncate(line, 70)))
    });
    Ok(())
}
