//! Resolving a `cohesion://` link to what it names, for showing in place.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Resolves `link`, a `cohesion://` link to a task, document or journal
/// entry, resource, agent, actor session or article, to the item it names
/// as an `Embed`. A link to an item deleted or never made is `notFound`;
/// text that isn't such a link fails validation on `link`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResolveLinkRequest {
    /// As written in Markdown: `cohesion://task/3/42`.
    pub link: String,
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
