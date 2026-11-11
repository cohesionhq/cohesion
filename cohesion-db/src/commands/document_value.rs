//! A table's rows and their values: the documents in a folder with
//! properties, each holding at most one value per property. A value is
//! stored in every representation its type gives; rows are listed filtered
//! and sorted in SQL; and a document's values go when it leaves the folder.

use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DocumentCell, DocumentFolderProperty, DocumentValue, PropertyDataType, PropertyValue, TableRow,
};
use cohesion_core::protocol::{
    GetTableRowRequest, ListTableRowsRequest, SetDocumentValueRequest, TableColumn, TableCondition,
    TableFilter, TableSort,
};
use cohesion_core::value::IntoValue;
use cohesion_core::{Document, FromRequest, Named, Projection, Value};

use crate::changed::{Changed, Invalidates};
use crate::commands::document_folder_property::{
    GetDocumentFolderPropertyCommand, ListDocumentFolderPropertiesCommand,
};
use crate::commands::document_formula::{forget_stale_marks, formulas_stale};
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered,
    Read, ReadableDatabase, Select, ValidationError, Write, WriteContainer,
};

/// What a row shows of its document: its id and title, without the
/// contents a table never shows.
#[derive(Projection, Clone, Debug)]
#[projection(of = Document)]
pub struct RowTitle {
    pub id: i64,
    pub title: String,
}

/// Stores document `document_id`'s value for `property`, which must already
/// be of its type and [checked](PropertyValue::checked), in every
/// representation that type gives, or removes it when `value` is `None`.
/// The document must be in the property's folder, as the schema's keys
/// hold. Every write of a value goes through here.
pub(crate) fn store_value(
    db: WriteContainer<'_>,
    property: &DocumentFolderProperty,
    document_id: i64,
    value: Option<PropertyValue>,
) -> Result<(), DatabaseError> {
    match value {
        None => {
            db.delete::<DocumentValue>()
                .filter_eq(DocumentValue::DOCUMENT_ID, document_id)
                .filter_eq(DocumentValue::PROPERTY_ID, property.id)
                .execute()?;
        }
        Some(value) => {
            debug_assert_eq!(value.data_type(), property.data_type());
            db.insert::<DocumentValue>()
                .set(DocumentValue::FOLDER_ID, property.folder_id)
                .set(DocumentValue::DOCUMENT_ID, document_id)
                .set(DocumentValue::PROPERTY_ID, property.id)
                .set(DocumentValue::TEXT_VALUE, value.text())
                .set(DocumentValue::INTEGER_VALUE, value.integer())
                .set(DocumentValue::REAL_VALUE, value.real())
                .set(DocumentValue::OPTION_ID, value.option_id())
                .on_conflict_update(&["document_id", "property_id"])
                .execute()?;
        }
    }
    Ok(())
}

/// Lets go of every value the documents of `folder_ids` hold, and of their
/// formula marks, before they leave those folders: a value belongs to its
/// document's folder, so a document can't leave with one. With
/// `document_id`, only that document's.
pub(crate) fn forget_values(
    db: WriteContainer<'_>,
    folder_ids: &[i64],
    document_id: Option<i64>,
) -> Result<(), DatabaseError> {
    for &folder_id in folder_ids {
        let params = [&folder_id.into_value(), &document_id.into_value()];
        db.container().execute_statement(
            "DELETE FROM document_values WHERE folder_id = ?1 AND (?2 IS NULL OR document_id = ?2)",
            params,
        )?;
        forget_stale_marks(db, folder_id, document_id)?;
    }
    Ok(())
}

/// The column of `document_values` a property of `data_type` filters on,
/// and the collation it compares by: text ignoring case, numbers as numbers,
/// a checkbox as 1 or 0, a date by its `YYYY-MM-DD`, a choice by its option.
fn compared_column(data_type: PropertyDataType) -> (&'static str, &'static str) {
    match data_type {
        PropertyDataType::Text => ("text_value", " COLLATE NOCASE"),
        PropertyDataType::Integer | PropertyDataType::Checkbox => ("integer_value", ""),
        PropertyDataType::Decimal | PropertyDataType::Currency => ("real_value", ""),
        PropertyDataType::Date => ("text_value", ""),
        PropertyDataType::Choice => ("option_id", ""),
    }
}

/// What a property of `data_type` sorts on, read from a row of
/// `document_values`, and its collation: its compared column, but a choice
/// by its option's place among the property's options.
fn sorted_column(data_type: PropertyDataType) -> (&'static str, &'static str) {
    match data_type {
        PropertyDataType::Choice => (
            "(SELECT sort_order FROM document_property_options \
             WHERE document_property_options.id = document_values.option_id)",
            "",
        ),
        other => compared_column(other),
    }
}

