//! A document folder's properties: the columns that make it a table. A
//! folder has none until one is created, and is a plain folder again once
//! its last is deleted; the Inbox never has any. Its documents' values are
//! in `document_value.rs`.

use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DocumentFolder, DocumentFolderProperty, DocumentValue, PropertyDataType, PropertyOption,
    PropertyValue,
};
use cohesion_core::protocol::{
    CreateDocumentFolderPropertyRequest, DeleteDocumentFolderPropertyRequest, FormulaDefinition,
    ListDocumentFolderPropertiesRequest, MoveDocumentFolderPropertyRequest, NewPropertyOption,
    RenameDocumentFolderPropertyRequest, SetDocumentFolderPropertyTypeRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::document_folder::GetDocumentFolderCommand;
use crate::commands::document_formula::{ensure_formula, new_formula};
use crate::commands::document_table::{TableChange, store_value, table_changed};
use crate::commands::property_option::{add_options, ensure_not_formula_choice};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// Rejects an empty property or option name.
pub(crate) fn ensure_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::in_field(
            "name",
            "Name can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// The live folder whose properties a command reads or changes; a deleted
/// one, or one of another workspace, is not found.
fn folder(
    ctx: &mut CommandContext<'_, Read>,
    workspace_id: i64,
    folder_id: i64,
) -> DatabaseCommandResult<DocumentFolder> {
    GetDocumentFolderCommand {
        workspace_id,
        folder_id,
    }
    .handle(ctx)
}

/// A live document folder's properties in their order; none for a folder
/// that isn't a table.
#[derive(FromRequest)]
#[request(ListDocumentFolderPropertiesRequest)]
pub struct ListDocumentFolderPropertiesCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for ListDocumentFolderPropertiesCommand {
    type Response = Vec<DocumentFolderProperty>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<DocumentFolderProperty>> {
        folder(ctx, self.workspace_id, self.folder_id)?;
        Ok(ctx
            .db
            .select()
            .filter_eq(DocumentFolderProperty::FOLDER_ID, self.folder_id)
            .order("sort_order, id")
            .all()?)
    }
}

/// One property of a live folder by id.
pub struct GetDocumentFolderPropertyCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
}
impl DatabaseCommand for GetDocumentFolderPropertyCommand {
    type Response = DocumentFolderProperty;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<DocumentFolderProperty> {
        folder(ctx, self.workspace_id, self.folder_id)?;
        ctx.db
            .select()
            .filter_eq(DocumentFolderProperty::FOLDER_ID, self.folder_id)
            .filter_eq(DocumentFolderProperty::ID, self.property_id)
            .one()
            .found()
    }
}

/// Adds a property with a non-empty name to a live folder, after its
/// others: a formula with a non-empty source and its update mode, or one
/// set by hand. A choice property, never a formula, starts with `options`
/// in their order; no other type takes any. No
/// document holds a value for it until a formula's outputs are computed. A
/// folder's first property makes it a table, which its folder list shows.
#[derive(FromRequest)]
#[request(CreateDocumentFolderPropertyRequest)]
pub struct CreateDocumentFolderPropertyCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub name: String,
    pub data_type: PropertyDataType,
    pub formula: Option<FormulaDefinition>,
    pub options: Vec<NewPropertyOption>,
}
impl DatabaseCommand for CreateDocumentFolderPropertyCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        ensure_name(&self.name)?;
        ensure_formula(&self.formula)?;
        ensure_not_formula_choice(self.data_type, self.formula.is_some())?;
        if self.data_type != PropertyDataType::Choice && !self.options.is_empty() {
            return Err(ValidationError::in_field(
                "options",
                "Only a choice property has options.".to_string(),
            )
            .into());
        }
        folder(&mut ctx.as_read(), self.workspace_id, self.folder_id)?;
        let property = ctx
            .db
            .insert::<DocumentFolderProperty>()
            .set(DocumentFolderProperty::FOLDER_ID, self.folder_id)
            .set(DocumentFolderProperty::NAME, self.name)
            .set(DocumentFolderProperty::DATA_TYPE, self.data_type)
            .set_group(new_formula(self.formula))
            .set_max_incremented_within(
                DocumentFolderProperty::SORT_ORDER,
                DocumentFolderProperty::FOLDER_ID,
                self.folder_id,
            )
            .returning_one()
            .found()?;
        let property = if self.options.is_empty() {
            property
        } else {
            add_options(ctx, property, self.options)?
        };
        // A formula is computed for every document; any other property is a
        // new key, empty, in the row every formula reads.
        let table = table_changed(ctx, self.folder_id, TableChange::Inputs)?;
        Ok(property.invalidating([table, Invalidation::DocumentFolders(self.workspace_id)]))
    }
}

