//! `Resource` in scripts.

use cohesion_core::model::{MetadataKey, MetadataOwner, Resource};
use cohesion_core::value::ValueError;
use cohesion_db::GetMetadataCommand;
use cohesion_macros::js_entity;
use rquickjs::{Ctx, Value};

use crate::conversion::ToJs;
use crate::conversion::throw_value_error;

/// A read-only copy of an item imported into a workspace from a connected
/// service, such as a Notion page. Get them from `Workspace.resources()` or
/// `SearchResult.open()`; scripts cannot change them.
///
/// The daemon refreshes `contents` from the service on its own. `contentKind`
/// says whether `contents` is Markdown, plain text or a JSON document to
/// `JSON.parse`. `kind` is the connector's identifier plus the item's kind
/// (`"com.cohesion.notion.page"`), and `lastUpdated` is in Unix seconds.
#[js_entity(Resource)]
impl JsResource {
    /// The first 500 characters of `contents`, ending in `…` when there is
    /// more. Use it to tell items apart without reading each one in full.
    pub fn excerpt(&self) -> String {
        super::excerpt(&self.inner.contents)
    }
    /// What the item's metadata says under `key`, such as
    /// `resource.get(MetadataKey.dateCreated)`, or `null` when the service
    /// didn't say. Dates are ISO 8601 in UTC (`"2026-10-24T09:30:00Z"`).
    #[ts(returns = "string | null")]
    pub fn get<'js>(
        &self,
        ctx: Ctx<'js>,
        #[ts(type = "MetadataKey")] key: String,
    ) -> rquickjs::Result<Value<'js>> {
        let key = MetadataKey::from_str_strict(&key)
            .map_err(|error: ValueError| throw_value_error(&ctx, error))?;
        let value = self.read(
            &ctx,
            GetMetadataCommand {
                owner: MetadataOwner::Resource {
                    resource_id: self.inner.id,
                },
                key: key.as_str().to_string(),
            },
        )?;
        value.to_js(&ctx)
    }
}
