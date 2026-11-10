//! Databases and their properties. A database belongs to a workspace and is
//! deleted for good, with everything in it; its rows and values are in
//! `database_row.rs`.

use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    Database, DatabaseDataType, DatabaseProperty, DatabaseRowValue, DatabaseValue,
};
use cohesion_core::protocol::{
    CreateDatabasePropertyRequest, CreateDatabaseRequest, DeleteDatabasePropertyRequest,
    DeleteDatabaseRequest, GetDatabaseRequest, ListDatabasePropertiesRequest, ListDatabasesRequest,
    MoveDatabasePropertyRequest, RenameDatabasePropertyRequest, RenameDatabaseRequest,
    SetDatabasePropertyTypeRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::database_formula::{ensure_formula, formulas_stale, new_formula};
use crate::commands::database_row::store_value;
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// Rejects an empty property name.
fn ensure_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::in_field(
            "name",
            "Name can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// A database of a live workspace by id; one of another workspace is not
/// found.
#[derive(FromRequest)]
#[request(GetDatabaseRequest)]
pub struct GetDatabaseCommand {
    pub workspace_id: i64,
    pub database_id: i64,
}
impl DatabaseCommand for GetDatabaseCommand {
    type Response = Database;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Database> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select()
            .filter_eq(Database::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Database::ID, self.database_id)
            .one()
            .found()
    }
}

/// The databases of a live workspace, by title ignoring case.
#[derive(FromRequest)]
#[request(ListDatabasesRequest)]
pub struct ListDatabasesCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListDatabasesCommand {
    type Response = Vec<Database>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Database>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select()
            .filter_eq(Database::WORKSPACE_ID, self.workspace_id)
            .order("title COLLATE NOCASE, id")
            .all()?)
    }
}

/// Creates a database with a non-empty title and nothing in it.
#[derive(FromRequest)]
#[request(CreateDatabaseRequest)]
pub struct CreateDatabaseCommand {
    pub workspace_id: i64,
    pub title: String,
}
impl DatabaseCommand for CreateDatabaseCommand {
    type Response = Database;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Database>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let database = ctx
            .db
            .insert::<Database>()
            .set(Database::WORKSPACE_ID, self.workspace_id)
            .set(Database::TITLE, self.title)
            .returning_one()
            .found()?;
        Ok(database.invalidating([Invalidation::Databases(self.workspace_id)]))
    }
}

/// Changes a database's title to a non-empty one.
#[derive(FromRequest)]
#[request(RenameDatabaseRequest)]
pub struct RenameDatabaseCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameDatabaseCommand {
    type Response = Database;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Database>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let database = ctx
            .db
            .update::<Database>()
            .set(Database::TITLE, self.title)
            .filter_eq(Database::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Database::ID, self.database_id)
            .returning_one()
            .found()?;
        Ok(database.invalidating([
            Invalidation::Databases(self.workspace_id),
            Invalidation::Database(self.database_id),
        ]))
    }
}

/// Deletes a database for good. Its properties, rows and values go with it
/// by the schema's cascades, and each row's document by the trigger that
/// removes a row's document with the row.
#[derive(FromRequest)]
#[request(DeleteDatabaseRequest)]
pub struct DeleteDatabaseCommand {
    pub workspace_id: i64,
    pub database_id: i64,
}
impl DatabaseCommand for DeleteDatabaseCommand {
    type Response = Database;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Database>> {
        let database = GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<Database>()
            .filter_eq(Database::ID, database.id)
            .execute()?;
        Ok(database.invalidating([
            Invalidation::Databases(self.workspace_id),
            Invalidation::Database(self.database_id),
            Invalidation::Documents(self.workspace_id),
        ]))
    }
}

/// A database's properties in their order.
#[derive(FromRequest)]
#[request(ListDatabasePropertiesRequest)]
pub struct ListDatabasePropertiesCommand {
    pub workspace_id: i64,
    pub database_id: i64,
}
impl DatabaseCommand for ListDatabasePropertiesCommand {
    type Response = Vec<DatabaseProperty>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<DatabaseProperty>> {
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(ctx)?;
        Ok(ctx
            .db
            .select()
            .filter_eq(DatabaseProperty::DATABASE_ID, self.database_id)
            .order("sort_order, id")
            .all()?)
    }
}

/// One property of a database by id.
pub struct GetDatabasePropertyCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
}
impl DatabaseCommand for GetDatabasePropertyCommand {
    type Response = DatabaseProperty;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<DatabaseProperty> {
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(ctx)?;
        ctx.db
            .select()
            .filter_eq(DatabaseProperty::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseProperty::ID, self.property_id)
            .one()
            .found()
    }
}