/// `value` as its type's column holds it, to compare against.
fn compared_value(value: &PropertyValue) -> Value {
    match value {
        PropertyValue::Text(text) | PropertyValue::Date(text) => text.clone().into_value(),
        PropertyValue::Integer(number) => number.into_value(),
        PropertyValue::Decimal(number) | PropertyValue::Currency(number) => number.into_value(),
        PropertyValue::Checkbox(checked) => i64::from(*checked).into_value(),
        PropertyValue::Choice(option_id) => option_id.into_value(),
    }
}

/// The property of `properties` with this id; another folder's, or none,
/// is not found.
fn property_of(
    properties: &[DocumentFolderProperty],
    property_id: i64,
) -> DatabaseCommandResult<&DocumentFolderProperty> {
    properties
        .iter()
        .find(|property| property.id == property_id)
        .ok_or(CommandError::NotFound(DocumentFolderProperty::NOUN))
}

/// A refusal of a filter, naming the field.
fn bad_filter(message: &'static str) -> CommandError {
    ValidationError::in_field("filters", message.to_string()).into()
}

/// `value`, a filter's, checked against a column of `data_type`.
fn filter_value(
    data_type: PropertyDataType,
    value: &PropertyValue,
) -> DatabaseCommandResult<Value> {
    if value.data_type() != data_type {
        return Err(bad_filter("A filter’s value must be of its column’s type."));
    }
    value.clone().checked().map_err(bad_filter)?;
    Ok(compared_value(value))
}

/// The refusal of a greater-or-less condition on a column of `data_type`
/// that doesn't order, if it doesn't.
fn ensure_ordered(data_type: PropertyDataType) -> DatabaseCommandResult<()> {
    if !data_type.is_ordered() {
        return Err(bad_filter(
            "Only numbers, amounts and dates compare as greater or less.",
        ));
    }
    Ok(())
}

/// Narrows `rows`, a select of one folder's documents, to the titles
/// meeting `condition`: a title is text, and never empty.
fn title_filtered<'c, O>(
    rows: Select<'c, Document, O>,
    condition: &TableCondition,
) -> DatabaseCommandResult<Select<'c, Document, O>> {
    let text = |value: &PropertyValue| filter_value(PropertyDataType::Text, value);
    Ok(match condition {
        TableCondition::IsEmpty => rows.filter("documents.title = ''"),
        TableCondition::IsNotEmpty => rows.filter("documents.title <> ''"),
        TableCondition::Equals { value } => rows
            .filter("documents.title = ? COLLATE NOCASE")
            .bind(text(value)?),
        TableCondition::NotEquals { value } => rows
            .filter("documents.title <> ? COLLATE NOCASE")
            .bind(text(value)?),
        TableCondition::Contains { text } => rows
            .filter("instr(lower(documents.title), lower(?)) > 0")
            .bind(text.clone()),
        TableCondition::NotContains { text } => rows
            .filter("instr(lower(documents.title), lower(?)) = 0")
            .bind(text.clone()),
        TableCondition::GreaterThan { .. }
        | TableCondition::GreaterOrEqual { .. }
        | TableCondition::LessThan { .. }
        | TableCondition::LessOrEqual { .. } => {
            return Err(ensure_ordered(PropertyDataType::Text).unwrap_err());
        }
    })
}

