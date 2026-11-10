//! A database's rows and their values: storing a value in every
//! representation its type gives, listing rows filtered and sorted in SQL,
//! and a row's own document.

use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DatabaseCell, DatabaseDataType, DatabaseProperty, DatabaseRow, DatabaseRowEntry,
    DatabaseRowValue, DatabaseValue, LinkKind, LinkTarget,
};
use cohesion_core::protocol::{
    CreateDatabaseRowDocumentRequest, CreateDatabaseRowRequest, DatabaseCondition, DatabaseFilter,
    DatabaseSort, DeleteDatabaseRowRequest, GetDatabaseRowRequest, ListDatabaseRowsRequest,
    SetDatabaseValueRequest,
};
use cohesion_core::{Document, DocumentType, FromRequest, Named, Value};

use crate::changed::{Changed, Invalidates};
use crate::commands::backlink::FollowLinksCommand;
use crate::commands::database::{
    GetDatabaseCommand, GetDatabasePropertyCommand, ListDatabasePropertiesCommand,
};
use crate::commands::database_formula::formulas_stale;
use crate::commands::document::changed;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered,
    Found, Read, ReadableDatabase, Select, ValidationError, Write, WriteContainer,
};

/// Stores row `row_id`'s value for `property`, which must already be of
/// its type and [checked](DatabaseValue::checked), in every representation
/// that type gives, or removes it when `value` is `None`. Every write of a
/// value goes through here.
pub(crate) fn store_value(
    db: WriteContainer<'_>,
    property: &DatabaseProperty,
    row_id: i64,
    value: Option<DatabaseValue>,
) -> Result<(), DatabaseError> {
    match value {
        None => {
            db.delete::<DatabaseRowValue>()
                .filter_eq(DatabaseRowValue::ROW_ID, row_id)
                .filter_eq(DatabaseRowValue::PROPERTY_ID, property.id)
                .execute()?;
        }
        Some(value) => {
            debug_assert_eq!(value.data_type(), property.data_type);
            db.insert::<DatabaseRowValue>()
                .set(DatabaseRowValue::DATABASE_ID, property.database_id)
                .set(DatabaseRowValue::ROW_ID, row_id)
                .set(DatabaseRowValue::PROPERTY_ID, property.id)
                .set(DatabaseRowValue::TEXT_VALUE, value.text())
                .set(DatabaseRowValue::INTEGER_VALUE, value.integer())
                .set(DatabaseRowValue::REAL_VALUE, value.real())
                .on_conflict_update(&["row_id", "property_id"])
                .execute()?;
        }
    }
    Ok(())
}

/// The column of `database_row_values` a property of `data_type` filters
/// and sorts on, and the collation it compares by: text ignoring case,
/// numbers as numbers, a checkbox as 1 or 0, a date by its `YYYY-MM-DD`.
fn compared_column(data_type: DatabaseDataType) -> (&'static str, &'static str) {
    match data_type {
        DatabaseDataType::Text => ("text_value", " COLLATE NOCASE"),
        DatabaseDataType::Integer | DatabaseDataType::Checkbox => ("integer_value", ""),
        DatabaseDataType::Decimal | DatabaseDataType::Currency => ("real_value", ""),
        DatabaseDataType::Date => ("text_value", ""),
    }
}

/// `value` as its type's column holds it, to compare against.
fn compared_value(value: &DatabaseValue) -> Value {
    use cohesion_core::value::IntoValue;
    match value {
        DatabaseValue::Text(text) | DatabaseValue::Date(text) => text.clone().into_value(),
        DatabaseValue::Integer(number) => number.into_value(),
        DatabaseValue::Decimal(number) | DatabaseValue::Currency(number) => number.into_value(),
        DatabaseValue::Checkbox(checked) => i64::from(*checked).into_value(),
    }
}

/// The property of `properties` with this id; another database's, or none,
/// is not found.
fn property_of(
    properties: &[DatabaseProperty],
    property_id: i64,
) -> DatabaseCommandResult<&DatabaseProperty> {
    properties
        .iter()
        .find(|property| property.id == property_id)
        .ok_or(CommandError::NotFound(DatabaseProperty::NOUN))
}

/// A refusal of a filter, naming the field.
fn bad_filter(message: &'static str) -> CommandError {
    ValidationError::in_field("filters", message.to_string()).into()
}

/// `value`, a filter's, checked against `property`'s type.
fn filter_value(
    property: &DatabaseProperty,
    value: &DatabaseValue,
) -> DatabaseCommandResult<Value> {
    if value.data_type() != property.data_type {
        return Err(bad_filter(
            "A filter’s value must be of its property’s type.",
        ));
    }
    value.clone().checked().map_err(bad_filter)?;
    Ok(compared_value(value))
}

