//! Requests about a document folder's table: its properties, its rows (the
//! documents in it) and their values. A folder is a table while it has a
//! property; documents come, go and move with the document requests.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{FormulaUpdateMode, IntelligenceAgentColor, PropertyDataType, PropertyValue};

/// Lists a document folder's properties in their order; none for a folder
/// that isn't a table.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListDocumentFolderPropertiesRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
}

/// A formula as a request sets it: its source, not empty (see
/// `PropertyFormula`), and when its outputs are computed again.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct FormulaDefinition {
    pub source: String,
    pub update_mode: FormulaUpdateMode,
}

/// An option as a request adds it: a name, not empty and unique ignoring
/// case among its property's, and its color, or `null` for none.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
#[ts_wire(input)]
pub struct NewPropertyOption {
    pub name: String,
    pub color: Option<IntelligenceAgentColor>,
}

/// Adds a property to a document folder, after its others, which makes a
/// plain folder a table: a formula computing its values as `formula`
/// defines, or one whose values are set by hand when absent. A choice
/// property starts with `options`, in that order, and is never a formula;
/// every other type takes none.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateDocumentFolderPropertyRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub name: String,
    pub data_type: PropertyDataType,
    pub formula: Option<FormulaDefinition>,
    pub options: Vec<NewPropertyOption>,
}

/// Makes property `property_id` of a folder a formula computing its values
/// as `formula` defines (refused for a choice property), or changes its
/// source or update mode; with
/// `formula` absent, makes it a property set by hand again, keeping its last
/// outputs as values. A property becoming a formula loses the values set by
/// hand. Changing only the update mode keeps the outputs and the error, and
/// computes nothing again.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetDocumentFolderPropertyFormulaRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub formula: Option<FormulaDefinition>,
}

/// Renames property `property_id` of a folder.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameDocumentFolderPropertyRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub name: String,
}

/// Changes the type of property `property_id` of a folder. Each value
/// whose text reads as the new type is kept as one (`"42"` becomes the
/// number 42); every other value is emptied. A choice's text is its
/// option's name. Becoming a choice makes an option of each distinct text,
/// ignoring case, in alphabetical order and with no color, and points the
/// values at them; a formula can't become one. A property that stops being a
/// choice loses its options.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetDocumentFolderPropertyTypeRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub data_type: PropertyDataType,
}

/// Moves property `property_id` of a folder just before its property
/// `before_id`, or last when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MoveDocumentFolderPropertyRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub before_id: Option<i64>,
}

/// Deletes property `property_id` of a folder, with every document's value
/// for it. Deleting the last makes the folder a plain one again.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDocumentFolderPropertyRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
}

/// Adds an option to choice property `property_id` of a folder, after its
/// others.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreatePropertyOptionRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub name: String,
    pub color: Option<IntelligenceAgentColor>,
}

/// Renames option `option_id` of a folder's choice property; the values
/// that hold it show the new name.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenamePropertyOptionRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub name: String,
}

/// Changes the color of option `option_id` of a folder's choice property,
/// or takes it away when `color` is absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetPropertyOptionColorRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub color: Option<IntelligenceAgentColor>,
}

/// Moves option `option_id` of a folder's choice property just before its
/// option `before_id`, or last when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MovePropertyOptionRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub before_id: Option<i64>,
}

/// Deletes option `option_id` of a folder's choice property; the values
/// that held it are emptied.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeletePropertyOptionRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
}

/// A column of a table to filter or sort on: the documents' titles, which
/// compare as text, or one of the folder's properties.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TableColumn {
    Title,
    Property { property_id: i64 },
}

/// What a filter keeps of a column's values. Values compare as their
/// column's type: text ignoring case, numbers as numbers, dates by day.
///
/// - `isEmpty`, `isNotEmpty`: whether the row holds a value; every type.
/// - `equals`, `notEquals`: the value is or isn't `value`, of the column's
///   type; every type. A choice is one option or isn't (an option of
///   another property matches no value).
/// - `contains`, `notContains`: the text holds `text` or doesn't; text only.
/// - `greaterThan`, `greaterOrEqual`, `lessThan`, `lessOrEqual`: against
///   `value`; integers, decimals, currency and dates (later is greater).
///
/// An empty value matches only `isEmpty`, `notEquals` and `notContains`. A
/// title is text and never empty.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TableCondition {
    IsEmpty,
    IsNotEmpty,
    Equals { value: PropertyValue },
    NotEquals { value: PropertyValue },
    Contains { text: String },
    NotContains { text: String },
    GreaterThan { value: PropertyValue },
    GreaterOrEqual { value: PropertyValue },
    LessThan { value: PropertyValue },
    LessOrEqual { value: PropertyValue },
}

/// Keeps the rows whose value in `column` meets `condition`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct TableFilter {
    pub column: TableColumn,
    pub condition: TableCondition,
}

/// Orders rows by their values in `column`, as its type compares them (a
/// choice by its options' order), empty values last either way; ties by
/// when the document was created.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct TableSort {
    pub column: TableColumn,
    pub descending: bool,
}

/// Lists a folder's live documents as rows, with their values: those
/// meeting every filter, in `sort`'s order, or the order they were created
/// when absent. A folder that isn't a table lists its documents with no
/// cells.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListTableRowsRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub filters: Vec<TableFilter>,
    pub sort: Option<TableSort>,
}

/// Fetches live document `document_id` of a folder as a row, with its
/// values.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetTableRowRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub document_id: i64,
}

/// Sets live document `document_id`'s value for property `property_id` of
/// its folder, which must be of the property's type, or empties it when
/// `value` is absent; refused for a formula, whose values are computed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetDocumentValueRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub document_id: i64,
    pub property_id: i64,
    pub value: Option<PropertyValue>,
}
