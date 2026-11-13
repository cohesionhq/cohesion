//! `UPDATE` queries.

use std::marker::PhantomData;

use cohesion_core::Model;
use cohesion_core::value::{IntoValue, Value};

use crate::container::Container;
use crate::database_error::DatabaseError;
use crate::query::clauses::{Assignments, Filtered, Where, assert_column, assignment_methods};
use crate::query::select::read_back;
use rusqlite::Error::QueryReturnedMoreThanOneRow;

/// `UPDATE <M::TABLE> SET ... [WHERE ...]`.
#[must_use]
pub struct Update<'c, M: Model> {
    container: &'c Container,
    values: Assignments,
    filter: Where,
    model: PhantomData<fn() -> M>,
}

impl<'c, M: Model> Update<'c, M> {
    pub(super) fn new(container: &'c Container) -> Self {
        Self {
            container,
            values: Assignments::default(),
            filter: Where::default(),
            model: PhantomData,
        }
    }

    assignment_methods!();

    /// Adds `text` to the end of the text in `column`.
    pub fn append(mut self, column: &'static str, text: impl IntoValue) -> Self {
        assert_column::<M>(column);
        self.values.raw_set(column, format!("{column} || ?"));
        self.values.params.push(text.into_value());
        self
    }

    /// Adds `text` to `column` on a new line: no newline is added to empty text.
    pub fn append_line(mut self, column: &'static str, text: impl IntoValue) -> Self {
        assert_column::<M>(column);
        self.values.raw_set(
            column,
            format!("{column} || iif({column} = '', '', char(10)) || ?"),
        );
        self.values.params.push(text.into_value());
        self
    }

    /// Adds one to the number in `column`.
    pub fn increment(mut self, column: cohesion_core::Column<M, i64>) -> Self {
        let name = column.name();
        self.values.raw_set(name, format!("{name} + 1"));
        self
    }

    pub fn filter(mut self, condition: &str) -> Self {
        self.filter.push(condition);
        self
    }

    pub fn bind(mut self, value: impl IntoValue) -> Self {
        self.filter.params.push(value.into_value());
        self
    }

    /// Updates the matching row and returns it as stored, read through the
    /// model's own select, so joined and derived fields are filled; more
    /// than one match is an error.
    pub fn returning_one(self) -> Result<Option<M>, DatabaseError> {
        let ids = self.container.query_ids(&self.sql(true), self.params())?;
        if ids.len() > 1 {
            return Err(DatabaseError::Sql(QueryReturnedMoreThanOneRow));
        }
        Ok(read_back::<M>(self.container, &ids)?.pop())
    }

    /// Updates every matching row and returns them as stored, like
    /// [`returning_one`](Self::returning_one).
    pub fn returning_all(self) -> Result<Vec<M>, DatabaseError> {
        let ids = self.container.query_ids(&self.sql(true), self.params())?;
        read_back::<M>(self.container, &ids)
    }

    pub fn execute(self) -> Result<usize, DatabaseError> {
        self.container
            .execute_statement(&self.sql(false), self.params())
    }

    pub(super) fn params(&self) -> impl Iterator<Item = &Value> {
        self.values.params.iter().chain(&self.filter.params)
    }

    pub(super) fn sql(&self, returning: bool) -> String {
        let assignments: Vec<String> = self
            .values
            .columns
            .iter()
            .map(|(column, value)| format!("{column} = {value}"))
            .collect();
        let mut sql = format!("UPDATE {} SET {}", M::TABLE, assignments.join(", "));
        self.filter.write(&mut sql);
        if returning {
            sql.push_str(" RETURNING id");
        }
        sql
    }
}

impl<M: Model> Filtered for Update<'_, M> {
    type Of = M;

    fn filter(self, condition: &str) -> Self {
        Update::filter(self, condition)
    }

    fn bind(self, value: impl IntoValue) -> Self {
        Update::bind(self, value)
    }

    fn conditions(&mut self) -> &mut Where {
        &mut self.filter
    }
}