/// Narrows `rows`, a select of one folder's documents, to those meeting
/// every one of `filters`. A property's filter keeps the documents whose id
/// is (or isn't) among those of the matching values, so the work is SQL's;
/// the column and operator are the fixed text the property's type and the
/// condition pick, and the property id and value are bound.
fn filtered<'c, O>(
    db: impl ReadableDatabase<'c>,
    mut rows: Select<'c, Document, O>,
    properties: &[DocumentFolderProperty],
    filters: &[TableFilter],
) -> DatabaseCommandResult<Select<'c, Document, O>> {
    for filter in filters {
        let property_id = match filter.column {
            TableColumn::Title => {
                rows = title_filtered(rows, &filter.condition)?;
                continue;
            }
            TableColumn::Property { property_id } => property_id,
        };
        let property = property_of(properties, property_id)?;
        let (column, collation) = compared_column(property.data_type());
        let values = || {
            db.select_column::<DocumentValue, i64>("document_id")
                .filter_eq(DocumentValue::PROPERTY_ID, property.id)
        };
        let ordered = |operator: &str, value: &PropertyValue| {
            ensure_ordered(property.data_type())?;
            let bound = filter_value(property.data_type(), value)?;
            Ok::<_, CommandError>(
                values()
                    .filter(&format!("{column} {operator} ?"))
                    .bind(bound),
            )
        };
        let contains = |text: &str| {
            if property.data_type() != PropertyDataType::Text {
                return Err(bad_filter("Only text can contain text."));
            }
            Ok(values()
                .filter("instr(lower(text_value), lower(?)) > 0")
                .bind(text.to_string()))
        };
        let equal = |value: &PropertyValue| -> DatabaseCommandResult<_> {
            let bound = filter_value(property.data_type(), value)?;
            Ok(values()
                .filter(&format!("{column}{collation} = ?"))
                .bind(bound))
        };
        rows = match &filter.condition {
            TableCondition::IsEmpty => rows.filter_not_in("id", values()),
            TableCondition::IsNotEmpty => rows.filter_in("id", values()),
            TableCondition::Equals { value } => rows.filter_in("id", equal(value)?),
            TableCondition::NotEquals { value } => rows.filter_not_in("id", equal(value)?),
            TableCondition::Contains { text } => rows.filter_in("id", contains(text)?),
            TableCondition::NotContains { text } => rows.filter_not_in("id", contains(text)?),
            TableCondition::GreaterThan { value } => rows.filter_in("id", ordered(">", value)?),
            TableCondition::GreaterOrEqual { value } => rows.filter_in("id", ordered(">=", value)?),
            TableCondition::LessThan { value } => rows.filter_in("id", ordered("<", value)?),
            TableCondition::LessOrEqual { value } => rows.filter_in("id", ordered("<=", value)?),
        };
    }
    Ok(rows)
}

/// Orders `rows` by `sort`'s column, as its type compares, empty values
/// last either way, then by when each document was created; by that alone
/// with no sort.
fn sorted<'c>(
    rows: Select<'c, Document, RowTitle>,
    properties: &[DocumentFolderProperty],
    sort: Option<TableSort>,
) -> DatabaseCommandResult<Select<'c, Document, RowTitle>> {
    let Some(sort) = sort else {
        return Ok(rows.order("documents.id"));
    };
    let direction = if sort.descending { "DESC" } else { "ASC" };
    let property_id = match sort.column {
        TableColumn::Title => {
            return Ok(rows.order(&format!(
                "documents.title COLLATE NOCASE {direction}, documents.id"
            )));
        }
        TableColumn::Property { property_id } => property_id,
    };
    let property = property_of(properties, property_id)?;
    let (column, collation) = sorted_column(property.data_type());
    // A document's value for one property is one row of the values table,
    // read as a scalar; `filter_in` has no counterpart for an `ORDER BY`.
    Ok(rows.order_bound(
        &format!(
            "(SELECT {column} FROM document_values \
             WHERE document_values.document_id = documents.id \
             AND document_values.property_id = ?){collation} {direction} NULLS LAST, \
             documents.id"
        ),
        property.id,
    ))
}

/// `rows`, documents of folder `folder_id`, with their `values`, each row's
/// cells in the order of `properties`.
fn entries(
    folder_id: i64,
    rows: Vec<RowTitle>,
    values: Vec<DocumentValue>,
    properties: &[DocumentFolderProperty],
) -> Vec<TableRow> {
    let mut values: HashMap<(i64, i64), DocumentValue> = values
        .into_iter()
        .map(|value| ((value.document_id, value.property_id), value))
        .collect();
    rows.into_iter()
        .map(|row| TableRow {
            cells: properties
                .iter()
                .filter_map(|property| {
                    let stored = values.remove(&(row.id, property.id))?;
                    Some(DocumentCell {
                        property_id: property.id,
                        value: stored.value(property.data_type())?,
                    })
                })
                .collect(),
            document_id: row.id,
            folder_id,
            title: row.title,
        })
        .collect()
}

/// The live documents of folder `folder_id` in workspace `workspace_id`, as
/// a select of `P`.
fn documents_in<'c, P: Projection<Of = Document>>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
    folder_id: i64,
) -> Select<'c, Document, P> {
    db.select_live_as::<P>()
        .filter_eq(Document::WORKSPACE_ID, workspace_id)
        .filter_eq(Document::FOLDER_ID, Some(folder_id))
}

