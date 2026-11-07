//! Query builders: each collects clauses and parameters, assembles its SQL
//! once in `sql()`, and runs it through the raw [`Container`](crate::Container).
//! A command starts reads from the [`ReadableDatabase`] trait both views
//! implement, and writes from [`WriteContainer`](crate::WriteContainer).
//! Only the full-text searches, whose virtual tables are no model, write a
//! `SELECT` of their own.
//!
//! `bind` supplies the `?` placeholders of `filter`s, in order, so
//! parameters line up with the SQL text even when `set`s come between. `filter` takes any SQL
//! condition, with or without placeholders. [`Filtered`] adds the typed
//! ones every builder with a `WHERE` shares: membership in a subquery
//! (`filter_in(column, db.select_column::<Other, _>("other_id"))`) or a list
//! of values, and a day filter.
//!
//! A model's columns are qualified by its table in a `SELECT`, so a
//! [`Projection`](cohesion_core::Projection) or a model that joins other
//! models never makes them ambiguous; a `filter` on a joined select
//! qualifies a column the tables share (`"tasks.id = ?"`). A joined model is
//! read whole by every select, and `Insert` and `Update` re-read what they
//! wrote through its select (`RETURNING` can't join); `Delete::returning_*`
//! needs a [`SingleTable`](cohesion_core::SingleTable) model. `Insert` and
//! `Update` share their setters: `set` (`Json` values included), `set_group`
//! for a `ColumnGroup` enum, `set_null`/`set_true`/`set_false`/`set_now`.

mod clauses;
mod container_builders;
mod delete;
mod insert;
mod select;
mod update;

pub use clauses::Filtered;
pub use container_builders::ReadableDatabase;
pub use delete::Delete;
pub use insert::Insert;
pub use select::Select;
pub use update::Update;

#[cfg(test)]
mod tests;