/// Narrows `rows`, a select of one database's rows, to those meeting every
/// one of `filters`. Each filter keeps the rows whose id is (or isn't) among
/// those of the matching values, so the work is SQL's; the column and
/// operator are the fixed text the property's type and the condition pick,
/// and the property id and value are bound.
fn filtered<'c, O>(
    db: impl ReadableDatabase<'c>,
    mut rows: Select<'c, DatabaseRow, O>,
    properties: &[DatabaseProperty],
    filters: &[DatabaseFilter],
) -> DatabaseCommandResult<Select<'c, DatabaseRow, O>> {
    for filter in filters {
        let property = property_of(properties, filter.property_id)?;
        let (column, collation) = compared_column(property.data_type);
        let values = || {
            db.select_column::<DatabaseRowValue, i64>("row_id")
                .filter_eq(DatabaseRowValue::PROPERTY_ID, property.id)
        };
        let ordered = |operator: &str, value: &DatabaseValue| {
            if !property.data_type.is_ordered() {
                return Err(bad_filter(
                    "Only numbers, amounts and dates compare as greater or less.",
                ));
            }
            let bound = filter_value(property, value)?;
            Ok(values()
                .filter(&format!("{column} {operator} ?"))
                .bind(bound))
        };
        let contains = |text: &str| {
            if property.data_type != DatabaseDataType::Text {
                return Err(bad_filter("Only text can contain text."));
            }
            Ok(values()
                .filter("instr(lower(text_value), lower(?)) > 0")
                .bind(text.to_string()))
        };
        let equal = |value: &DatabaseValue| -> DatabaseCommandResult<_> {
            let bound = filter_value(property, value)?;
            Ok(values()
                .filter(&format!("{column}{collation} = ?"))
                .bind(bound))
        };
        rows = match &filter.condition {
            DatabaseCondition::IsEmpty => rows.filter_not_in("id", values()),
            DatabaseCondition::IsNotEmpty => rows.filter_in("id", values()),
            DatabaseCondition::Equals { value } => rows.filter_in("id", equal(value)?),
            DatabaseCondition::NotEquals { value } => rows.filter_not_in("id", equal(value)?),
            DatabaseCondition::Contains { text } => rows.filter_in("id", contains(text)?),
            DatabaseCondition::NotContains { text } => rows.filter_not_in("id", contains(text)?),
            DatabaseCondition::GreaterThan { value } => rows.filter_in("id", ordered(">", value)?),
            DatabaseCondition::GreaterOrEqual { value } => {
                rows.filter_in("id", ordered(">=", value)?)
            }
            DatabaseCondition::LessThan { value } => rows.filter_in("id", ordered("<", value)?),
            DatabaseCondition::LessOrEqual { value } => rows.filter_in("id", ordered("<=", value)?),
        };
    }
    Ok(rows)
}

/// Orders `rows` by `sort`'s property, as its type compares, empty values
/// last either way, then by when each row was added; by that alone with no
/// sort.
fn sorted<'c>(
    rows: Select<'c, DatabaseRow>,
    properties: &[DatabaseProperty],
    sort: Option<DatabaseSort>,
) -> DatabaseCommandResult<Select<'c, DatabaseRow>> {
    let Some(sort) = sort else {
        return Ok(rows.order("database_rows.id"));
    };
    let property = property_of(properties, sort.property_id)?;
    let (column, collation) = compared_column(property.data_type);
    let direction = if sort.descending { "DESC" } else { "ASC" };
    // A row's value for one property is one row of the values table, read
    // as a scalar; `filter_in` has no counterpart for an `ORDER BY`.
    Ok(rows.order_bound(
        &format!(
            "(SELECT {column} FROM database_row_values \
             WHERE database_row_values.row_id = database_rows.id \
             AND database_row_values.property_id = ?){collation} {direction} NULLS LAST, \
             database_rows.id"
        ),
        property.id,
    ))
}

/// `rows` with their `values`, each row's cells in the order of
/// `properties`.
fn entries(
    rows: Vec<DatabaseRow>,
    values: Vec<DatabaseRowValue>,
    properties: &[DatabaseProperty],
) -> Vec<DatabaseRowEntry> {
    let mut values: HashMap<(i64, i64), DatabaseRowValue> = values
        .into_iter()
        .map(|value| ((value.row_id, value.property_id), value))
        .collect();
    rows.into_iter()
        .map(|row| DatabaseRowEntry {
            cells: properties
                .iter()
                .filter_map(|property| {
                    let stored = values.remove(&(row.id, property.id))?;
                    Some(DatabaseCell {
                        property_id: property.id,
                        value: stored.value(property.data_type)?,
                    })
                })
                .collect(),
            id: row.id,
            database_id: row.database_id,
            document_id: row.document_id,
        })
        .collect()
}

