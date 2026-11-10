//! The SQLite connection and the primitives every query goes through.
//! [`Container`] owns the connection, runs migrations and holds the raw
//! statement helpers; [`ReadContainer`] and [`WriteContainer`] are the
//! borrowed views a read or write command sees. Both read through
//! `ReadableDatabase`; only [`WriteContainer`] adds the write builders. The
//! builders that start a query (`select`, `insert`, ...) are in
//! `query/container_builders.rs`.

use std::path::{Path, PathBuf};

use cohesion_core::log::Module;
use cohesion_core::model::{ImageBytes, LinkKind, LinkTarget, outgoing_links};
use cohesion_core::private_files::{create_private_dir, open_private};
use cohesion_core::protocol::DatabaseSize;
use cohesion_core::value::{Row, RowError, read_column};
use cohesion_core::{Model, Value};
use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, Error::QueryReturnedMoreThanOneRow, params_from_iter};

use crate::database_error::DatabaseError;
use crate::migrations::{CASCADING_MIGRATIONS, MIGRATIONS};
use crate::retention::worth_vacuuming;
use crate::sql_value::{ParamRef, SqlRow};
use crate::storage_areas;

/// The open SQLite connection and the raw statement helpers every query goes
/// through. Borrow it as a [`WriteContainer`] (or [`ReadContainer`]) to query it.
pub struct Container {
    connection: Connection,
}

/// What a read command sees: `select`s over a borrowed connection.
#[derive(Clone, Copy)]
pub struct ReadContainer<'c> {
    pub(crate) container: &'c Container,
}

/// What a write command sees: everything a read sees, plus the write builders.
#[derive(Clone, Copy)]
pub struct WriteContainer<'c> {
    pub(crate) container: &'c Container,
}

impl<'c> ReadContainer<'c> {
    /// Borrows `container` for reading.
    pub fn new(container: &'c Container) -> Self {
        Self { container }
    }
}

impl<'c> WriteContainer<'c> {
    /// Borrows `container` for reading and writing.
    pub fn new(container: &'c Container) -> Self {
        Self { container }
    }

    /// This view narrowed to reads.
    pub fn read(self) -> ReadContainer<'c> {
        ReadContainer::new(self.container)
    }
}

