//! A folder's table as a write sees it: the values its documents hold, and
//! how far behind each document's formula outputs are
//! (`documents.formula_stale`). Every write to a table says what it changed
//! with [`table_changed`] or [`rows_left`] and gets the table's
//! invalidation key back, so the key and the formula marks can't come
//! apart; the SQL that touches the marks is all here. What computes the
//! outputs is in [`document_formula`](super::document_formula).

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{DocumentFolderProperty, DocumentValue, PropertyValue};
use cohesion_core::value::IntoValue;

use crate::{
    CommandContext, DatabaseError, DatabaseEventKind, Filtered, ReadableDatabase, WriteContainer,
};

/// What a write changed of a folder's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TableChange {
    /// What every document's formulas read: a property created, renamed,
    /// retyped, deleted or made a formula or not, a formula's source, or an
    /// option's name.
    Inputs,
    /// What this document's formulas read: a value of it, or its coming
    /// into the folder.
    RowInputs(i64),
    /// Only what the table shows, which no formula reads: a title, an
    /// order, a colour, a document deleted or restored, or stored outputs.
    Shown,
}

/// Records that a write changed folder `folder_id`'s table as `change`
/// says, and returns the key that write invalidates. A change to what
/// formulas read marks the documents whose outputs it outdates and raises
/// [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale) for them.
/// Every write to a table gets its key here.
pub(crate) fn table_changed(
    ctx: &mut CommandContext<'_>,
    folder_id: i64,
    change: TableChange,
) -> Result<Invalidation, DatabaseError> {
    match change {
        TableChange::Inputs => mark(ctx, folder_id, None)?,
        TableChange::RowInputs(document_id) => mark(ctx, folder_id, Some(document_id))?,
        TableChange::Shown => {}
    }
    Ok(Invalidation::DocumentTable(folder_id))
}

/// Lets go of every value the documents of `folder_ids` hold, and of their
/// formula marks, before they leave those folders, and returns the keys of
/// the tables they leave: a value belongs to its document's folder, so a
/// document can't leave with one. With `document_id`, only that document's.
pub(crate) fn rows_left(
    db: WriteContainer<'_>,
    folder_ids: &[i64],
    document_id: Option<i64>,
) -> Result<Vec<Invalidation>, DatabaseError> {
    for &folder_id in folder_ids {
        let params = [&folder_id.into_value(), &document_id.into_value()];
        db.container().execute_statement(
            "DELETE FROM document_values WHERE folder_id = ?1 AND (?2 IS NULL OR document_id = ?2)",
            params,
        )?;
        db.container().execute_statement(
            "UPDATE documents SET formula_stale = 0
             WHERE folder_id = ?1 AND (?2 IS NULL OR id = ?2) AND formula_stale > 0",
            params,
        )?;
    }
    Ok(folder_ids
        .iter()
        .copied()
        .map(Invalidation::DocumentTable)
        .collect())
}

/// Stores document `document_id`'s value for `property`, which must already
/// be of its type and [checked](PropertyValue::checked), in every
/// representation that type gives, or removes it when `value` is `None`.
/// The document must be in the property's folder, as the schema's keys
/// hold. Every write of a value goes through here; the write then says
/// what it changed with [`table_changed`].
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

/// Marks the formula outputs of folder `folder_id`'s documents (only
/// document `document_id`'s, when given) as out of date and raises
/// [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale) for them;
/// does nothing in a folder with no formula.
fn mark(
    ctx: &mut CommandContext<'_>,
    folder_id: i64,
    document_id: Option<i64>,
) -> Result<(), DatabaseError> {
    let marked = ctx.db.container().execute_statement(
        "UPDATE documents SET formula_stale = formula_stale + 1
         WHERE folder_id = ?1 AND (?2 IS NULL OR id = ?2)
         AND EXISTS (SELECT 1 FROM document_folder_properties
             WHERE folder_id = ?1 AND formula_source IS NOT NULL)",
        [&folder_id.into_value(), &document_id.into_value()],
    )?;
    raise(ctx, folder_id, marked);
    Ok(())
}

/// Raises [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale) for
/// folder `folder_id` when a write marked any of its documents.
fn raise(ctx: &mut CommandContext<'_>, folder_id: i64, marked: usize) {
    if marked > 0 {
        ctx.events.send(
            ctx.trace
                .event(DatabaseEventKind::TableFormulasStale { folder_id }),
        );
    }
}

/// Has every document of folder `folder_id` computed again without
/// invalidating its table: for a pass that left an error it couldn't
/// confirm, which changed nothing a client shows.
pub(crate) fn compute_again(
    ctx: &mut CommandContext<'_>,
    folder_id: i64,
) -> Result<(), DatabaseError> {
    mark(ctx, folder_id, None)
}

/// Marks every live document of folder `folder_id` as out of date, raising
/// the event for them, and says whether there were any: the hourly mark of
/// a folder with a continuous formula. A deleted document is left as it is.
pub(crate) fn compute_live_rows_again(
    ctx: &mut CommandContext<'_>,
    folder_id: i64,
) -> Result<bool, DatabaseError> {
    let marked = ctx.db.container().execute_statement(
        "UPDATE documents SET formula_stale = formula_stale + 1
         WHERE folder_id = ? AND deleted = FALSE",
        [&folder_id.into_value()],
    )?;
    raise(ctx, folder_id, marked);
    Ok(marked > 0)
}

/// Clears document `document_id`'s mark once its outputs are stored, unless
/// a change raised it past `formula_stale`, what the pass read, meanwhile.
pub(crate) fn outputs_stored(
    db: WriteContainer<'_>,
    folder_id: i64,
    document_id: i64,
    formula_stale: i64,
) -> Result<(), DatabaseError> {
    db.container().execute_statement(
        "UPDATE documents SET formula_stale = 0
         WHERE id = ? AND folder_id = ? AND formula_stale = ?",
        [
            &document_id.into_value(),
            &folder_id.into_value(),
            &formula_stale.into_value(),
        ],
    )?;
    Ok(())
}
