//! Formula properties: a property of a folder whose value in each of its
//! documents a JavaScript function of the row computes ([`PropertyFormula`]).
//! This crate can't run JavaScript, so every change to what a formula reads
//! marks the documents whose outputs it outdates (`documents.formula_stale`)
//! and raises [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale);
//! the daemon reads those documents with [`ListStaleTableRowsCommand`],
//! computes the outputs, and stores them with
//! [`StoreTableFormulaOutputsCommand`], as values like any other, so
//! filtering and sorting read them in SQL. A document stays marked until its
//! outputs are stored, or it leaves the folder, so a restart computes what
//! was left ([`ListTablesWithStaleFormulasCommand`]). A formula whose
//! update mode is `continuously` is computed every hour as well: the daemon
//! marks its folder's live documents with
//! [`MarkContinuousFormulasStaleCommand`].
//!
//! A formula reads the document's values of the properties that aren't
//! formulas, by name, and nothing else of it, so formulas never depend on
//! each other and their order doesn't matter. A deleted document in the
//! folder keeps its values, so it is computed too.

use std::collections::{HashMap, HashSet};

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DocumentFolderProperty, DocumentValue, PropertyDataType, PropertyFormula, PropertyValue,
};
use cohesion_core::protocol::{FormulaDefinition, SetDocumentFolderPropertyFormulaRequest};
use cohesion_core::value::IntoValue;
use cohesion_core::{FromRequest, Model, Projection};

use crate::changed::{Changed, Invalidates};
use crate::commands::document_folder_property::GetDocumentFolderPropertyCommand;
use crate::commands::document_value::store_value;
use crate::commands::property_option::ensure_not_formula_choice;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, DatabaseEventKind,
    Filtered, Read, ReadableDatabase, ValidationError, Write,
};

/// A document with how many changes its formula outputs are behind (0 when
/// they are current, or it is in no table), which [`Document`] leaves out:
/// clients never see it.
///
/// [`Document`]: cohesion_core::Document
#[derive(Model)]
#[model(table = "documents", backup = Included)]
#[allow(dead_code)] // Written by `formulas_stale`; read through `RowStaleness`.
pub struct StaleMarkedDocument {
    id: i64,
    folder_id: Option<i64>,
    formula_stale: i64,
}

/// A document's id and its mark, as a pass reads it: the outputs it stores
/// clear the mark only if no change raised it again meanwhile.
#[derive(Projection, Clone, Copy, Debug, PartialEq)]
#[projection(of = StaleMarkedDocument)]
pub struct RowStaleness {
    pub id: i64,
    pub formula_stale: i64,
}

/// Marks the formula outputs of folder `folder_id`'s documents (only
/// document `document_id`'s, when given) as out of date and raises
/// [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale) for them;
/// does nothing in a folder with no formula. Every change to what a formula
/// reads calls this: a value, a document coming into the folder, a
/// property, or a source.
pub(crate) fn formulas_stale(
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
    if marked > 0 {
        ctx.events.send(
            ctx.trace
                .event(DatabaseEventKind::TableFormulasStale { folder_id }),
        );
    }
    Ok(())
}