impl Container {
    /// Opens (or creates) the database file at `path`, readable by this user
    /// alone, as is its directory. Call [`Container::migrate`] next.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref();
        if let Some(directory) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            create_private_dir(directory)?;
        }
        // SQLite gives its journal and WAL files the database file's mode.
        if !path.exists() {
            open_private(path)?;
        }
        let container = Self::configure(Connection::open(path)?)?;
        // Readers don't wait on a commit, and a commit writes less. This
        // stays at SQLite's default `synchronous = FULL`, so a write a client
        // heard back about survives losing power. A copy of a live file must
        // go through SQLite (`.backup`, `VACUUM INTO`), never `cp` alone.
        container
            .connection
            .query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        Ok(container)
    }

    /// A private, empty database that disappears when dropped; for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        Self::configure(Connection::open_in_memory()?)
    }

    fn configure(connection: Connection) -> Result<Self, DatabaseError> {
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { connection })
    }

    /// Applies, in order, every migration this database has not seen yet.
    pub fn migrate(&mut self) -> Result<(), DatabaseError> {
        let applied: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        let applied = usize::try_from(applied).unwrap_or(0);
        if applied > MIGRATIONS.len() {
            return Err(DatabaseError::UnknownSchema {
                applied,
                known: MIGRATIONS.len(),
            });
        }
        self.register_migration_functions()?;
        let migrated = self.apply(applied);
        self.connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrated
    }

    /// What SQLite lacks that a migration needs: `unbase64(text)`, the bytes
    /// base64 text holds or `NULL`, to move an image out of it; and
    /// `outgoing_links(kind, id, text)`, the links the text of item `kind`
    /// `id` makes (`outgoing_links`) as a JSON array of `{kind, id, link}`,
    /// to keep its backlinks by the rule the commands keep them by.
    pub(crate) fn register_migration_functions(&self) -> Result<(), DatabaseError> {
        self.connection.create_scalar_function(
            "outgoing_links",
            3,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |context| {
                let kind = LinkKind::from_str_strict(&context.get::<String>(0)?)
                    .map_err(|error| rusqlite::Error::UserFunctionError(error.into()))?;
                let source = LinkTarget {
                    kind,
                    id: context.get(1)?,
                };
                let links: Vec<_> = outgoing_links(source, &context.get::<String>(2)?)
                    .into_iter()
                    .map(|link| {
                        serde_json::json!({
                            "kind": link.target.kind.as_str(),
                            "id": link.target.id,
                            "link": link.link,
                        })
                    })
                    .collect();
                Ok(serde_json::Value::Array(links).to_string())
            },
        )?;
        self.connection.create_scalar_function(
            "unbase64",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |context| {
                let text = context.get::<String>(0)?;
                Ok(ImageBytes::from_base64(&text)
                    .ok()
                    .map(ImageBytes::into_bytes))
            },
        )?;
        Ok(())
    }

    /// Applies every migration after the first `applied`, each in its own
    /// transaction. One from [`CASCADING_MIGRATIONS`] on runs with foreign
    /// keys off, and a row they refuse rolls it back.
    fn apply(&mut self, applied: usize) -> Result<(), DatabaseError> {
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(applied) {
            let wrap = |error| DatabaseError::Migration {
                name: migration.name,
                error,
            };
            let checked = index >= CASCADING_MIGRATIONS;
            // Only takes effect outside a transaction.
            self.connection
                .pragma_update(None, "foreign_keys", !checked)
                .map_err(wrap)?;
            self.trace_sql(migration.sql);
            let transaction = self.connection.transaction().map_err(wrap)?;
            transaction.execute_batch(migration.sql).map_err(wrap)?;
            let refused = checked
                && transaction
                    .prepare("PRAGMA foreign_key_check")
                    .and_then(|mut check| check.exists([]))
                    .map_err(wrap)?;
            if refused {
                return Err(wrap(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                    Some("a row refers to one the migration removed".to_string()),
                )));
            }
            transaction
                .pragma_update(None, "user_version", (index + 1) as i64)
                .map_err(wrap)?;
            transaction.commit().map_err(wrap)?;
        }
        Ok(())
    }

    /// Runs `VACUUM` if the file's free pages are [`worth_vacuuming`];
    /// returns the bytes they held, or `None` if they were not. Fails inside
    /// a transaction.
    pub(crate) fn vacuum_if_reclaimable(&self) -> Result<Option<i64>, DatabaseError> {
        let size = self.size()?;
        if !worth_vacuuming(size.bytes, size.free_bytes) {
            return Ok(None);
        }
        self.trace_sql("VACUUM");
        self.connection.execute_batch("VACUUM")?;
        Ok(Some(size.free_bytes))
    }

    /// Copies the database, as it stands, to a new private file in a
    /// `backups` directory beside it, named for the time, and returns the
    /// file. Fails inside a transaction, and for a database with no file.
    pub(crate) fn copy_for_backup(&self) -> Result<PathBuf, DatabaseError> {
        let directory = self
            .file()
            .as_deref()
            .and_then(Path::parent)
            .ok_or_else(|| {
                DatabaseError::Storage(std::io::Error::other("the database has no file"))
            })?
            .join("backups");
        create_private_dir(&directory)?;
        let stamp: String =
            self.connection
                .query_row("SELECT strftime('%Y-%m-%d-%H%M%S', 'now')", [], |row| {
                    row.get(0)
                })?;
        let file = directory.join(format!("cohesion-{stamp}.db"));
        // `VACUUM INTO` takes an empty file as it finds it, so the mode holds.
        open_private(&file)?;
        self.trace_sql("VACUUM INTO");
        self.connection
            .execute("VACUUM INTO ?1", [file.to_string_lossy()])?;
        Ok(file)
    }

    /// The database's file; `None` for one in memory.
    pub(crate) fn file(&self) -> Option<PathBuf> {
        self.connection
            .path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }

    /// How large the database file is, how much of it is free pages, and
    /// what each area takes up of the rest.
    pub(crate) fn size(&self) -> Result<DatabaseSize, DatabaseError> {
        let pragma = |name| {
            self.connection
                .pragma_query_value(None, name, |row| row.get::<_, i64>(0))
        };
        let page_size = pragma("page_size")?;
        let bytes = pragma("page_count")? * page_size;
        let free_bytes = pragma("freelist_count")? * page_size;
        self.trace_sql("dbstat");
        Ok(DatabaseSize {
            bytes,
            free_bytes,
            areas: storage_areas::measure(&self.connection, bytes, free_bytes)?,
        })
    }

    /// Runs a statement without rows; returns the number of rows changed.
    /// Only the write builders, which `WriteContainer` alone can start, call this.
    pub(crate) fn execute_statement<'v>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
    ) -> Result<usize, DatabaseError> {
        self.trace_sql(sql);
        self.execute_statement_silently(sql, params)
    }

    /// [`Container::execute_statement`] without tracing the SQL.
    pub(crate) fn execute_statement_silently<'v>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
    ) -> Result<usize, DatabaseError> {
        Ok(self
            .connection
            .execute(sql, params_from_iter(params.into_iter().map(ParamRef)))?)
    }

    /// Every row `sql` returns, read as `M`.
    pub(crate) fn query_all<'v, M: Model>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
    ) -> Result<Vec<M>, DatabaseError> {
        self.query_rows(sql, params, M::from_row)
    }

    /// The `id` of every row `sql` returns: a write's `RETURNING id`.
    pub(crate) fn query_ids<'v>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
    ) -> Result<Vec<i64>, DatabaseError> {
        self.query_rows(sql, params, |row| read_column(row, "id"))
    }

    /// Every row `sql` returns, read by `read`.
    pub(crate) fn query_rows<'v, O>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
        read: impl Fn(&dyn Row) -> Result<O, RowError>,
    ) -> Result<Vec<O>, DatabaseError> {
        self.trace_sql(sql);
        let mut prepared = self.connection.prepare(sql)?;
        let mut rows = prepared.query(params_from_iter(params.into_iter().map(ParamRef)))?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(read(&SqlRow(row))?);
        }
        Ok(result)
    }

    /// The single row `sql` returns, read by `read`, if any; more than one is an error.
    pub(crate) fn query_row<'v, O>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = &'v Value>,
        read: impl Fn(&dyn Row) -> Result<O, RowError>,
    ) -> Result<Option<O>, DatabaseError> {
        self.trace_sql(sql);
        let mut prepared = self.connection.prepare(sql)?;
        let mut rows = prepared.query(params_from_iter(params.into_iter().map(ParamRef)))?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let result = read(&SqlRow(row))?;
        if rows.next()?.is_some() {
            return Err(DatabaseError::Sql(QueryReturnedMoreThanOneRow));
        }
        Ok(Some(result))
    }

    /// The underlying connection, for savepoints and tests.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Records a statement about to run, at Debug level.
    fn trace_sql(&self, sql: &str) {
        cohesion_core::log::debug(Module::Database, format_args!("{}", sql.replace("\n", " ")));
    }
}
