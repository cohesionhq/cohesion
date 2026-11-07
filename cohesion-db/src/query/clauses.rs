//! The `WHERE` and assignment clauses the builders share.

use cohesion_core::value::IntoValue;
use cohesion_core::{Column, DayFilter, Model, SoftDelete, Value};

use crate::query::select::Select;

/// Clauses shared by every builder that has a `WHERE`.
#[derive(Default)]
pub struct Where {
    pub(super) filters: Vec<String>,
    pub(super) params: Vec<Value>,
}

impl Where {
    pub(super) fn push(&mut self, filter: impl Into<String>) {
        self.filters.push(filter.into());
    }

    pub(super) fn write(&self, sql: &mut String) {
        if !self.filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&self.filters.join(" AND "));
        }
    }
}

/// What every builder with a `WHERE` offers: `filter` and `bind`, so one
/// function can narrow a `Select`, an `Update` or a `Delete` alike, and
/// conditions on a column of the model that name a subquery or a list of
/// values, so a command never writes `IN (SELECT ...)` or counts
/// placeholders itself. Each takes the column by name, qualified with the
/// model's table unless it already names one.
pub trait Filtered: Sized {
    /// The model whose rows the builder filters.
    type Of: Model;

    /// Adds a condition, joined to earlier ones with `AND`; `?`s are filled
    /// by `bind`.
    fn filter(self, condition: &str) -> Self;

    /// Supplies the next `?` placeholder.
    fn bind(self, value: impl IntoValue) -> Self;

    /// Where the next condition goes, with any `bind` after it filling it.
    fn conditions(&mut self) -> &mut Where;

    /// Keeps the rows that have not been soft-deleted.
    fn live(mut self) -> Self
    where
        Self::Of: SoftDelete,
    {
        self.conditions()
            .push(format!("{}.deleted = FALSE", <Self::Of as Model>::TABLE));
        self
    }

    /// Keeps rows whose `column` is `value`, or `NULL` when `value` is
    /// `None`: the typed form of `filter("column = ?").bind(value)`.
    fn filter_eq<T: IntoValue>(mut self, column: Column<Self::Of, T>, value: impl Into<T>) -> Self {
        let conditions = self.conditions();
        match value.into().into_value() {
            Value::Null => conditions.push(format!("{} IS NULL", column.qualified())),
            value => {
                conditions.push(format!("{} = ?", column.qualified()));
                conditions.params.push(value);
            }
        }
        self
    }

    /// Keeps rows whose `column` is among the single column `subquery`
    /// selects (see `ReadableDatabase::select_column`).
    fn filter_in<S: Model, T>(mut self, column: &str, subquery: Select<'_, S, T>) -> Self {
        let column = qualify::<Self::Of>(column);
        push_subquery(self.conditions(), &column, "IN", subquery);
        self
    }

    /// Keeps rows whose `column` is not among the single column `subquery`
    /// selects.
    fn filter_not_in<S: Model, T>(mut self, column: &str, subquery: Select<'_, S, T>) -> Self {
        let column = qualify::<Self::Of>(column);
        push_subquery(self.conditions(), &column, "NOT IN", subquery);
        self
    }

    /// Keeps rows whose `column` is one of `values`; none match when
    /// `values` is empty.
    fn filter_in_values<V: IntoValue>(
        mut self,
        column: &str,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        let column = qualify::<Self::Of>(column);
        push_values(self.conditions(), &column, "IN", values);
        self
    }

    /// Keeps rows whose `column` is none of `values`; all match when
    /// `values` is empty.
    fn filter_not_in_values<V: IntoValue>(
        mut self,
        column: &str,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        let column = qualify::<Self::Of>(column);
        push_values(self.conditions(), &column, "NOT IN", values);
        self
    }

    /// Keeps rows whose columns hold `group`, compared as `set_group` would
    /// store it (`IS`, so a `NULL` column matches a `None`).
    fn filter_group<G: cohesion_core::ColumnGroup>(mut self, group: G) -> Self {
        for (column, value) in G::COLUMNS.iter().zip(group.into_values()) {
            let column = qualify::<Self::Of>(column);
            let conditions = self.conditions();
            conditions.push(format!("{column} IS ?"));
            conditions.params.push(value);
        }
        self
    }

    /// Keeps rows whose `YYYY-MM-DD` `column`, a column or an SQL expression
    /// over columns, matches `days`. Range bounds go through `date()`, so a
    /// malformed bound matches nothing rather than comparing as text.
    fn filter_day(mut self, column: &str, days: DayFilter) -> Self {
        match days {
            DayFilter::Day(day) => self.filter(&format!("{column} = ?")).bind(day),
            DayFilter::Days(days) => {
                push_values(self.conditions(), column, "IN", days);
                self
            }
            DayFilter::Range(range) => {
                if let Some(before) = range.before {
                    self = self.filter(&format!("{column} < date(?)")).bind(before);
                }
                if let Some(after) = range.after {
                    self = self.filter(&format!("{column} > date(?)")).bind(after);
                }
                self
            }
        }
    }
}

