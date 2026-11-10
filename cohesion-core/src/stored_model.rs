//! A storage-agnostic description of a model, implemented with
//! `#[derive(Model)]`, so persistence and scripting can work with any model
//! without per-model code.

use std::marker::PhantomData;

use crate::value::{Row, RowError, Value};

/// What a model, or a projection of one, is called in a client's messages:
/// "The requested task folder was not found.". `#[derive(Model)]` spells the
/// struct's name in lower words, and `#[derive(Projection)]` takes its
/// model's.
pub trait Named {
    /// Named `NOUN` so a `name` column's const (`Extension::NAME`) can't shadow it.
    const NOUN: &'static str;
}

/// A stored column of model `M` holding values of type `T`, which
/// `#[derive(Model)]` declares for each field as `M::FIELD`
/// (`Task::TITLE`), so a query that names a column through it can't
/// misspell it, take another model's, or set it to a value of another type.
pub struct Column<M, T> {
    name: &'static str,
    qualified: &'static str,
    of: PhantomData<fn() -> (M, T)>,
}

impl<M, T> Column<M, T> {
    #[doc(hidden)]
    pub const fn new(name: &'static str, qualified: &'static str) -> Self {
        Self {
            name,
            qualified,
            of: PhantomData,
        }
    }

    /// The column's name, as `INSERT` and `UPDATE ... SET` write it.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// `table.column`, as a condition or a join writes it.
    pub const fn qualified(&self) -> &'static str {
        self.qualified
    }
}

impl<M, T> Clone for Column<M, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M, T> Copy for Column<M, T> {}

/// Whether a backup holds a table's rows. Every model says, so a new table
/// can't be left out of a backup, or fill one, by accident.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupClass {
    /// The rows go in every backup.
    Included,
    /// A backup holds none of them: bytes that don't compress and that
    /// nothing else depends on, such as a transcript's screenshots.
    LeftOut,
}

/// A struct stored as one row of `TABLE`, with the fields of other tables
/// it joins (`#[model(left_join = Client, on = "...")]`) read into it.
pub trait Model: Named + Sized {
    const TABLE: &'static str;
    /// Whether a backup holds this table's rows.
    const BACKUP: BackupClass;
    /// Every column, `id` first, then in declaration order; the columns of
    /// a [`ColumnGroup`] field in the group's order. A column is named
    /// after its Rust field.
    fn columns() -> Vec<&'static str>;
    /// Reads a model from one result row, by column name.
    fn from_row(row: &dyn Row) -> Result<Self, RowError>;
    /// The row's primary key.
    fn id(&self) -> i64;

    /// The `SELECT` list that reads the model: `"table.id, table.title, ..."`
    /// with every column qualified by its table, so a join can't make it
    /// ambiguous, and a field read some other way (`expr`, `or_from`) as its
    /// expression `AS` the field's name.
    fn select_list() -> String;
    /// The joins the model's reads need, such as
    /// `" LEFT JOIN clients ON clients.id = t.client_id"`; empty for a model
    /// of one table. Every read of the model (`select`, `select_live`,
    /// `Insert` and `Update`'s `returning_*`) applies them, so a field that
    /// comes from another table is never missing.
    fn joins() -> String;
}

/// A model read from its own table alone: no joins, and no field read from
/// anything but its column. Implemented by `#[derive(Model)]` for those, and
/// only those; a statement's `RETURNING *` can read nothing else, so
/// `Delete::returning_*` accepts only these models.
pub trait SingleTable: Model {}

/// A model with a `deleted: bool` column that rows are hidden by rather than
/// removed. Implemented by `#[derive(Model)]` when such a field exists;
/// `ReadableDatabase::select_live` only accepts these models.
pub trait SoftDelete: Model {}

/// Some of a model's columns, or SQL expressions over them, read as their
/// own struct: what a caller selects when the whole row is more than it
/// needs. A projection may also join other models' tables and take columns
/// from them. Implemented with `#[derive(Projection)]`; read with
/// `ReadableDatabase::select_as`.
pub trait Projection: Named + Sized {
    /// The model whose table the columns come from.
    type Of: Model;
    /// `"t.id, t.title, substr(t.contents, 1, 280) AS excerpt"` for SELECT
    /// lists: base columns qualified by their table.
    fn columns() -> String;
    /// The joins, such as `" LEFT JOIN clients ON clients.id = t.client_id"`;
    /// empty when the projection reads one table.
    fn joins() -> String;
    /// Reads the projection from one result row, by column name.
    fn from_row(row: &dyn Row) -> Result<Self, RowError>;
}

/// Fails to compile, through `#[derive(Model)]`, when a [`ColumnGroup`]
/// shares a column name with the model's other columns or another group.
pub const fn assert_columns_disjoint(group: &[&str], others: &[&str]) {
    let mut i = 0;
    while i < group.len() {
        let mut j = 0;
        while j < others.len() {
            if str_eq(group[i], others[j]) {
                panic!("a ColumnGroup column has the name of another column of the model");
            }
            j += 1;
        }
        i += 1;
    }
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Several nullable columns of a model that together say one thing, held
/// as one enum field: each variant is a combination of columns that are
/// set, and the other variants' columns are `NULL`. Implemented with
/// `#[derive(ColumnGroup)]`; a model field marked `#[model(group)]` is
/// read and written through it.
pub trait ColumnGroup: Sized {
    /// Every column of every variant, each once.
    const COLUMNS: &'static [&'static str];
    /// Reads the variant the row's columns spell out; a row whose columns
    /// match no variant, or more than one, is a [`RowError`].
    fn from_row(row: &dyn Row) -> Result<Self, RowError>;
    /// A value for every one of [`COLUMNS`](Self::COLUMNS), in order:
    /// `NULL` for the columns this variant doesn't have.
    fn into_values(self) -> Vec<Value>;
    /// What the group reads besides its columns, as `(name, sql)`: SQL over
    /// the model's row, such as the rows of another table that belong to it,
    /// selected `AS` the name [`from_row`](Self::from_row) reads it by, and
    /// never written. Empty for a group of columns alone. The field of a
    /// group that reads any is marked `#[model(group, reads)]`, which keeps
    /// its model from being [`SingleTable`]: `RETURNING *` can't read them.
    const READS: &'static [(&'static str, &'static str)] = &[];
}