/// Adds a property with a non-empty name to a database, after its others:
/// a formula with a non-empty source, or one set by hand. No row holds a
/// value for it until a formula's outputs are computed.
#[derive(FromRequest)]
#[request(CreateDatabasePropertyRequest)]
pub struct CreateDatabasePropertyCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub name: String,
    pub data_type: DatabaseDataType,
    pub formula: Option<String>,
}
impl DatabaseCommand for CreateDatabasePropertyCommand {
    type Response = DatabaseProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseProperty>> {
        ensure_name(&self.name)?;
        ensure_formula(&self.formula)?;
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(&mut ctx.as_read())?;
        let property = ctx
            .db
            .insert::<DatabaseProperty>()
            .set(DatabaseProperty::DATABASE_ID, self.database_id)
            .set(DatabaseProperty::NAME, self.name)
            .set(DatabaseProperty::DATA_TYPE, self.data_type)
            .set_group(new_formula(self.formula))
            .set_max_incremented_within(
                DatabaseProperty::SORT_ORDER,
                DatabaseProperty::DATABASE_ID,
                self.database_id,
            )
            .returning_one()
            .found()?;
        // A formula is computed for every row; any other property is a new
        // key, empty, in the row every formula reads.
        formulas_stale(ctx, self.database_id, None)?;
        Ok(property.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Changes a property's name to a non-empty one.
#[derive(FromRequest)]
#[request(RenameDatabasePropertyRequest)]
pub struct RenameDatabasePropertyCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub name: String,
}
impl DatabaseCommand for RenameDatabasePropertyCommand {
    type Response = DatabaseProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseProperty>> {
        ensure_name(&self.name)?;
        GetDatabaseCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
        }
        .handle(&mut ctx.as_read())?;
        let property = ctx
            .db
            .update::<DatabaseProperty>()
            .set(DatabaseProperty::NAME, self.name)
            .filter_eq(DatabaseProperty::DATABASE_ID, self.database_id)
            .filter_eq(DatabaseProperty::ID, self.property_id)
            .returning_one()
            .found()?;
        // Formulas read the row by property name.
        formulas_stale(ctx, self.database_id, None)?;
        Ok(property.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Changes a property's type. Every value is read again from its text as
/// the new type ([`DatabaseValue::parse`]) and stored as one, in the
/// representations the new type gives; a value whose text isn't one is
/// emptied. Text keeps every value; `"3.5"` doesn't survive becoming an
/// integer. A formula's outputs are then computed again, as the new type.
#[derive(FromRequest)]
#[request(SetDatabasePropertyTypeRequest)]
pub struct SetDatabasePropertyTypeCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub data_type: DatabaseDataType,
}
impl DatabaseCommand for SetDatabasePropertyTypeCommand {
    type Response = DatabaseProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseProperty>> {
        GetDatabasePropertyCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        let property = ctx
            .db
            .update::<DatabaseProperty>()
            .set(DatabaseProperty::DATA_TYPE, self.data_type)
            .filter_eq(DatabaseProperty::ID, self.property_id)
            .returning_one()
            .found()?;
        let values: Vec<DatabaseRowValue> = ctx
            .db
            .select()
            .filter_eq(DatabaseRowValue::PROPERTY_ID, self.property_id)
            .all()?;
        for stored in values {
            let value = DatabaseValue::parse(self.data_type, &stored.text_value);
            store_value(ctx.db, &property, stored.row_id, value)?;
        }
        formulas_stale(ctx, self.database_id, None)?;
        Ok(property.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Moves a property just before its sibling `before_id`, or last, and
/// renumbers the database's properties in their new order.
#[derive(FromRequest)]
#[request(MoveDatabasePropertyRequest)]
pub struct MoveDatabasePropertyCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
    pub before_id: Option<i64>,
}
impl DatabaseCommand for MoveDatabasePropertyCommand {
    type Response = Vec<DatabaseProperty>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<DatabaseProperty>>> {
        let list = |ctx: &mut CommandContext<'_>| {
            ListDatabasePropertiesCommand {
                workspace_id: self.workspace_id,
                database_id: self.database_id,
            }
            .handle(&mut ctx.as_read())
        };
        GetDatabasePropertyCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        let mut order: Vec<i64> = list(ctx)?
            .iter()
            .map(|property| property.id)
            .filter(|&id| id != self.property_id)
            .collect();
        let position = match self.before_id {
            None => order.len(),
            Some(before_id) => order
                .iter()
                .position(|&id| id == before_id)
                .ok_or_else(|| {
                    ValidationError::new("The property to go before isn’t in that database.")
                })?,
        };
        order.insert(position, self.property_id);
        for (sort_order, property_id) in order.into_iter().enumerate() {
            ctx.db
                .update::<DatabaseProperty>()
                .set(DatabaseProperty::SORT_ORDER, sort_order as i64)
                .filter_eq(DatabaseProperty::ID, property_id)
                .execute()?;
        }
        Ok(list(ctx)?.invalidating([Invalidation::Database(self.database_id)]))
    }
}

/// Deletes a property; every row's value for it goes with it by the
/// schema's cascade.
#[derive(FromRequest)]
#[request(DeleteDatabasePropertyRequest)]
pub struct DeleteDatabasePropertyCommand {
    pub workspace_id: i64,
    pub database_id: i64,
    pub property_id: i64,
}
impl DatabaseCommand for DeleteDatabasePropertyCommand {
    type Response = DatabaseProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DatabaseProperty>> {
        let property = GetDatabasePropertyCommand {
            workspace_id: self.workspace_id,
            database_id: self.database_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<DatabaseProperty>()
            .filter_eq(DatabaseProperty::ID, property.id)
            .execute()?;
        formulas_stale(ctx, self.database_id, None)?;
        Ok(property.invalidating([Invalidation::Database(self.database_id)]))
    }
}

#[cfg(test)]
pub(crate) mod tests;
