//! Formula properties: a property whose value in each row a JavaScript
//! function of the row computes ([`DatabaseFormula`]). This crate can't run
//! JavaScript, so every change to what a formula reads marks the rows whose
//! outputs it outdates (`formula_stale`) and raises
//! [`DatabaseFormulasStale`](DatabaseEventKind::DatabaseFormulasStale); the
//! daemon reads those rows with [`ListStaleDatabaseRowsCommand`], computes
//! the outputs, and stores them with [`StoreDatabaseFormulaOutputsCommand`],
//! as values like any other, so filtering and sorting read them in SQL. A
//! row stays marked until its outputs are stored, so a restart computes
//! what was left ([`ListDatabasesWithStaleFormulasCommand`]).
//!
//! A formula reads the row's values of the properties that aren't formulas,
//! by name, so formulas never depend on each other and their order doesn't
//! matter.

use std::collections::{HashMap, HashSet};

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DatabaseDataType, DatabaseFormula, DatabaseProperty, DatabaseRowValue, DatabaseValue,
};
use cohesion_core::protocol::SetDatabasePropertyFormulaRequest;
use cohesion_core::value::IntoValue;
use cohesion_core::{FromRequest, Model, Projection};

use crate::changed::{Changed, Invalidates};
use crate::commands::database::GetDatabasePropertyCommand;
use crate::commands::database_row::store_value;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, DatabaseEventKind,
    Filtered, Read, ReadableDatabase, ValidationError, Write,
};

/// A row of `database_rows` with how many changes its formula outputs are
/// behind (0 when they are current), which [`DatabaseRow`] leaves out:
/// clients never see it.
///
/// [`DatabaseRow`]: cohesion_core::model::DatabaseRow
#[derive(Model)]
#[model(table = "database_rows", backup = Included)]
#[allow(dead_code)] // Written by `formulas_stale`; read through `RowStaleness`.
pub struct StaleMarkedRow {
    id: i64,
    database_id: i64,
    document_id: Option<i64>,
    formula_stale: i64,
}

/// A row's id and its mark, as a pass reads it: the outputs it stores clear
/// the mark only if no change raised it again meanwhile.
#[derive(Projection, Clone, Copy, Debug, PartialEq)]
#[projection(of = StaleMarkedRow)]
pub struct RowStaleness {
    pub id: i64,
    pub formula_stale: i64,
}

/// Marks the formula outputs of database `database_id`'s rows (only row
/// `row_id`'s, when given) as out of date and raises
/// [`DatabaseFormulasStale`](DatabaseEventKind::DatabaseFormulasStale) for
/// them; does nothing in a database with no formula. Every change to what a
/// formula reads calls this: a value, a row, a property, or a source.
pub(crate) fn formulas_stale(
    ctx: &mut CommandContext<'_>,
    database_id: i64,
    row_id: Option<i64>,
) -> Result<(), DatabaseError> {
    let marked = ctx.db.container().execute_statement(
        "UPDATE database_rows SET formula_stale = formula_stale + 1
         WHERE database_id = ?1 AND (?2 IS NULL OR id = ?2)
         AND EXISTS (SELECT 1 FROM database_properties
             WHERE database_id = ?1 AND formula_source IS NOT NULL)",
        [&database_id.into_value(), &row_id.into_value()],
    )?;
    if marked > 0 {
        ctx.events.send(
            ctx.trace
                .event(DatabaseEventKind::DatabaseFormulasStale { database_id }),
        );
    }
    Ok(())
}