/// Changes a property's name to a non-empty one.
#[derive(FromRequest)]
#[request(RenameDocumentFolderPropertyRequest)]
pub struct RenameDocumentFolderPropertyCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub name: String,
}
impl DatabaseCommand for RenameDocumentFolderPropertyCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        ensure_name(&self.name)?;
        folder(&mut ctx.as_read(), self.workspace_id, self.folder_id)?;
        let property = ctx
            .db
            .update::<DocumentFolderProperty>()
            .set(DocumentFolderProperty::NAME, self.name)
            .filter_eq(DocumentFolderProperty::FOLDER_ID, self.folder_id)
            .filter_eq(DocumentFolderProperty::ID, self.property_id)
            .returning_one()
            .found()?;
        // Formulas read the row by property name.
        let table = table_changed(ctx, self.folder_id, TableChange::Inputs)?;
        Ok(property.invalidating([table]))
    }
}

/// Changes a property's type. Every value is read again from its text as
/// the new type ([`PropertyValue::parse`]) and stored as one, in the
/// representations the new type gives; a value whose text isn't one is
/// emptied. Text keeps every value; `"3.5"` doesn't survive becoming an
/// integer. A choice's text is its option's name, and a property that stops
/// being a choice loses its options. One becoming a choice gets an option
/// for each distinct text, ignoring case, in alphabetical order and with no
/// color, and each value names its text's; a formula can't become one. A
/// formula's outputs are then computed again, as the new type.
#[derive(FromRequest)]
#[request(SetDocumentFolderPropertyTypeRequest)]
pub struct SetDocumentFolderPropertyTypeCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub data_type: PropertyDataType,
}
impl DatabaseCommand for SetDocumentFolderPropertyTypeCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let before = GetDocumentFolderPropertyCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        ensure_not_formula_choice(self.data_type, before.formula.is_some())?;
        if before.data_type() == self.data_type {
            return Ok(before.invalidating_nothing());
        }
        // Each value as text: a choice's is its option's name.
        let texts: Vec<(i64, String)> = ctx
            .db
            .select::<DocumentValue>()
            .filter_eq(DocumentValue::PROPERTY_ID, self.property_id)
            .order("id")
            .all()?
            .into_iter()
            .map(|stored| {
                let text = match stored.option_id {
                    Some(option_id) => before
                        .option(option_id)
                        .map(|option| option.name.clone())
                        .unwrap_or_default(),
                    None => stored.text_value,
                };
                (stored.document_id, text)
            })
            .collect();
        // An option keeps to a choice property, so they go before the type.
        ctx.db
            .delete::<PropertyOption>()
            .filter_eq(PropertyOption::PROPERTY_ID, self.property_id)
            .execute()?;
        let property = ctx
            .db
            .update::<DocumentFolderProperty>()
            .set(DocumentFolderProperty::DATA_TYPE, self.data_type)
            .filter_eq(DocumentFolderProperty::ID, self.property_id)
            .returning_one()
            .found()?;
        let property = if self.data_type == PropertyDataType::Choice {
            choices_from_texts(ctx, property, texts)?
        } else {
            for (document_id, text) in texts {
                let value = PropertyValue::parse(self.data_type, &text);
                store_value(ctx.db, &property, document_id, value)?;
            }
            property
        };
        let table = table_changed(ctx, self.folder_id, TableChange::Inputs)?;
        Ok(property.invalidating([table]))
    }
}

