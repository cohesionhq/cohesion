//! `SELECT` queries.

use std::collections::HashMap;
use std::marker::PhantomData;

use cohesion_core::model::{HasMetadata, Metadata};
use cohesion_core::value::{FromValue, IntoValue, Row, RowError, read_column};
use cohesion_core::{Model, Projection, Value};

use crate::container::Container;
use crate::database_error::DatabaseError;
use crate::query::clauses::{Filtered, Where};

/// `SELECT <columns> FROM <M::TABLE> [joins] [WHERE ...] [ORDER BY ...] [LIMIT n [OFFSET m]]`,
/// reading each row as `O`: the whole model by default, one of its columns,
/// or a [`Projection`] of it (which may join other tables).
///
/// Columns of `M` are qualified by its table, so a join never makes them
/// ambiguous. A `filter` is free SQL: on a joined model or projection,
/// qualify a column the joined tables share (`"clients.id = ?"`).
#[must_use]
pub struct Select<'c, M: Model, O = M> {
    container: &'c Container,
    columns: String,
    joins: String,
    distinct: bool,
    read: fn(&dyn Row) -> Result<O, RowError>,
    pub(super) filter: Where,
    order: Option<String>,
    /// What fills the `?`s of `order`, which come after the filters'.
    order_params: Vec<Value>,
    limit: Option<u64>,
    offset: u64,
    model: PhantomData<fn() -> M>,
}

impl<'c, M: Model> Select<'c, M> {
    pub(super) fn new(container: &'c Container) -> Self {
        Self::with_columns(container, M::select_list(), M::joins(), M::from_row)
    }
}

impl<'c, P: Projection> Select<'c, P::Of, P> {
    pub(super) fn projected(container: &'c Container) -> Self {
        Self::with_columns(container, P::columns(), P::joins(), P::from_row)
    }
}

impl<'c, M: Model, T: FromValue> Select<'c, M, T> {
    /// Selects just `column` of `M`'s rows, each read as a `T`.
    pub(super) fn column(container: &'c Container, column: &'static str) -> Self {
        debug_assert!(
            M::columns().contains(&column),
            "{column} is not a column of {}",
            M::TABLE
        );
        Self::with_columns(
            container,
            format!("{}.{column} AS value", M::TABLE),
            M::joins(),
            |row| read_column(row, "value"),
        )
    }
}

