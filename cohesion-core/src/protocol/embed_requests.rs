//! Resolving a `cohesion://` link to what it names, for showing in place.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::Embed;

/// Resolves `link`, a `cohesion://` link to a task, document or journal
/// entry, resource, document folder, contact, agent, actor session or
/// article, to the item it names as an `Embed`. A link to an item deleted
/// or never made is `notFound`; text that isn't such a link fails
/// validation on `link`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResolveLinkRequest {
    /// As written in Markdown: `cohesion://task/3/42`.
    pub link: String,
}

/// Resolves each of `links` as `resolveLink` does, in one request: what a
/// screen showing many links' cards at once asks. A link to an item deleted
/// or never made resolves to no embed; text that isn't such a link fails
/// validation on `link`, failing the whole request.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResolveLinksRequest {
    pub links: Vec<String>,
}

/// One of [`ResolveLinksRequest`]'s links and the item it names, `None`
/// when that item is deleted or never was.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct ResolvedLink {
    /// As the request gave it.
    pub link: String,
    pub embed: Option<Box<Embed>>,
}

/// Splits Markdown `text` into its sections, in order: Markdown, and each
/// `cohesion://` link alone on its line outside code, resolved as
/// `resolveLink` resolves it. A client renders the Markdown and shows each
/// embed as a card, with no parsing of its own.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResolveEmbedsRequest {
    /// Any Markdown: a document's contents, a task's description, a draft.
    pub text: String,
}

/// The items whose text links to the item `link` names, as `resolveLink`
/// resolves each: a link anywhere in their text, alone on its line or in a
/// sentence, outside code. Documents, journal entries and tasks'
/// descriptions are followed so far. The item named needn't exist; text that isn't a `cohesion://`
/// link fails validation on `link`. `backlinks` of the item's kind and id
/// says when the list changed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct BacklinksOfRequest {
    /// As written in Markdown: `cohesion://task/3/42`, or without its
    /// workspace, `cohesion://task/42`.
    pub link: String,
}