/// A formula as a request carries it: absent, or with a source that isn't
/// empty.
pub(crate) fn ensure_formula(formula: &Option<FormulaDefinition>) -> Result<(), ValidationError> {
    if formula
        .as_ref()
        .is_some_and(|formula| formula.source.trim().is_empty())
    {
        return Err(ValidationError::in_field(
            "formula",
            "A formula can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// `formula` as a new formula, with no error until it is computed.
pub(crate) fn new_formula(formula: Option<FormulaDefinition>) -> Option<PropertyFormula> {
    formula.map(|formula| PropertyFormula {
        source: formula.source,
        update_mode: formula.update_mode,
        error: None,
    })
}

/// Makes a property a formula with a non-empty source, changes its source
/// or update mode, or makes it a property set by hand again. A property
/// becoming a formula loses the values set by hand; one that stops being a
/// formula keeps its last outputs as values. A new source drops the old
/// one's error and has every document computed again; a formula whose
/// source stays keeps its outputs and error, and only takes the new mode.
/// A choice property can't become a formula.
#[derive(FromRequest)]
#[request(SetDocumentFolderPropertyFormulaRequest)]
pub struct SetDocumentFolderPropertyFormulaCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub formula: Option<FormulaDefinition>,
}
impl DatabaseCommand for SetDocumentFolderPropertyFormulaCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        ensure_formula(&self.formula)?;
        let before = GetDocumentFolderPropertyCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        ensure_not_formula_choice(before.data_type(), self.formula.is_some())?;
        let formula = match (before.formula, self.formula) {
            (Some(before), Some(after)) if before.source == after.source => {
                // Only the mode can differ: what is stored still holds.
                Some(PropertyFormula {
                    update_mode: after.update_mode,
                    ..before
                })
            }
            (before, after) => {
                if before.is_none() && after.is_some() {
                    ctx.db
                        .delete::<DocumentValue>()
                        .filter_eq(DocumentValue::PROPERTY_ID, self.property_id)
                        .execute()?;
                }
                formulas_stale(ctx, self.folder_id, None)?;
                new_formula(after)
            }
        };
        let property = ctx
            .db
            .update::<DocumentFolderProperty>()
            .set_group(formula)
            .filter_eq(DocumentFolderProperty::ID, self.property_id)
            .returning_one()?
            .expect("the property was just read");
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// One document whose formula outputs are out of date, with what a formula
/// reads of it.
#[derive(Clone, Debug)]
pub struct StaleTableRow {
    pub staleness: RowStaleness,
    /// Each property that isn't a formula, by name, with the document's value
    /// for it or `None` when empty; a choice as its option's name, as text.
    /// Of two properties with one name, the one created first is read.
    pub inputs: Vec<(String, Option<PropertyValue>)>,
}

/// A formula property as a pass computes it.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaProperty {
    pub property_id: i64,
    pub source: String,
    /// The type its outputs must be.
    pub data_type: PropertyDataType,
}

/// What a folder's formulas need to bring its outdated documents up to date.
#[derive(Debug)]
pub struct StaleTableRows {
    pub formulas: Vec<FormulaProperty>,
    pub rows: Vec<StaleTableRow>,
}

/// The documents of a folder whose formula outputs are out of date, by
/// id, with what its formulas read of each; nothing for a folder that is
/// gone or holds none.
pub struct ListStaleTableRowsCommand {
    pub folder_id: i64,
}
impl DatabaseCommand for ListStaleTableRowsCommand {
    type Response = StaleTableRows;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<StaleTableRows> {
        let properties: Vec<DocumentFolderProperty> = ctx
            .db
            .select()
            .filter_eq(DocumentFolderProperty::FOLDER_ID, self.folder_id)
            .order("id")
            .all()?;
        let stale: Vec<RowStaleness> = ctx
            .db
            .select_as::<RowStaleness>()
            .filter_eq(StaleMarkedDocument::FOLDER_ID, Some(self.folder_id))
            .filter("formula_stale > 0")
            .order("documents.id")
            .all()?;
        let stale_ids = ctx
            .db
            .select_id::<StaleMarkedDocument>()
            .filter_eq(StaleMarkedDocument::FOLDER_ID, Some(self.folder_id))
            .filter("formula_stale > 0");
        let mut values: HashMap<(i64, i64), DocumentValue> = ctx
            .db
            .select::<DocumentValue>()
            .filter_eq(DocumentValue::FOLDER_ID, self.folder_id)
            .filter_in("document_id", stale_ids)
            .all()?
            .into_iter()
            .map(|value| ((value.document_id, value.property_id), value))
            .collect();
        let mut formulas = Vec::new();
        let mut inputs = Vec::new();
        for property in properties {
            let data_type = property.data_type();
            match property.formula {
                Some(PropertyFormula { source, .. }) => formulas.push(FormulaProperty {
                    property_id: property.id,
                    source,
                    data_type,
                }),
                None => inputs.push(property),
            }
        }
        let mut names = HashSet::new();
        let inputs: Vec<DocumentFolderProperty> = inputs
            .into_iter()
            .filter(|property| names.insert(property.name.clone()))
            .collect();
        let rows = stale
            .into_iter()
            .map(|staleness| StaleTableRow {
                staleness,
                inputs: inputs
                    .iter()
                    .map(|property| {
                        let value = values
                            .remove(&(staleness.id, property.id))
                            .and_then(|stored| stored.value(property.data_type()))
                            .and_then(|value| match value {
                                PropertyValue::Choice(option_id) => property
                                    .option(option_id)
                                    .map(|option| PropertyValue::Text(option.name.clone())),
                                value => Some(value),
                            });
                        (property.name.clone(), value)
                    })
                    .collect(),
            })
            .collect();
        Ok(StaleTableRows { formulas, rows })
    }
}

/// What one formula computed for the documents of a pass, in the pass's
/// order: each one's output, empty or of the formula's type, or why it failed.
#[derive(Debug)]
pub struct FormulaOutputs {
    /// The formula as computed; outputs of a source or type the property no
    /// longer has are dropped.
    pub formula: FormulaProperty,
    pub outputs: Vec<Result<Option<PropertyValue>, String>>,
}

/// Stores what a pass computed for `rows`, documents of a folder read by
/// [`ListStaleTableRowsCommand`], and clears each one's mark unless a
/// change raised it again meanwhile, whose own pass follows. Outputs of a
/// formula that changed or went since are dropped, as are those of a
/// document that left the folder or went.
///
/// A formula's error is the first failure of the pass; a pass without one
/// clears it if it covered every document, and otherwise marks every one so
/// the pass that follows finds whether another still fails.
///
/// Only a value or an error that differs from what was stored is written,
/// and the table is invalidated only when one did, so an hourly pass that
/// computes what was already there tells no client to refetch.
pub struct StoreTableFormulaOutputsCommand {
    pub folder_id: i64,
    pub rows: Vec<RowStaleness>,
    pub formulas: Vec<FormulaOutputs>,
}
impl DatabaseCommand for StoreTableFormulaOutputsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let current: HashSet<i64> = ctx
            .db
            .select_id::<StaleMarkedDocument>()
            .filter_eq(StaleMarkedDocument::FOLDER_ID, Some(self.folder_id))
            .all()?
            .into_iter()
            .collect();
        let covered: HashSet<i64> = self.rows.iter().map(|row| row.id).collect();
        let every_row = current.is_subset(&covered);
        let mut recheck = false;
        let mut changed = false;
        for computed in self.formulas {
            let property: Option<DocumentFolderProperty> = ctx
                .db
                .select()
                .filter_eq(DocumentFolderProperty::FOLDER_ID, self.folder_id)
                .filter_eq(DocumentFolderProperty::ID, computed.formula.property_id)
                .one()?;
            let Some(property) = property.filter(|property| {
                property.data_type() == computed.formula.data_type
                    && property
                        .formula
                        .as_ref()
                        .is_some_and(|formula| formula.source == computed.formula.source)
            }) else {
                continue;
            };
            let stored: HashMap<i64, DocumentValue> = ctx
                .db
                .select::<DocumentValue>()
                .filter_eq(DocumentValue::PROPERTY_ID, property.id)
                .all()?
                .into_iter()
                .map(|value| (value.document_id, value))
                .collect();
            let mut failure = None;
            for (row, output) in self.rows.iter().zip(computed.outputs) {
                if !current.contains(&row.id) {
                    continue;
                }
                let value = output.unwrap_or_else(|error| {
                    failure.get_or_insert(error);
                    None
                });
                let before = stored.get(&row.id);
                let same = match &value {
                    None => before.is_none(),
                    Some(value) => before
                        .and_then(|stored| stored.value(property.data_type()))
                        .is_some_and(|stored| stored == *value),
                };
                if !same {
                    store_value(ctx.db, &property, row.id, value)?;
                    changed = true;
                }
            }
            let Some(formula) = property.formula else {
                continue;
            };
            let error = match failure {
                Some(error) => Some(error),
                None if formula.error.is_some() && !every_row => {
                    recheck = true;
                    continue;
                }
                None => None,
            };
            if error != formula.error {
                ctx.db
                    .update::<DocumentFolderProperty>()
                    .set_group(Some(PropertyFormula { error, ..formula }))
                    .filter_eq(DocumentFolderProperty::ID, property.id)
                    .execute()?;
                changed = true;
            }
        }
        for row in &self.rows {
            ctx.db.container().execute_statement(
                "UPDATE documents SET formula_stale = 0
                 WHERE id = ? AND folder_id = ? AND formula_stale = ?",
                [
                    &row.id.into_value(),
                    &self.folder_id.into_value(),
                    &row.formula_stale.into_value(),
                ],
            )?;
        }
        if recheck {
            formulas_stale(ctx, self.folder_id, None)?;
        }
        Ok(if changed {
            ().invalidating([Invalidation::DocumentTable(self.folder_id)])
        } else {
            ().invalidating_nothing()
        })
    }
}