/// Makes `property`, just turned a choice, an option of each distinct
/// non-empty one of `texts`, its documents' values as text, ignoring case
/// (the first in alphabetical order names it), in alphabetical order and with no
/// color, and stores each document's value as its text's option. Returns
/// the property with its options.
fn choices_from_texts(
    ctx: &mut CommandContext<'_>,
    property: DocumentFolderProperty,
    texts: Vec<(i64, String)>,
) -> DatabaseCommandResult<DocumentFolderProperty> {
    let mut names: Vec<&str> = texts
        .iter()
        .map(|(_, text)| text.as_str())
        .filter(|text| !text.is_empty())
        .collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    names.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
    let options = names
        .into_iter()
        .map(|name| NewPropertyOption {
            name: name.to_string(),
            color: None,
        })
        .collect();
    let property = add_options(ctx, property, options)?;
    for (document_id, text) in &texts {
        let option = property
            .kind
            .options()
            .iter()
            .find(|option| option.name.to_lowercase() == text.to_lowercase());
        let value = option.map(|option| PropertyValue::Choice(option.id));
        store_value(ctx.db, &property, *document_id, value)?;
    }
    Ok(property)
}

/// Moves a property just before its sibling `before_id`, or last, and
/// renumbers the folder's properties in their new order.
#[derive(FromRequest)]
#[request(MoveDocumentFolderPropertyRequest)]
pub struct MoveDocumentFolderPropertyCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub before_id: Option<i64>,
}
impl DatabaseCommand for MoveDocumentFolderPropertyCommand {
    type Response = Vec<DocumentFolderProperty>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<DocumentFolderProperty>>> {
        let list = |ctx: &mut CommandContext<'_>| {
            ListDocumentFolderPropertiesCommand {
                workspace_id: self.workspace_id,
                folder_id: self.folder_id,
            }
            .handle(&mut ctx.as_read())
        };
        GetDocumentFolderPropertyCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
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
                    ValidationError::new("The property to go before isn’t in that folder.")
                })?,
        };
        order.insert(position, self.property_id);
        for (sort_order, property_id) in order.into_iter().enumerate() {
            ctx.db
                .update::<DocumentFolderProperty>()
                .set(DocumentFolderProperty::SORT_ORDER, sort_order as i64)
                .filter_eq(DocumentFolderProperty::ID, property_id)
                .execute()?;
        }
        let table = table_changed(ctx, self.folder_id, TableChange::Shown)?;
        Ok(list(ctx)?.invalidating([table]))
    }
}

/// Deletes a property; every document's value for it goes with it by the
/// schema's cascade. Deleting a folder's last property makes it a plain
/// folder again, which its folder list shows.
#[derive(FromRequest)]
#[request(DeleteDocumentFolderPropertyRequest)]
pub struct DeleteDocumentFolderPropertyCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
}
impl DatabaseCommand for DeleteDocumentFolderPropertyCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = GetDocumentFolderPropertyCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
            property_id: self.property_id,
        }
        .handle(&mut ctx.as_read())?;
        // Its options go by the schema's cascade, and its values with them.
        ctx.db
            .delete::<DocumentFolderProperty>()
            .filter_eq(DocumentFolderProperty::ID, property.id)
            .execute()?;
        let table = table_changed(ctx, self.folder_id, TableChange::Inputs)?;
        Ok(property.invalidating([table, Invalidation::DocumentFolders(self.workspace_id)]))
    }
}

#[cfg(test)]
pub(crate) mod tests;