/// A formula's source as a request carries it: absent, or not empty.
pub(crate) fn ensure_formula(formula: &Option<String>) -> Result<(), ValidationError> {
    if formula
        .as_ref()
        .is_some_and(|source| source.trim().is_empty())
    {
        return Err(ValidationError::in_field(
            "formula",
            "A formula can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// `source` as a new formula, with no error until it is computed.
pub(crate) fn new_formula(source: Option<String>) -> Option<DatabaseFormula> {
    source.map(|source| DatabaseFormula {
        source,
        error: None,
    })
}

/// Makes a property a formula with a non-empty source, changes its source,
/// or makes it a property set by hand again. A property becoming a formula
/// loses the values set by hand; one that stops being a formula keeps its
/// last outputs as values. The error of the old source goes, and every row
/// is computed again.
#[derive(FromRequest)]
#[request(SetDatabasePropertyFormulaRequest)]
pub struct SetDatabasePropertyFormulaCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub formula: Option<String>,
}
impl DatabaseCommand for SetDatabasePropertyFormulaCommand {
    type Response = DatabaseProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseProperty>> {
        ensure_formula(&self.formula)?;
        let before = GetDatabasePropertyCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        if before.formula.is_none() && self.formula.is_some() {
            ctx.db
                .delete::<DatabaseRowValue>()
                .filter_eq(DatabaseRowValue::PROPERTY_ID, self.property_id)
                .execute()?;
        }
        let property = ctx
            .db
            .update::<DatabaseProperty>()
            .set_group(new_formula(self.formula))
            .filter_eq(DatabaseProperty::ID, self.property_id)
            .returning_one()?
            .expect("the property was just read");
        formulas_stale(ctx, self.database_id, None)?;
        Ok(property.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// One row whose formula outputs are out of date, with what a formula reads
/// of it.
#[derive(Clone, Debug)]
pub struct StaleDatabaseRow {
    pub staleness: RowStaleness,
    /// Each property that isn't a formula, by name, with the row's value
    /// for it or `None` when empty. Of two properties with one name, the
    /// one created first is read.
    pub inputs: Vec<(String, Option<DatabaseValue>)>,
}

/// A formula property as a pass computes it.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaProperty {
    pub property_id: i64,
    pub source: String,
    /// The type its outputs must be.
    pub data_type: DatabaseDataType,
}

/// What a database's formulas need to bring its outdated rows up to date.
#[derive(Debug)]
pub struct StaleDatabaseRows {
    pub formulas: Vec<FormulaProperty>,
    pub rows: Vec<StaleDatabaseRow>,
}

/// The rows of a database whose formula outputs are out of date, by id,
/// with what its formulas read of each; nothing for a database that is
/// gone.
pub struct ListStaleDatabaseRowsCommand {
    pub database_id: i64,
}
impl DatabaseCommand for ListStaleDatabaseRowsCommand {
    type Response = StaleDatabaseRows;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<StaleDatabaseRows> {
        let properties: Vec<DatabaseProperty> = ctx
            .db
            .select()
            .filter_eq(DatabaseProperty::DATABASE_ID, self.database_id)
            .order("id")
            .all()?;
        let stale: Vec<RowStaleness> = ctx
            .db
            .select_as::<RowStaleness>()
            .filter_eq(StaleMarkedRow::DATABASE_ID, self.database_id)
            .filter("formula_stale > 0")
            .order("database_rows.id")
            .all()?;
        let stale_ids = ctx
            .db
            .select_id::<StaleMarkedRow>()
            .filter_eq(StaleMarkedRow::DATABASE_ID, self.database_id)
            .filter("formula_stale > 0");
        let mut values: HashMap<(i64, i64), DatabaseRowValue> = ctx
            .db
            .select::<DatabaseRowValue>()
            .filter_eq(DatabaseRowValue::DATABASE_ID, self.database_id)
            .filter_in("row_id", stale_ids)
            .all()?
            .into_iter()
            .map(|value| ((value.row_id, value.property_id), value))
            .collect();
        let mut formulas = Vec::new();
        let mut inputs = Vec::new();
        for property in properties {
            match property.formula {
                Some(DatabaseFormula { source, .. }) => formulas.push(FormulaProperty {
                    property_id: property.id,
                    source,
                    data_type: property.data_type,
                }),
                None => inputs.push(property),
            }
        }
        let mut names = HashSet::new();
        let inputs: Vec<DatabaseProperty> = inputs
            .into_iter()
            .filter(|property| names.insert(property.name.clone()))
            .collect();
        let rows = stale
            .into_iter()
            .map(|staleness| StaleDatabaseRow {
                staleness,
                inputs: inputs
                    .iter()
                    .map(|property| {
                        let value = values
                            .remove(&(staleness.id, property.id))
                            .and_then(|stored| stored.value(property.data_type));
                        (property.name.clone(), value)
                    })
                    .collect(),
            })
            .collect();
        Ok(StaleDatabaseRows { formulas, rows })
    }
}

/// What one formula computed for the rows of a pass, in the pass's order:
/// each row's output, empty or of the formula's type, or why it failed.
#[derive(Debug)]
pub struct FormulaOutputs {
    /// The formula as computed; outputs of a source or type the property no
    /// longer has are dropped.
    pub formula: FormulaProperty,
    pub outputs: Vec<Result<Option<DatabaseValue>, String>>,
}

/// Stores what a pass computed for `rows` of a database, read by
/// [`ListStaleDatabaseRowsCommand`], and clears each row's mark unless a
/// change raised it again meanwhile, whose own pass follows. Outputs of a
/// formula that changed or went since are dropped, as are those of a row
/// that went.
///
/// A formula's error is the first failure of the pass; a pass without one
/// clears it if it covered every row, and otherwise marks every row so the
/// pass that follows finds whether another row still fails.
pub struct StoreDatabaseFormulaOutputsCommand {
    pub database_id: i64,
    pub rows: Vec<RowStaleness>,
    pub formulas: Vec<FormulaOutputs>,
}
impl DatabaseCommand for StoreDatabaseFormulaOutputsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let current: HashSet<i64> = ctx
            .db
            .select_id::<StaleMarkedRow>()
            .filter_eq(StaleMarkedRow::DATABASE_ID, self.database_id)
            .all()?
            .into_iter()
            .collect();
        let covered: HashSet<i64> = self.rows.iter().map(|row| row.id).collect();
        let every_row = current.is_subset(&covered);
        let mut recheck = false;
        for computed in self.formulas {
            let property: Option<DatabaseProperty> = ctx
                .db
                .select()
                .filter_eq(DatabaseProperty::DATABASE_ID, self.database_id)
                .filter_eq(DatabaseProperty::ID, computed.formula.property_id)
                .one()?;
            let Some(property) = property.filter(|property| {
                property.data_type == computed.formula.data_type
                    && property
                        .formula
                        .as_ref()
                        .is_some_and(|formula| formula.source == computed.formula.source)
            }) else {
                continue;
            };
            let mut failure = None;
            for (row, output) in self.rows.iter().zip(computed.outputs) {
                if !current.contains(&row.id) {
                    continue;
                }
                let value = output.unwrap_or_else(|error| {
                    failure.get_or_insert(error);
                    None
                });
                store_value(ctx.db, &property, row.id, value)?;
            }
            let had_error = property
                .formula
                .as_ref()
                .is_some_and(|formula| formula.error.is_some());
            let error = match failure {
                Some(error) => Some(error),
                None if had_error && !every_row => {
                    recheck = true;
                    continue;
                }
                None => None,
            };
            ctx.db
                .update::<DatabaseProperty>()
                .set_group(Some(DatabaseFormula {
                    source: computed.formula.source,
                    error,
                }))
                .filter_eq(DatabaseProperty::ID, property.id)
                .execute()?;
        }
        for row in &self.rows {
            ctx.db.container().execute_statement(
                "UPDATE database_rows SET formula_stale = 0 WHERE id = ? AND formula_stale = ?",
                [&row.id.into_value(), &row.formula_stale.into_value()],
            )?;
        }
        if recheck {
            formulas_stale(ctx, self.database_id, None)?;
        }
        Ok(().invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// The databases with rows whose formula outputs are out of date, for the
/// daemon to bring up to date as it starts.
pub struct ListDatabasesWithStaleFormulasCommand;
impl DatabaseCommand for ListDatabasesWithStaleFormulasCommand {
    type Response = Vec<i64>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<i64>> {
        Ok(ctx
            .db
            .select_column::<StaleMarkedRow, i64>("database_id")
            .filter("formula_stale > 0")
            .distinct()
            .order("database_id")
            .all()?)
    }
}

#[cfg(test)]
mod tests;