/// Marks every live document of each folder with a formula whose update
/// mode is `continuously` as out of date, and raises
/// [`TableFormulasStale`](DatabaseEventKind::TableFormulasStale) for each
/// folder it marked, so the daemon's pass computes them again; the daemon
/// runs it every hour. Returns those folders. A deleted document is left
/// as it is.
pub struct MarkContinuousFormulasStaleCommand;
impl DatabaseCommand for MarkContinuousFormulasStaleCommand {
    type Response = Vec<i64>;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Vec<i64>>> {
        let folders: Vec<i64> = ctx
            .db
            .select_column::<DocumentFolderProperty, i64>("folder_id")
            .filter("formula_update_mode = 'continuously'")
            .distinct()
            .order("folder_id")
            .all()?;
        let mut marked = Vec::new();
        for folder_id in folders {
            let count = ctx.db.container().execute_statement(
                "UPDATE documents SET formula_stale = formula_stale + 1
                 WHERE folder_id = ? AND deleted = FALSE",
                [&folder_id.into_value()],
            )?;
            if count > 0 {
                ctx.events.send(
                    ctx.trace
                        .event(DatabaseEventKind::TableFormulasStale { folder_id }),
                );
                marked.push(folder_id);
            }
        }
        Ok(marked.invalidating_nothing())
    }
}

/// The folders with documents whose formula outputs are out of date, for
/// the daemon to bring up to date as it starts.
pub struct ListTablesWithStaleFormulasCommand;
impl DatabaseCommand for ListTablesWithStaleFormulasCommand {
    type Response = Vec<i64>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<i64>> {
        Ok(ctx
            .db
            .select_column::<StaleMarkedDocument, i64>("folder_id")
            .filter("formula_stale > 0 AND folder_id IS NOT NULL")
            .distinct()
            .order("folder_id")
            .all()?)
    }
}

#[cfg(test)]
mod tests;
