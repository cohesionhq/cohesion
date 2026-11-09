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
