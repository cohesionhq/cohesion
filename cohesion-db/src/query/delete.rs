//! `DELETE` queries: hard deletes.

use std::marker::PhantomData;

use cohesion_core::value::IntoValue;
use cohesion_core::{Model, SingleTable};

use crate::container::Container;
use crate::database_error::DatabaseError;
use crate::query::clauses::{Filtered, Where};

/// `DELETE FROM <M::TABLE> [WHERE ...]`: a hard delete.
#[must_use]
pub struct Delete<'c, M: Model> {
    container: &'c Container,
    filter: Where,
    model: PhantomData<fn() -> M>,
}

impl<'c, M: Model> Delete<'c, M> {
    pub(super) fn new(container: &'c Container) -> Self {
        Self {
            container,
            filter: Where::default(),
            model: PhantomData,
        }
    }

    /// Adds a condition, joined to earlier ones with `AND`; `?`s are filled by `bind`.
    pub fn filter(mut self, condition: &str) -> Self {
        self.filter.push(condition);
        self
    }

    /// Supplies the next `?` placeholder.
    pub fn bind(mut self, value: impl IntoValue) -> Self {
        self.filter.params.push(value.into_value());
        self
    }

    /// Deletes every matching row; returns how many were removed.
    pub fn execute(self) -> Result<usize, DatabaseError> {
        self.container
            .execute_statement(&self.sql(false), &self.filter.params)
    }
}

/// A deleted row can't be read again through joins, so only a model of one
/// table, which `RETURNING *` reads whole, can be returned.
impl<M: SingleTable> Delete<'_, M> {
    /// Deletes the matching row and returns it; more than one match is an error.
    pub fn returning_one(self) -> Result<Option<M>, DatabaseError> {
        self.container
            .query_row(&self.sql(true), &self.filter.params, M::from_row)
    }

    /// Deletes every matching row and returns them.
    pub fn returning_all(self) -> Result<Vec<M>, DatabaseError> {
        self.container
            .query_all(&self.sql(true), &self.filter.params)
    }
}

impl<M: Model> Delete<'_, M> {
    pub(super) fn sql(&self, returning: bool) -> String {
        let mut sql = format!("DELETE FROM {}", M::TABLE);
        self.filter.write(&mut sql);
        if returning {
            sql.push_str(" RETURNING *");
        }
        sql
    }
}

impl<M: Model> Filtered for Delete<'_, M> {
    type Of = M;

    fn filter(self, condition: &str) -> Self {
        Delete::filter(self, condition)
    }

    fn bind(self, value: impl IntoValue) -> Self {
        Delete::bind(self, value)
    }

    fn conditions(&mut self) -> &mut Where {
        &mut self.filter
    }
}
