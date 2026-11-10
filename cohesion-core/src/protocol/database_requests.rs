//! Requests about databases: their properties, rows and values, and a row's
//! own document.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{DatabaseDataType, DatabaseValue};

/// Lists the databases in a workspace, by title.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListDatabasesRequest {
    pub workspace_id: i64,
}

/// Fetches database `database_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetDatabaseRequest {
    pub workspace_id: i64,
    pub database_id: i64,
}

/// Creates a database with no properties or rows in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateDatabaseRequest {
    pub workspace_id: i64,
    pub title: String,
}

/// Retitles database `database_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameDatabaseRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub title: String,
}

/// Deletes database `database_id` in a workspace for good, with its
/// properties, rows, values and rows' documents.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDatabaseRequest {
    pub workspace_id: i64,
    pub database_id: i64,
}

/// Lists a database's properties in their order.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListDatabasePropertiesRequest {
    pub workspace_id: i64,
    pub database_id: i64,
}

/// Adds a property to a database, after its others: a formula computing
/// its values from `formula`'s source (see `DatabaseFormula`), or one whose
/// values are set by hand when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateDatabasePropertyRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub name: String,
    pub data_type: DatabaseDataType,
    pub formula: Option<String>,
}

/// Makes property `property_id` of a database a formula computing its
/// values from `formula`'s source, or changes its source; with `formula`
/// absent, makes it a property set by hand again, keeping its last outputs
/// as values. A property becoming a formula loses the values set by hand.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetDatabasePropertyFormulaRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub formula: Option<String>,
}

/// Renames property `property_id` of a database.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameDatabasePropertyRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub name: String,
}

/// Changes the type of property `property_id` of a database. Each value
/// whose text reads as the new type is kept as one (`"42"` becomes the
/// number 42); every other value is emptied.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetDatabasePropertyTypeRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub data_type: DatabaseDataType,
}

/// Moves property `property_id` of a database just before its property
/// `before_id`, or last when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MoveDatabasePropertyRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub before_id: Option<i64>,
}

/// Deletes property `property_id` of a database, with every row's value
/// for it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDatabasePropertyRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
}

/// What a filter keeps of a property's values. Values compare as their
/// property's type: text ignoring case, numbers as numbers, dates by day.
///
/// - `isEmpty`, `isNotEmpty`: whether the row holds a value; every type.
/// - `equals`, `notEquals`: the value is or isn't `value`, of the property's
///   type; every type.
/// - `contains`, `notContains`: the text holds `text` or doesn't; text only.
/// - `greaterThan`, `greaterOrEqual`, `lessThan`, `lessOrEqual`: against
///   `value`; integers, decimals, currency and dates (later is greater).
///
/// An empty value matches only `isEmpty`, `notEquals` and `notContains`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DatabaseCondition {
    IsEmpty,
    IsNotEmpty,
    Equals { value: DatabaseValue },
    NotEquals { value: DatabaseValue },
    Contains { text: String },
    NotContains { text: String },
    GreaterThan { value: DatabaseValue },
    GreaterOrEqual { value: DatabaseValue },
    LessThan { value: DatabaseValue },
    LessOrEqual { value: DatabaseValue },
}

/// Keeps the rows whose value for `property_id` meets `condition`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DatabaseFilter {
    pub property_id: i64,
    pub condition: DatabaseCondition,
}

/// Orders rows by their values for `property_id`, as its type compares
/// them, empty values last either way; ties by when the row was added.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DatabaseSort {
    pub property_id: i64,
    pub descending: bool,
}

/// Lists a database's rows with their values: those meeting every filter,
/// in `sort`'s order, or the order they were added when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListDatabaseRowsRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub filters: Vec<DatabaseFilter>,
    pub sort: Option<DatabaseSort>,
}

/// Fetches row `row_id` of a database with its values.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetDatabaseRowRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}

/// Adds an empty row to a database.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateDatabaseRowRequest {
    pub workspace_id: i64,
    pub database_id: i64,
}

/// Deletes row `row_id` of a database for good, with its values and its
/// document.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDatabaseRowRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}

/// Sets row `row_id`'s value for property `property_id`, which must be of
/// the property's type, or empties it when `value` is absent; refused for a
/// formula, whose values are computed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetDatabaseValueRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
    pub property_id: i64,
    pub value: Option<DatabaseValue>,
}

/// Gives row `row_id` of a database an empty document of its own; refused
/// when it has one. `deleteDocument` removes it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateDatabaseRowDocumentRequest {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}
