use std::io::{self, IsTerminal};

use clap::Subcommand;
use cohesion_core::protocol::{AppendToJournalRequest, EditJournalRequest, GetJournalRequest};

use super::calendar_day::today;
use super::terminal_io::{edit_in_editor, words_and_stdin};
use crate::{cli_context::Context, cli_error::CliError};

#[derive(Subcommand)]
pub enum JournalCommand {
    /// Print today's journal.
    Get,
    /// Open today's journal in $VISUAL or $EDITOR and save it when the editor exits.
    Edit,
    /// Add a line to today's journal.
    #[command(visible_alias = "add")]
    Append {
        /// Text to add; several words are joined with spaces. Text piped to stdin is appended.
        text: Vec<String>,
    },
}

pub async fn run(ctx: &Context, verb: Option<JournalCommand>) -> Result<(), CliError> {
    match verb {
        // Piped text has nowhere to go but the journal; an editor needs the terminal.
        None if !io::stdin().is_terminal() => append(ctx, Vec::new()).await,
        None | Some(JournalCommand::Edit) => edit(ctx).await,
        Some(JournalCommand::Get) => get(ctx).await,
        Some(JournalCommand::Append { text }) => append(ctx, text).await,
    }
}

async fn get(ctx: &Context) -> Result<(), CliError> {
    let contents = journal_contents(ctx, ctx.workspace().await?, today()).await?;
    print!("{contents}");
    if !contents.ends_with('\n') {
        println!();
    }
    Ok(())
}

/// Saves the journal only if the editor changed it.
async fn edit(ctx: &Context) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    // Saves to the day that was opened, even if the editor stays open past midnight.
    let date = today();
    let contents = journal_contents(ctx, workspace_id, date.clone()).await?;
    let content = edit_in_editor(&contents)?;
    if content == contents {
        println!("No changes to the journal.");
        return Ok(());
    }
    ctx.call(EditJournalRequest {
        workspace_id,
        date,
        content,
    })
    .await?;
    println!("Saved the journal.");
    Ok(())
}

/// The journal's text for `date`, empty if nothing was written that day.
async fn journal_contents(
    ctx: &Context,
    workspace_id: i64,
    date: String,
) -> Result<String, CliError> {
    let journal = ctx.call(GetJournalRequest { workspace_id, date }).await?;
    Ok(journal.map(|journal| journal.contents).unwrap_or_default())
}

async fn append(ctx: &Context, text: Vec<String>) -> Result<(), CliError> {
    let content = words_and_stdin(&text)?;
    let content = content.trim_end();
    if content.is_empty() {
        return Err(CliError::NothingToAppend);
    }
    ctx.call(AppendToJournalRequest {
        workspace_id: ctx.workspace().await?,
        date: today(),
        content: content.to_string(),
    })
    .await?;
    println!("Added to the journal.");
    Ok(())
}