/// A folder's live documents as rows with their values: those meeting
/// every one of `filters`, ordered by `sort` or by when they were created.
/// Filtering and sorting happen in SQL; a filter or sort on another
/// folder's property is not found, and a condition the column's type can't
/// meet, or a value of another type, fails validation on `filters`. A
/// folder that isn't a table lists its documents with no cells.
#[derive(FromRequest)]
#[request(ListTableRowsRequest)]
pub struct ListTableRowsCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub filters: Vec<TableFilter>,
    pub sort: Option<TableSort>,
}
impl DatabaseCommand for ListTableRowsCommand {
    type Response = Vec<TableRow>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<TableRow>> {
        let properties = ListDocumentFolderPropertiesCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
        }
        .handle(ctx)?;
        let rows = documents_in::<RowTitle>(ctx.db, self.workspace_id, self.folder_id);
        let rows = filtered(ctx.db, rows, &properties, &self.filters)?;
        let rows = sorted(rows, &properties, self.sort)?.all()?;
        let ids = ctx
            .db
            .select_id::<Document>()
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::FOLDER_ID, Some(self.folder_id))
            .filter_eq(Document::DELETED, false);
        let ids = filtered(ctx.db, ids, &properties, &self.filters)?;
        let values = ctx
            .db
            .select::<DocumentValue>()
            .filter_eq(DocumentValue::FOLDER_ID, self.folder_id)
            .filter_in("document_id", ids)
            .all()?;
        Ok(entries(self.folder_id, rows, values, &properties))
    }
}

/// One live document of a folder as a row, with its values; one in another
/// folder is not found.
#[derive(FromRequest)]
#[request(GetTableRowRequest)]
pub struct GetTableRowCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub document_id: i64,
}
impl DatabaseCommand for GetTableRowCommand {
    type Response = TableRow;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<TableRow> {
        let properties = ListDocumentFolderPropertiesCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
        }
        .handle(ctx)?;
        let row = documents_in::<RowTitle>(ctx.db, self.workspace_id, self.folder_id)
            .filter_eq(Document::ID, self.document_id)
            .one()?
            .ok_or(CommandError::NotFound(Document::NOUN))?;
        let values = ctx
            .db
            .select::<DocumentValue>()
            .filter_eq(DocumentValue::DOCUMENT_ID, row.id)
            .all()?;
        Ok(entries(self.folder_id, vec![row], values, &properties).remove(0))
    }
}

/// Sets a live document's value for a property of its folder, or empties it
/// when `value` is `None`. The value must be of the property's type and
/// [checked](PropertyValue::checked); empty text empties it. A formula's
/// values are computed, never set; the document's formulas are computed
/// again.
#[derive(FromRequest)]
#[request(SetDocumentValueRequest)]
pub struct SetDocumentValueCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub document_id: i64,
    pub property_id: i64,
    pub value: Option<PropertyValue>,
}
impl DatabaseCommand for SetDocumentValueCommand {
    type Response = TableRow;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<TableRow>> {
        let property = GetDocumentFolderPropertyCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        if property.formula.is_some() {
            return Err(ValidationError::in_field(
                "property_id",
                "A formula’s values can’t be set. Change its formula instead.".to_string(),
            )
            .into());
        }
        let in_folder = documents_in::<RowTitle>(ctx.db, self.workspace_id, self.folder_id)
            .filter_eq(Document::ID, self.document_id)
            .exists()?;
        if !in_folder {
            return Err(CommandError::NotFound(Document::NOUN));
        }
        let value = match self.value {
            None => None,
            Some(value) if value.data_type() != property.data_type() => {
                return Err(ValidationError::in_field(
                    "value",
                    "The value must be of its property’s type.".to_string(),
                )
                .into());
            }
            Some(PropertyValue::Choice(option_id)) if property.option(option_id).is_none() => {
                return Err(ValidationError::in_field(
                    "value",
                    "The option isn’t one of this property’s.".to_string(),
                )
                .into());
            }
            Some(value) => value
                .checked()
                .map_err(|message| ValidationError::in_field("value", message.to_string()))?,
        };
        store_value(ctx.db, &property, self.document_id, value)?;
        formulas_stale(ctx, self.folder_id, Some(self.document_id))?;
        let row = GetTableRowCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
            document_id: self.document_id,
        }
        .handle(&mut ctx.as_read())?;
        Ok(row.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// The first `limit` live documents of folder `folder_id` in workspace
/// `workspace_id`, whose properties are `properties`, oldest first, as rows
/// with their values, and how many it holds in all: what its embed
/// previews.
pub(crate) fn first_rows<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
    folder_id: i64,
    properties: &[DocumentFolderProperty],
    limit: u64,
) -> Result<(Vec<TableRow>, i64), DatabaseError> {
    let rows: Vec<RowTitle> = documents_in(db, workspace_id, folder_id)
        .order("documents.id")
        .limit(limit)
        .all()?;
    let values = db
        .select::<DocumentValue>()
        .filter_eq(DocumentValue::FOLDER_ID, folder_id)
        .filter_in_values("document_id", rows.iter().map(|row| row.id))
        .all()?;
    let count = documents_in::<RowTitle>(db, workspace_id, folder_id).count()?;
    Ok((entries(folder_id, rows, values, properties), count))
}

#[cfg(test)]
mod tests;
