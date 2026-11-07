use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::{Connector, ConnectorAccount, ResourceKind};

/// What a resource's `contents` hold; stored as text.
///
/// - `"markdown"`: Markdown text, searchable like a document.
/// - `"text"`: plain text, searchable like a document.
/// - `"json"`: a JSON document as text; parse it with `JSON.parse`. Only the
///   title is searchable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum ResourceContentKind {
    Markdown,
    Text,
    Json,
}

/// What a resource's metadata may say, as a connector fills it. Dates are
/// ISO 8601 in UTC with a `Z` suffix (`"2026-10-24T09:30:00Z"`), so they
/// sort as text; everything else is text as the service shows it.
///
/// - `"dateCreated"`: when the item was created.
/// - `"dateModified"`: when the item's contents last changed.
/// - `"dateDue"`: when the item is due, for a task.
/// - `"dateStart"`: when an event starts.
/// - `"dateEnd"`: when an event ends.
/// - `"author"`: who wrote or owns the item.
/// - `"url"`: where the item opens at the service.
/// - `"location"`: where an event takes place, as the service shows it.
/// - `"folder"`: the folder, notebook or path holding the item, such as
///   `"Work/Plans"`.
/// - `"category"`: the category, project or list the item is filed under.
/// - `"keywords"`: tags or labels, separated by commas.
/// - `"status"`: the item's state at the service, such as `"Done"`.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, TextEnum, TsWire,
)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum MetadataKey {
    DateCreated,
    DateModified,
    DateDue,
    DateStart,
    DateEnd,
    Author,
    Url,
    Location,
    Folder,
    Category,
    Keywords,
    Status,
}

/// The order a resource listing comes back in. Ties fall back to the
/// newest import, and resources with no value for a metadata key come last
/// in either direction.
///
/// - `"title"`: alphabetical, ignoring case.
/// - `"lastUpdated"`: when the item last changed in the connected service.
/// - a metadata key: that key's value, as text ignoring case, so dates sort
///   by time.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TsWire)]
#[ts_wire(type = "'title' | 'lastUpdated' | MetadataKey")]
#[serde(from = "ResourceSortRepr", into = "ResourceSortRepr")]
pub enum ResourceSort {
    Title,
    LastUpdated,
    Metadata(MetadataKey),
}

/// [`ResourceSort`] on the wire: a column's name or a metadata key.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ResourceSortRepr {
    Column(ResourceColumn),
    Metadata(MetadataKey),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ResourceColumn {
    Title,
    LastUpdated,
}

impl From<ResourceSortRepr> for ResourceSort {
    fn from(repr: ResourceSortRepr) -> Self {
        match repr {
            ResourceSortRepr::Column(ResourceColumn::Title) => Self::Title,
            ResourceSortRepr::Column(ResourceColumn::LastUpdated) => Self::LastUpdated,
            ResourceSortRepr::Metadata(key) => Self::Metadata(key),
        }
    }
}

impl From<ResourceSort> for ResourceSortRepr {
    fn from(sort: ResourceSort) -> Self {
        match sort {
            ResourceSort::Title => Self::Column(ResourceColumn::Title),
            ResourceSort::LastUpdated => Self::Column(ResourceColumn::LastUpdated),
            ResourceSort::Metadata(key) => Self::Metadata(key),
        }
    }
}

/// A read-only copy of an item imported into a workspace from a connected
/// service. The daemon refreshes it from the service periodically; scripts
/// cannot change it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "resources", backup = Included)]
#[model(join = ConnectorAccount, on = "connector_accounts.id = resources.account_id")]
#[model(join = Connector, on = "connectors.id = connector_accounts.connector_id")]
#[model(join = ResourceKind, on = "resource_kinds.id = resources.kind_id")]
pub struct Resource {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace this resource was imported into.
    pub workspace_id: i64,
    /// Id of the connector account the resource came from.
    pub account_id: i64,

    /// The item's identifier in the connected service.
    pub identifier: String,
    /// Id of the kind of item this is, one its connector declares.
    pub kind_id: i64,
    /// What sort of item this is, as `"<connector identifier>.<kind>"`, for
    /// example `"com.example.notion.page"`. Filter on it with
    /// `workspace.resources({ kind })`.
    #[model(expr = Resource::KIND_SQL)]
    pub kind: String,
    /// Display name.
    pub title: String,
    /// Whether `contents` is Markdown, plain text or JSON text.
    pub content_kind: ResourceContentKind,
    /// The item's contents: Markdown or plain text, or JSON text to
    /// `JSON.parse`, depending on the content kind.
    pub contents: String,
    /// When the item last changed in the connected service, in Unix
    /// **seconds** (in JavaScript use `new Date(lastUpdated * 1000)`).
    pub last_updated: i64,
    /// When this copy last changed in Cohesion, in Unix **seconds**.
    pub modified: i64,
}

impl Resource {
    /// A resource's full kind: its connector's identifier and its kind's
    /// short key.
    pub const KIND_SQL: &str = "connectors.identifier || '.' || resource_kinds.identifier";
}

/// A kind of resource shown in a workspace. A kind is hidden in every
/// workspace until it is shown there; hidden, its resources stay.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "workspace_resource_kinds", backup = Included)]
pub struct WorkspaceResourceKind {
    pub id: i64,
    pub workspace_id: i64,
    pub kind_id: i64,
    /// Where the kind sits among those the workspace shows; a kind shown
    /// later goes last.
    pub sort_order: i64,
}