impl<'c, M: Model, O> Select<'c, M, O> {
    fn with_columns(
        container: &'c Container,
        columns: String,
        joins: String,
        read: fn(&dyn Row) -> Result<O, RowError>,
    ) -> Self {
        Self {
            container,
            columns,
            joins,
            distinct: false,
            read,
            filter: Where::default(),
            order: None,
            order_params: Vec::new(),
            limit: None,
            offset: 0,
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

    /// Sets the `ORDER BY` clause, e.g. `"modified DESC"`.
    pub fn order(mut self, order: &str) -> Self {
        self.order = Some(order.to_string());
        self
    }

    /// Sets the `ORDER BY` clause with one `?` placeholder, filled by
    /// `value` whatever the filters bind, e.g. a sort key read from another
    /// table for the id a client names.
    pub fn order_bound(mut self, order: &str, value: impl IntoValue) -> Self {
        self.order = Some(order.to_string());
        self.order_params = vec![value.into_value()];
        self
    }

    /// Leaves out repeated rows.
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Returns at most `limit` rows.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skips the first `offset` rows; only with a `limit`.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    /// Every matching row.
    pub fn all(self) -> Result<Vec<O>, DatabaseError> {
        self.container.query_rows(
            &self.sql(),
            self.params().chain(&self.order_params),
            self.read,
        )
    }

    /// The matching row, if any; more than one is an error.
    pub fn one(self) -> Result<Option<O>, DatabaseError> {
        self.container.query_row(
            &self.sql(),
            self.params().chain(&self.order_params),
            self.read,
        )
    }

    /// Whether any row matches. Runs `SELECT EXISTS(...)`, so no row is read
    /// or parsed; `order` and `limit` are ignored.
    pub fn exists(self) -> Result<bool, DatabaseError> {
        self.container
            .query_row(&self.exists_sql(), self.params(), |row| {
                read_column(row, "found")
            })
            .map(|found| found.unwrap_or(false))
    }

    /// How many rows match. Runs `SELECT COUNT(*)`, so no row is read;
    /// `order`, `limit` and `offset` are ignored.
    pub fn count(self) -> Result<i64, DatabaseError> {
        let mut sql = String::from("SELECT COUNT(*) AS value");
        self.write_from(&mut sql);
        self.container
            .query_row(&sql, self.params(), |row| read_column(row, "value"))
            .map(|count| count.unwrap_or(0))
    }

    /// The SQL and parameters of this query used as `(SELECT ...)` inside
    /// another's condition.
    pub(super) fn into_subquery(self) -> (String, Vec<Value>) {
        let sql = self.sql();
        let mut params = self.filter.params;
        params.extend(self.order_params);
        (sql, params)
    }

    /// What fills the filters' `?`s; `order`'s come after them.
    fn params(&self) -> impl Iterator<Item = &Value> {
        self.filter.params.iter()
    }

    fn write_from(&self, sql: &mut String) {
        sql.push_str(" FROM ");
        sql.push_str(M::TABLE);
        sql.push_str(&self.joins);
        self.filter.write(sql);
    }

    pub(super) fn sql(&self) -> String {
        let mut sql = format!(
            "SELECT {}{}",
            if self.distinct { "DISTINCT " } else { "" },
            self.columns
        );
        self.write_from(&mut sql);
        if let Some(order) = &self.order {
            sql.push_str(" ORDER BY ");
            sql.push_str(order);
        }
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
            if self.offset > 0 {
                sql.push_str(&format!(" OFFSET {}", self.offset));
            }
        }
        sql
    }

    pub(super) fn exists_sql(&self) -> String {
        let mut sql = String::from("SELECT EXISTS(SELECT 1");
        self.write_from(&mut sql);
        sql.push_str(") AS found");
        sql
    }
}

impl<M: Model, O> Filtered for Select<'_, M, O> {
    type Of = M;

    fn filter(self, condition: &str) -> Self {
        Select::filter(self, condition)
    }

    fn bind(self, value: impl IntoValue) -> Self {
        Select::bind(self, value)
    }

    fn conditions(&mut self) -> &mut Where {
        &mut self.filter
    }
}

/// The rows of `M` with these `ids`, read through the model's own select
/// (its joins and derived fields), in the order of `ids`: how a write
/// returns what it stored, since `RETURNING` can't join.
pub(super) fn read_back<M: Model>(
    container: &Container,
    ids: &[i64],
) -> Result<Vec<M>, DatabaseError> {
    let list = format!(
        "[{}]",
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    );
    let mut rows = Select::<M>::new(container)
        .filter(&format!(
            "{}.id IN (SELECT value FROM json_each(?))",
            M::TABLE
        ))
        .bind(list)
        .all()?;
    rows.sort_by_key(|row| ids.iter().position(|id| *id == row.id()));
    Ok(rows)
}

/// Filtering a model's rows by the metadata scripts attached to them.
impl<'c, M: HasMetadata, O> Select<'c, M, O> {
    /// Keeps only rows carrying every one of these metadata pairs.
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        for (key, value) in metadata {
            let carrying = Select::<Metadata, i64>::column(self.container, M::METADATA_COLUMN)
                .filter("key = ? AND value = ?")
                .bind(key)
                .bind(value);
            self = self.filter_in("id", carrying);
        }
        self
    }

    /// Keeps only rows carrying every one of these metadata keys, whatever
    /// their values.
    pub fn with_metadata_keys(mut self, keys: Vec<String>) -> Self {
        for key in keys {
            let carrying = Select::<Metadata, i64>::column(self.container, M::METADATA_COLUMN)
                .filter("key = ?")
                .bind(key);
            self = self.filter_in("id", carrying);
        }
        self
    }
}
