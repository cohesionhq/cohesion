use cohesion_macros::{ColumnGroup, Model};
use serde::{Deserialize, Serialize};

use super::{Document, Resource, Task, TaskFolder, Workspace};
use crate::stored_model::Model;

/// One key/value pair a script attached to an item.
#[derive(Clone, Serialize, Deserialize, Model)]
#[model(table = "metadata", backup = Included)]
pub struct Metadata {
    /// Unique, stable identifier.
    pub id: i64,
    /// The item this pair belongs to.
    #[model(group)]
    pub owner: MetadataOwner,
    /// Unique per item.
    pub key: String,
    pub value: String,
}

/// The item a metadata pair belongs to. Real foreign keys, so an item's pairs
/// cascade with it, and the table's check holds a pair to one item. A new kind
/// of item is a column here, a variant and a [`HasMetadata`] impl.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ColumnGroup)]
pub enum MetadataOwner {
    Workspace { workspace_id: i64 },
    Document { document_id: i64 },
    Task { task_id: i64 },
    TaskFolder { task_folder_id: i64 },
    Resource { resource_id: i64 },
}

/// A model scripts can attach metadata to; `#[js_entity(Model, metadata)]`
/// needs it, and lists filter by metadata through it.
pub trait HasMetadata: Model {
    /// The `metadata` column naming this model's rows.
    const METADATA_COLUMN: &'static str;
    /// Row `id` of this model, as a pair's owner.
    fn metadata_owner(id: i64) -> MetadataOwner;
}

macro_rules! has_metadata {
    ($($model:ty => $variant:ident { $column:ident }),* $(,)?) => {
        $(impl HasMetadata for $model {
            const METADATA_COLUMN: &'static str = stringify!($column);
            fn metadata_owner(id: i64) -> MetadataOwner {
                MetadataOwner::$variant { $column: id }
            }
        })*
    };
}

has_metadata! {
    Workspace => Workspace { workspace_id },
    Document => Document { document_id },
    Task => Task { task_id },
    TaskFolder => TaskFolder { task_folder_id },
    Resource => Resource { resource_id },
}
