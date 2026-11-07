//! `INSERT` queries, including upserts.

use std::marker::PhantomData;

use cohesion_core::value::IntoValue;
use cohesion_core::{Column, Model};

use crate::container::Container;
use crate::database_error::DatabaseError;
use crate::query::clauses::{Assignments, assert_column, assignment_methods};
use crate::query::select::read_back;

/// `INSERT INTO <M::TABLE> (...) VALUES (...) [ON CONFLICT (...) DO UPDATE SET ...]`.
#[must_use]
pub struct Insert<'c, M: Model> {
    container: &'c Container,
    pub(super) values: Assignments,
    conflict: &'static [&'static str],
    /// A column the new row must exceed the conflicting one's in for an
    /// upsert to overwrite it.
    increasing: Option<&'static str>,
    model: PhantomData<fn() -> M>,
}

impl<'c, M: Model> Insert<'c, M> {
    pub(super) fn new(container: &'c Container) -> Self {
        Self {
            container,
            values: Assignments::default(),
            conflict: &[],
            increasing: None,
            model: PhantomData,
        }
    }

    assignment_methods!();

    /// Makes this an upsert: when a row with the same `columns` already exists,
    /// overwrite its other set columns instead of failing. `columns` must be
    /// covered by a primary key or unique constraint.
    pub fn on_conflict_update(mut self, columns: &'static [&'static str]) -> Self {
        for column in columns {
            assert_column::<M>(column);
        }
        self.conflict = columns;
        self
    }

    /// Narrows an upsert: a conflicting row is overwritten only when the new
    /// row's `column` is greater than its own, and left as it is otherwise.
    pub fn when_greater<T>(mut self, column: Column<M, T>) -> Self {
        self.increasing = Some(column.name());
        self
    }

    /// Stores in `column` one past the largest value in the table, or 1 in
    /// an empty one: where a new row goes at the end of a sorted list.
    pub fn set_max_incremented<T: From<i64>>(mut self, column: Column<M, T>) -> Self {
        let column = column.name();
        self.values.raw_set(
            column,
            format!("(SELECT COALESCE(MAX({column}), 0) + 1 FROM {})", M::TABLE),
        );
        self
    }

    /// Like [`set_max_incremented`](Self::set_max_incremented), counting only the rows
    /// whose `scope_column` is `scope`: the end of one parent's list.
    pub fn set_max_incremented_within<T: From<i64>, S: IntoValue>(
        mut self,
        column: Column<M, T>,
        scope_column: Column<M, S>,
        scope: impl Into<S>,
    ) -> Self {
        let (column, scope_column) = (column.name(), scope_column.name());
        self.values.raw_set(
            column,
            format!(
                "(SELECT COALESCE(MAX({column}), 0) + 1 FROM {} WHERE {scope_column} = ?)",
                M::TABLE
            ),
        );
        self.values.params.push(scope.into().into_value());
        self
    }

    /// Inserts the row and returns it as stored, read through the model's
    /// own select, so joined and derived fields are filled. `None` when an
    /// upsert's conflict left the row alone.
    pub fn returning_one(self) -> Result<Option<M>, DatabaseError> {
        let ids = self
            .container
            .query_ids(&self.sql(true), &self.values.params)?;
        Ok(read_back::<M>(self.container, &ids)?.pop())
    }

    /// Inserts the row; returns the number of rows inserted.
    pub fn execute(self) -> Result<usize, DatabaseError> {
        self.container
            .execute_statement(&self.sql(false), &self.values.params)
    }

    /// Inserts the row; returns the number of rows inserted. Does not
    /// print to console.
    pub fn execute_silently(self) -> Result<usize, DatabaseError> {
        self.container
            .execute_statement_silently(&self.sql(false), &self.values.params)
    }

    pub(super) fn sql(&self, returning: bool) -> String {
        let mut sql = if self.values.columns.is_empty() {
            format!("INSERT INTO {} DEFAULT VALUES", M::TABLE)
        } else {
            let (columns, values): (Vec<&str>, Vec<&str>) = self
                .values
                .columns
                .iter()
                .map(|(column, value)| (*column, value.as_str()))
                .unzip();
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                M::TABLE,
                columns.join(", "),
                values.join(", ")
            )
        };
        if !self.conflict.is_empty() {
            let conflict = self.conflict.join(", ");
            let updates: Vec<String> = self
                .values
                .columns
                .iter()
                .filter(|(column, _)| !self.conflict.contains(column))
                .map(|(column, _)| format!("{column} = excluded.{column}"))
                .collect();
            if updates.is_empty() {
                sql.push_str(&format!(" ON CONFLICT ({conflict}) DO NOTHING"));
            } else {
                sql.push_str(&format!(
                    " ON CONFLICT ({conflict}) DO UPDATE SET {}",
                    updates.join(", ")
                ));
                if let Some(column) = self.increasing {
                    sql.push_str(&format!(" WHERE excluded.{column} > {}.{column}", M::TABLE));
                }
            }
        }
        if returning {
            sql.push_str(" RETURNING id");
        }
        sql
    }
}