/// A database's rows with their values: those meeting every one of
/// `filters`, ordered by `sort` or by when they were added. Filtering and
/// sorting happen in SQL; a filter or sort on another database's property
/// is not found, and a condition the property's type can't meet, or a value
/// of another type, fails validation on `filters`.
#[derive(FromRequest)]
#[request(ListDatabaseRowsRequest)]
pub struct ListDatabaseRowsCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub filters: Vec<DatabaseFilter>,
    pub sort: Option<DatabaseSort>,
}
impl DatabaseCommand for ListDatabaseRowsCommand {
    type Response = Vec<DatabaseRowEntry>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<DatabaseRowEntry>> {
        let properties = ListDatabasePropertiesCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(ctx)?;
        let rows = ctx
            .db
            .select::<DatabaseRow>()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id);
        let rows = filtered(ctx.db, rows, &properties, &self.filters)?;
        let rows = sorted(rows, &properties, self.sort)?.all()?;
        let ids = ctx
            .db
            .select_id::<DatabaseRow>()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id);
        let ids = filtered(ctx.db, ids, &properties, &self.filters)?;
        let values = ctx
            .db
            .select::<DatabaseRowValue>()
            .filter_eq(DatabaseRowValue::DATABASE_ID, self.database_id)
            .filter_in("row_id", ids)
            .all()?;
        Ok(entries(rows, values, &properties))
    }
}

/// One row of a database with its values.
#[derive(FromRequest)]
#[request(GetDatabaseRowRequest)]
pub struct GetDatabaseRowCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}
impl DatabaseCommand for GetDatabaseRowCommand {
    type Response = DatabaseRowEntry;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<DatabaseRowEntry> {
        let properties = ListDatabasePropertiesCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(ctx)?;
        let row: DatabaseRow = ctx
            .db
            .select()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseRow::ID, self.row_id)
            .one()
            .found()?;
        let values = ctx
            .db
            .select::<DatabaseRowValue>()
            .filter_eq(DatabaseRowValue::ROW_ID, row.id)
            .all()?;
        Ok(entries(vec![row], values, &properties).remove(0))
    }
}

/// Adds a row with no values to a database.
#[derive(FromRequest)]
#[request(CreateDatabaseRowRequest)]
pub struct CreateDatabaseRowCommand {
    pub workspace_id: i64,
    pub database_id: i64,
}
impl DatabaseCommand for CreateDatabaseRowCommand {
    type Response = DatabaseRowEntry;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseRowEntry>> {
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(&mut ctx.as_read())?;
        let row = ctx
            .db
            .insert::<DatabaseRow>()
            .set(DatabaseRow::DATABASE_ID, self.database_id)
            .returning_one()
            .found()?;
        formulas_stale(ctx, self.database_id, Some(row.id))?;
        let entry = DatabaseRowEntry {
            id: row.id,
            database_id: row.database_id,
            document_id: row.document_id,
            cells: Vec::new(),
        };
        Ok(entry.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Deletes a row for good. Its values go with it by the schema's cascade,
/// and its document by the trigger that removes a row's document with it,
/// after the document's links are let go ([`forget_document_links`]).
#[derive(FromRequest)]
#[request(DeleteDatabaseRowRequest)]
pub struct DeleteDatabaseRowCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}
impl DatabaseCommand for DeleteDatabaseRowCommand {
    type Response = DatabaseRow;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<DatabaseRow>> {
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(&mut ctx.as_read())?;
        let rows = ctx
            .db
            .select::<DatabaseRow>()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseRow::ID, self.row_id);
        forget_document_links(ctx, rows)?;
        let row: DatabaseRow = ctx
            .db
            .delete()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseRow::ID, self.row_id)
            .returning_one()
            .found()?;
        let documents = row
            .document_id
            .map(|_| Invalidation::Documents(self.workspace_id));
        Ok(row
            .invalidating([Invalidation::Database(self.database_id)])
            .and(documents))
    }
}

