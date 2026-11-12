//! Resolving a `cohesion://` link to the item it names, as an `Embed`, and
//! Markdown to its text and embeds.

use cohesion_core::FromRequest;
use cohesion_core::model::{
    Actor, ActorMessageContent, ActorMessageKind, ActorMessageSender, ActorSessionMessage,
    ActorSessionMessageContent, Document, Embed, Feed, ItemLink, TABLE_PREVIEW_ROWS, Task,
    TextPiece, TextSection, text_pieces,
};
use cohesion_core::protocol::{
    ResolveEmbedsRequest, ResolveLinkRequest, ResolveLinksRequest, ResolvedLink,
};

use crate::commands::document_folder::GetDocumentFolderCommand;
use crate::commands::document_folder_property::ListDocumentFolderPropertiesCommand;
use crate::commands::document_value::first_rows;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    GetActorSessionCommand, GetArticleCommand, GetContactCommand, GetIntelligenceAgentCommand,
    GetResourceCommand, Read, ReadableDatabase, ValidationError,
};

/// `link`, a request's `link` field, as the item link it is, or a validation
/// failure on that field.
pub(crate) fn item_link(link: &str) -> Result<ItemLink, CommandError> {
    link.trim()
        .parse()
        .map_err(|error: cohesion_core::model::NotAnItemLink| {
            ValidationError::in_field("link", error.to_string()).into()
        })
}

/// The live item `link` names. A task or document in a deleted workspace is
/// as gone as a deleted one.
#[derive(FromRequest)]
#[request(ResolveLinkRequest)]
pub struct ResolveLinkCommand {
    pub link: String,
}
impl DatabaseCommand for ResolveLinkCommand {
    type Response = Embed;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Embed> {
        Ok(match item_link(&self.link)? {
            ItemLink::Task { workspace_id, id } => {
                let select = ctx.db.select_live::<Task>().filter_eq(Task::ID, id);
                let select = match workspace_id {
                    Some(ws) => select.filter_eq(Task::WORKSPACE_ID, ws),
                    None => select,
                };
                let task = select.one().found()?;
                ensure_workspace(ctx.db, task.workspace_id)?;
                Embed::task(task)
            }
            ItemLink::Document { workspace_id, id } => {
                let select = ctx.db.select_live::<Document>().filter_eq(Document::ID, id);
                let select = match workspace_id {
                    Some(ws) => select.filter_eq(Document::WORKSPACE_ID, ws),
                    None => select,
                };
                let document = select.one().found()?;
                ensure_workspace(ctx.db, document.workspace_id)?;
                Embed::document(document)
            }
            ItemLink::Resource { workspace_id, id } => Embed::resource(
                GetResourceCommand {
                    workspace_id,
                    resource_id: id,
                }
                .handle(ctx)?,
            ),
            ItemLink::DocumentFolder { workspace_id, id } => {
                let folder = GetDocumentFolderCommand {
                    workspace_id,
                    folder_id: id,
                }
                .handle(ctx)?;
                let properties = ListDocumentFolderPropertiesCommand {
                    workspace_id,
                    folder_id: id,
                }
                .handle(ctx)?;
                let (rows, row_count) =
                    first_rows(ctx.db, workspace_id, id, &properties, TABLE_PREVIEW_ROWS)?;
                Embed::document_folder(folder, properties, rows, row_count)
            }
            ItemLink::Contact { workspace_id, id } => Embed::contact(
                GetContactCommand {
                    workspace_id,
                    contact_id: id,
                }
                .handle(ctx)?,
            ),
            ItemLink::Agent { id } => {
                Embed::agent(GetIntelligenceAgentCommand { agent_id: id }.handle(ctx)?)
            }
            ItemLink::ActorSession { id } => {
                let session = GetActorSessionCommand { id }.handle(ctx)?;
                let actor = ctx
                    .db
                    .select_live::<Actor>()
                    .filter_eq(Actor::ID, session.actor_id)
                    .one()?;
                let latest = ctx
                    .db
                    .select::<ActorSessionMessage>()
                    .filter("session_id = ? AND sender = ? AND kind = ?")
                    .bind(id)
                    .bind(ActorMessageSender::Actor)
                    .bind(ActorMessageKind::Text)
                    .order("id DESC")
                    .limit(1)
                    .one()?
                    .and_then(|message| match message.content {
                        ActorSessionMessageContent::Actor(ActorMessageContent::Text { text }) => {
                            Some(text)
                        }
                        _ => None,
                    });
                Embed::actor_session(session, actor.map(|a| a.title), latest.as_deref())
            }
            ItemLink::Article { id } => {
                let article = GetArticleCommand { article_id: id }.handle(ctx)?;
                let feed = ctx
                    .db
                    .select::<Feed>()
                    .filter_eq(Feed::ID, article.feed_id)
                    .one()
                    .found()?;
                Embed::article(article, feed.title)
            }
        })
    }
}

/// Markdown `text` split into its text and its embeds (`text_pieces`), each
/// embed resolved as `ResolveLinkCommand` resolves it, or `None` once its
/// item is gone.
#[derive(FromRequest)]
#[request(ResolveEmbedsRequest)]
pub struct ResolveEmbedsCommand {
    pub text: String,
}
impl DatabaseCommand for ResolveEmbedsCommand {
    type Response = Vec<TextSection>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<TextSection>> {
        text_pieces(&self.text)
            .into_iter()
            .map(|piece| {
                Ok(match piece {
                    TextPiece::Markdown(text) => TextSection::Markdown { text },
                    TextPiece::Embed { href, label, .. } => TextSection::Embed {
                        embed: resolve_if_live(ctx, &href)?,
                        link: href,
                        label,
                    },
                })
            })
            .collect()
    }
}

/// Each of `links` resolved as [`ResolveLinkCommand`] resolves it, in
/// order, or `None` once its item is gone.
#[derive(FromRequest)]
#[request(ResolveLinksRequest)]
pub struct ResolveLinksCommand {
    pub links: Vec<String>,
}
impl DatabaseCommand for ResolveLinksCommand {
    type Response = Vec<ResolvedLink>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<ResolvedLink>> {
        self.links
            .into_iter()
            .map(|link| {
                Ok(ResolvedLink {
                    embed: resolve_if_live(ctx, &link)?,
                    link,
                })
            })
            .collect()
    }
}

/// `link` resolved as [`ResolveLinkCommand`] resolves it, or `None` when
/// the item it names is gone.
fn resolve_if_live(
    ctx: &mut CommandContext<'_, Read>,
    link: &str,
) -> DatabaseCommandResult<Option<Box<Embed>>> {
    match (ResolveLinkCommand {
        link: link.to_string(),
    })
    .handle(ctx)
    {
        Ok(embed) => Ok(Some(Box::new(embed))),
        Err(CommandError::NotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