/// Adds `column <operator> (subquery)` to `conditions`.
fn push_subquery<S: Model, T>(
    conditions: &mut Where,
    column: &str,
    operator: &str,
    subquery: Select<'_, S, T>,
) {
    let (sql, params) = subquery.into_subquery();
    conditions.push(format!("{column} {operator} ({sql})"));
    conditions.params.extend(params);
}

/// Adds `column <operator> (values...)` to `conditions`: an empty `IN` list
/// matches no row, an empty `NOT IN` list every row.
fn push_values<V: IntoValue>(
    conditions: &mut Where,
    column: &str,
    operator: &str,
    values: impl IntoIterator<Item = V>,
) {
    let values: Vec<Value> = values.into_iter().map(IntoValue::into_value).collect();
    if values.is_empty() {
        if operator == "IN" {
            conditions.push("FALSE");
        }
        return;
    }
    let placeholders = vec!["?"; values.len()].join(", ");
    conditions.push(format!("{column} {operator} ({placeholders})"));
    conditions.params.extend(values);
}

/// Catches, in debug builds, a column that isn't one of `M`'s.
pub(super) fn assert_column<M: Model>(column: &str) {
    debug_assert!(
        M::columns().contains(&column),
        "{column} is not a column of {}",
        M::TABLE
    );
}

/// `column` as SQL: qualified with `M`'s table unless it already is.
pub(super) fn qualify<M: Model>(column: &str) -> String {
    if column.contains('.') {
        column.to_string()
    } else {
        debug_assert!(
            M::columns().contains(&column),
            "{column} is not a column of {}",
            M::TABLE
        );
        format!("{}.{column}", M::TABLE)
    }
}

/// Column assignments for `INSERT` and `UPDATE`: a column and its SQL (`?` or an expression).
#[derive(Default)]
pub(super) struct Assignments {
    pub(super) columns: Vec<(&'static str, String)>,
    pub(super) params: Vec<Value>,
}

impl Assignments {
    pub(super) fn set(&mut self, column: &'static str, value: Value) {
        self.columns.push((column, "?".to_string()));
        self.params.push(value);
    }

    pub(super) fn raw_set(&mut self, column: &'static str, expression: impl Into<String>) {
        self.columns.push((column, expression.into()));
    }
}

/// The column setters `Insert` and `Update` share, written once and expanded
/// into each builder's own `impl` (as inherent methods, so callers import
/// nothing). The builder keeps its columns in `self.values` and its model is `M`.
macro_rules! assignment_methods {
    () => {
        /// Stores `value` in `column`.
        pub fn set<T: IntoValue>(
            mut self,
            column: cohesion_core::Column<M, T>,
            value: impl Into<T>,
        ) -> Self {
            self.values.set(column.name(), value.into().into_value());
            self
        }

        /// Stores a [`ColumnGroup`](cohesion_core::ColumnGroup) field: every
        /// one of its columns, `NULL` for those the group's current variant
        /// doesn't have.
        pub fn set_group<G: cohesion_core::ColumnGroup>(mut self, group: G) -> Self {
            for (column, value) in G::COLUMNS.iter().zip(group.into_values()) {
                assert_column::<M>(column);
                self.values.set(column, value);
            }
            self
        }

        /// Stores `NULL` in nullable `column`.
        pub fn set_null<T>(self, column: cohesion_core::Column<M, Option<T>>) -> Self {
            self.set_sql(column.name(), "NULL")
        }

        /// Stores `TRUE` in `column`.
        pub fn set_true<T: From<bool>>(self, column: cohesion_core::Column<M, T>) -> Self {
            self.set_sql(column.name(), "TRUE")
        }

        /// Stores `FALSE` in `column`.
        pub fn set_false<T: From<bool>>(self, column: cohesion_core::Column<M, T>) -> Self {
            self.set_sql(column.name(), "FALSE")
        }

        /// Stores the current time, in Unix seconds, in `column`.
        pub fn set_now<T: From<i64>>(self, column: cohesion_core::Column<M, T>) -> Self {
            self.set_sql(column.name(), "unixepoch()")
        }

        fn set_sql(mut self, column: &'static str, expression: &'static str) -> Self {
            self.values.raw_set(column, expression);
            self
        }
    };
}
pub(super) use assignment_methods;