/// Sets one value of a row, or empties it when `value` is `None`. The value
/// must be of the property's type and [checked](DatabaseValue::checked);
/// empty text empties it. A formula's values are computed, never set; the
/// row's formulas are computed again.
#[derive(FromRequest)]
#[request(SetDatabaseValueRequest)]
pub struct SetDatabaseValueCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
    pub property_id: i64,
    pub value: Option<DatabaseValue>,
}
impl DatabaseCommand for SetDatabaseValueCommand {
    type Response = DatabaseRowEntry;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseRowEntry>> {
        let property = GetDatabasePropertyCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
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
        let _: DatabaseRow = ctx
            .db
            .select()
            .filter_eq(DatabaseRow::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseRow::ID, self.row_id)
            .one()
            .found()?;
        let value = match self.value {
            None => None,
            Some(value) if value.data_type() != property.data_type => {
                return Err(ValidationError::in_field(
                    "value",
                    "The value must be of its property’s type.".to_string(),
                )
                .into());
            }
            Some(value) => value
                .checked()
                .map_err(|message| ValidationError::in_field("value", message.to_string()))?,
        };
        store_value(ctx.db, &property, self.row_id, value)?;
        formulas_stale(ctx, self.database_id, Some(self.row_id))?;
        let entry = GetDatabaseRowCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            row_id: self.row_id,
        }
        .handle(&mut ctx.as_read())?;
        Ok(entry.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Gives a row an empty, untitled document of its own, of type
/// `databaseRow`, in the database's workspace; refused when the row has one.
/// The document stays out of the document list and search, and goes with
/// the row.
#[derive(FromRequest)]
#[request(CreateDatabaseRowDocumentRequest)]
pub struct CreateDatabaseRowDocumentCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub row_id: i64,
}
impl DatabaseCommand for CreateDatabaseRowDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        let row = GetDatabaseRowCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            row_id: self.row_id,
        }
        .handle(&mut ctx.as_read())?;
        if row.document_id.is_some() {
            return Err(ValidationError::new("This row already has a document.").into());
        }
        let document = ctx
            .db
            .insert::<Document>()
            .set(Document::WORKSPACE_ID, self.workspace_id)
            .set(Document::TITLE, "")
            .set(Document::CONTENTS, "")
            .set(Document::DOCUMENT_TYPE, DocumentType::DatabaseRow)
            .returning_one()
            .found()?;
        ctx.db
            .update::<DatabaseRow>()
            .set(DatabaseRow::DOCUMENT_ID, Some(document.id))
            .filter_eq(DatabaseRow::ID, row.id)
            .execute()?;
        Ok(changed(ctx, document)?.and([Invalidation::Database(self.database_id)]))
    }
}

/// Lets go of what the documents of `rows`, about to be removed with their
/// rows by the schema's trigger, link to ([`FollowLinksCommand`]), so the
/// backlinks of each item they linked to are invalidated.
pub(crate) fn forget_document_links(
    ctx: &mut CommandContext<'_>,
    rows: Select<'_, DatabaseRow>,
) -> DatabaseCommandResult<()> {
    for row in rows.all()? {
        if let Some(id) = row.document_id {
            ctx.run(FollowLinksCommand {
                source: LinkTarget {
                    kind: LinkKind::Document,
                    id,
                },
                text: None,
            })?;
        }
    }
    Ok(())
}

/// The first `limit` rows of database `database_id`, whose properties are
/// `properties`, in the order they were added, with their values, and how
/// many rows it has in all: what its embed previews.
pub(crate) fn first_rows<'c>(
    db: impl ReadableDatabase<'c>,
    database_id: i64,
    properties: &[DatabaseProperty],
    limit: u64,
) -> Result<(Vec<DatabaseRowEntry>, i64), DatabaseError> {
    let rows: Vec<DatabaseRow> = db
        .select()
        .filter_eq(DatabaseRow::DATABASE_ID, database_id)
        .order("id")
        .limit(limit)
        .all()?;
    let values = db
        .select::<DatabaseRowValue>()
        .filter_eq(DatabaseRowValue::DATABASE_ID, database_id)
        .filter_in_values("row_id", rows.iter().map(|row| row.id))
        .all()?;
    let count = db
        .select::<DatabaseRow>()
        .filter_eq(DatabaseRow::DATABASE_ID, database_id)
        .count()?;
    Ok((entries(rows, values, properties), count))
}

/// The row whose document is `document_id`, if it is a row's.
pub(crate) fn row_of_document<'c>(
    db: impl ReadableDatabase<'c>,
    document_id: i64,
) -> Result<Option<DatabaseRow>, DatabaseError> {
    db.select()
        .filter_eq(DatabaseRow::DOCUMENT_ID, Some(document_id))
        .one()
}

#[cfg(test)]
mod tests;
