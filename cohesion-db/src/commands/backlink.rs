//! Backlinks, kept on behalf of the items whose text links to others: a
//! command that writes such an item runs [`FollowLinksCommand`] on what it
//! wrote, which rewrites the item's backlinks from its text, and
//! [`BacklinksOfCommand`] answers what links to an item. The rule for which
//! text is a link is `outgoing_links` (`cohesion_core::model`), which the
//! `backlinks` migration filled the table by.

use std::collections::HashSet;

use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    Backlink, Embed, ItemLink, LinkKind, LinkSource, LinkTarget, OutgoingLink, outgoing_links,
};
use cohesion_core::protocol::BacklinksOfRequest;

use crate::changed::{Changed, Invalidates};
use crate::commands::embed::item_link;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Read,
    ReadableDatabase, ResolveLinkCommand, Write,
};

/// Rewrites the backlinks of `source` from `text`, its text as just written,
/// or removes them all when it is `None`, the item deleted. Invalidates
/// `backlinks` of every item it linked to before or links to now, since
/// what each shows of the source (its title, whether it is there) may have
/// changed even where the links didn't.
pub struct FollowLinksCommand {
    pub source: LinkTarget,
    pub text: Option<String>,
}

impl FollowLinksCommand {
    /// Follows the links of `item`, as it now stands.
    pub fn of(item: &impl LinkSource) -> Self {
        Self {
            source: item.link_target(),
            text: item.linking_text().map(str::to_string),
        }
    }
}

impl DatabaseCommand for FollowLinksCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let before: Vec<Backlink> = ctx
            .db
            .select()
            .filter_eq(Backlink::SOURCE_KIND, self.source.kind)
            .filter_eq(Backlink::SOURCE_ID, self.source.id)
            .order("id")
            .all()?;
        let now = self
            .text
            .map(|text| outgoing_links(self.source, &text))
            .unwrap_or_default();
        let kept: Vec<OutgoingLink> = before
            .iter()
            .map(|backlink| OutgoingLink {
                target: backlink.target(),
                link: backlink.link.clone(),
            })
            .collect();
        if kept != now {
            ctx.db
                .delete::<Backlink>()
                .filter_eq(Backlink::SOURCE_KIND, self.source.kind)
                .filter_eq(Backlink::SOURCE_ID, self.source.id)
                .execute()?;
            for link in &now {
                ctx.db
                    .insert::<Backlink>()
                    .set(Backlink::SOURCE_KIND, self.source.kind)
                    .set(Backlink::SOURCE_ID, self.source.id)
                    .set(Backlink::TARGET_KIND, link.target.kind)
                    .set(Backlink::TARGET_ID, link.target.id)
                    .set(Backlink::LINK, link.link.clone())
                    .execute()?;
            }
        }
        let targets: HashSet<LinkTarget> =
            kept.iter().chain(&now).map(|link| link.target).collect();
        Ok(().invalidating(targets.into_iter().map(Invalidation::Backlinks)))
    }
}

/// The live items whose text links to the item `link` names, each as
/// `ResolveLinkCommand` resolves it, by title: a database row's document
/// as its row. One in a deleted workspace is left out, as it is gone.
#[derive(FromRequest)]
#[request(BacklinksOfRequest)]
pub struct BacklinksOfCommand {
    pub link: String,
}
impl DatabaseCommand for BacklinksOfCommand {
    type Response = Vec<Embed>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Embed>> {
        let target = item_link(&self.link)?.target();
        let sources: Vec<Backlink> = ctx
            .db
            .select()
            .filter_eq(Backlink::TARGET_KIND, target.kind)
            .filter_eq(Backlink::TARGET_ID, target.id)
            .order("id")
            .all()?;
        let mut embeds = Vec::new();
        for backlink in sources {
            let Some(link) = source_link(backlink.source()) else {
                continue;
            };
            match (ResolveLinkCommand {
                link: link.to_string(),
            })
            .handle(ctx)
            {
                Ok(embed) => embeds.push(embed),
                Err(CommandError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        embeds.sort_by_cached_key(|embed| embed.title.to_lowercase());
        Ok(embeds)
    }
}

/// A link to `source`, an item backlinks are kept from, which
/// `ResolveLinkCommand` resolves only while it is live.
fn source_link(source: LinkTarget) -> Option<ItemLink> {
    match source.kind {
        LinkKind::Document => Some(ItemLink::Document {
            workspace_id: None,
            id: source.id,
        }),
        LinkKind::Task => Some(ItemLink::Task {
            workspace_id: None,
            id: source.id,
        }),
        // Only documents' and tasks' links are followed so far.
        LinkKind::Resource
        | LinkKind::Database
        | LinkKind::DatabaseRow
        | LinkKind::Contact
        | LinkKind::Agent
        | LinkKind::ActorSession
        | LinkKind::Article => None,
    }
}

#[cfg(test)]
mod tests;
