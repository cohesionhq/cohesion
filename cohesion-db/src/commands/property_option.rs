//! A choice property's options: what its values pick from. Only a choice
//! property has them (the schema keys an option to its property's type), a
//! value names its option by id, and the schema holds that option to the
//! value's own property, so renaming an option changes no value and
//! deleting one empties the values that held it. Every command replies
//! with the property and its options.

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DocumentFolderProperty, IntelligenceAgentColor, PropertyDataType, PropertyOption,
};
use cohesion_core::protocol::{
    CreatePropertyOptionRequest, DeletePropertyOptionRequest, MovePropertyOptionRequest,
    NewPropertyOption, RenamePropertyOptionRequest, SetPropertyOptionColorRequest,
};
use cohesion_core::{FromRequest, Named};

use crate::changed::{Changed, Invalidates};
use crate::commands::document_folder_property::{GetDocumentFolderPropertyCommand, ensure_name};
use crate::commands::document_formula::formulas_stale;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    ReadableDatabase, ValidationError, Write,
};

/// Refuses a choice property that would be a formula: a formula computes
/// values, and a choice's options are picked by hand.
pub(crate) fn ensure_not_formula_choice(
    data_type: PropertyDataType,
    formula: bool,
) -> Result<(), ValidationError> {
    if data_type == PropertyDataType::Choice && formula {
        return Err(ValidationError::in_field(
            "data_type",
            "A formula can’t be a choice.".to_string(),
        ));
    }
    Ok(())
}

/// Refuses `name` for an option of `property` when it is empty or another
/// of its options, other than `except`, has it, ignoring case.
fn ensure_option_name(
    property: &DocumentFolderProperty,
    name: &str,
    except: Option<i64>,
) -> Result<(), ValidationError> {
    ensure_name(name)?;
    let taken = property.kind.options().iter().any(|option| {
        Some(option.id) != except && option.name.to_lowercase() == name.to_lowercase()
    });
    if taken {
        return Err(ValidationError::in_field(
            "name",
            format!("This column already has an option named “{name}”."),
        ));
    }
    Ok(())
}

/// Reads choice property `property_id` of a live folder; another type's
/// options can't be changed, since it has none.
fn choice_property(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    folder_id: i64,
    property_id: i64,
) -> DatabaseCommandResult<DocumentFolderProperty> {
    let property = GetDocumentFolderPropertyCommand {
        workspace_id,
        folder_id,
        property_id,
    }
    .handle(&mut ctx.as_read())?;
    if property.data_type() != PropertyDataType::Choice {
        return Err(ValidationError::in_field(
            "property_id",
            "Only a choice property has options.".to_string(),
        )
        .into());
    }
    Ok(property)
}

/// `property` read again, with its options as they now are.
fn reread(
    ctx: &mut CommandContext<'_>,
    property: &DocumentFolderProperty,
) -> DatabaseCommandResult<DocumentFolderProperty> {
    ctx.db
        .select()
        .filter_eq(DocumentFolderProperty::ID, property.id)
        .one()
        .found()
}

/// Adds `options` to choice `property`, after its others and in their
/// order, each with a non-empty name no other of its options has, ignoring
/// case. Returns the property with its options.
pub(crate) fn add_options(
    ctx: &mut CommandContext<'_>,
    property: DocumentFolderProperty,
    options: Vec<NewPropertyOption>,
) -> DatabaseCommandResult<DocumentFolderProperty> {
    let mut property = property;
    for option in options {
        ensure_option_name(&property, &option.name, None)?;
        ctx.db
            .insert::<PropertyOption>()
            .set(PropertyOption::PROPERTY_ID, property.id)
            .set(PropertyOption::NAME, option.name)
            .set(PropertyOption::COLOR, option.color)
            .set_max_incremented_within(
                PropertyOption::SORT_ORDER,
                PropertyOption::PROPERTY_ID,
                property.id,
            )
            .execute()?;
        property = reread(ctx, &property)?;
    }
    Ok(property)
}

/// The option `option_id` of choice `property`; another property's is not
/// found.
fn option_of(
    property: &DocumentFolderProperty,
    option_id: i64,
) -> DatabaseCommandResult<&PropertyOption> {
    property
        .option(option_id)
        .ok_or(CommandError::NotFound(PropertyOption::NOUN))
}

/// Adds an option with a non-empty name, unique ignoring case among its
/// property's, to a choice property, after its others.
#[derive(FromRequest)]
#[request(CreatePropertyOptionRequest)]
pub struct CreatePropertyOptionCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub name: String,
    pub color: Option<IntelligenceAgentColor>,
}
impl DatabaseCommand for CreatePropertyOptionCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = choice_property(ctx, self.workspace_id, self.folder_id, self.property_id)?;
        let property = add_options(
            ctx,
            property,
            vec![NewPropertyOption {
                name: self.name,
                color: self.color,
            }],
        )?;
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// Renames an option to a non-empty name no other of its property's has,
/// ignoring case. Its values name it by id, so they show the new name; a
/// formula reads a choice by name, so the folder's formulas are computed
/// again.
#[derive(FromRequest)]
#[request(RenamePropertyOptionRequest)]
pub struct RenamePropertyOptionCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub name: String,
}
impl DatabaseCommand for RenamePropertyOptionCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = choice_property(ctx, self.workspace_id, self.folder_id, self.property_id)?;
        option_of(&property, self.option_id)?;
        ensure_option_name(&property, &self.name, Some(self.option_id))?;
        ctx.db
            .update::<PropertyOption>()
            .set(PropertyOption::NAME, self.name)
            .filter_eq(PropertyOption::ID, self.option_id)
            .execute()?;
        formulas_stale(ctx, self.folder_id, None)?;
        let property = reread(ctx, &property)?;
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// Changes an option's color, or takes it away.
#[derive(FromRequest)]
#[request(SetPropertyOptionColorRequest)]
pub struct SetPropertyOptionColorCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub color: Option<IntelligenceAgentColor>,
}
impl DatabaseCommand for SetPropertyOptionColorCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = choice_property(ctx, self.workspace_id, self.folder_id, self.property_id)?;
        option_of(&property, self.option_id)?;
        ctx.db
            .update::<PropertyOption>()
            .set(PropertyOption::COLOR, self.color)
            .filter_eq(PropertyOption::ID, self.option_id)
            .execute()?;
        let property = reread(ctx, &property)?;
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// Moves an option just before its sibling `before_id`, or last, and
/// renumbers its property's options in their new order, which a sort on
/// the property follows.
#[derive(FromRequest)]
#[request(MovePropertyOptionRequest)]
pub struct MovePropertyOptionCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
    pub before_id: Option<i64>,
}
impl DatabaseCommand for MovePropertyOptionCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = choice_property(ctx, self.workspace_id, self.folder_id, self.property_id)?;
        option_of(&property, self.option_id)?;
        let mut order: Vec<i64> = property
            .kind
            .options()
            .iter()
            .map(|option| option.id)
            .filter(|&id| id != self.option_id)
            .collect();
        let position = match self.before_id {
            None => order.len(),
            Some(before_id) => order
                .iter()
                .position(|&id| id == before_id)
                .ok_or_else(|| {
                    ValidationError::new("The option to go before isn’t in that column.")
                })?,
        };
        order.insert(position, self.option_id);
        for (sort_order, option_id) in order.into_iter().enumerate() {
            ctx.db
                .update::<PropertyOption>()
                .set(PropertyOption::SORT_ORDER, sort_order as i64)
                .filter_eq(PropertyOption::ID, option_id)
                .execute()?;
        }
        let property = reread(ctx, &property)?;
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

/// Deletes an option; the values that held it go with it by the schema's
/// cascade, leaving those documents empty for the property, and the
/// folder's formulas are computed again.
#[derive(FromRequest)]
#[request(DeletePropertyOptionRequest)]
pub struct DeletePropertyOptionCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub property_id: i64,
    pub option_id: i64,
}
impl DatabaseCommand for DeletePropertyOptionCommand {
    type Response = DocumentFolderProperty;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolderProperty>> {
        let property = choice_property(ctx, self.workspace_id, self.folder_id, self.property_id)?;
        option_of(&property, self.option_id)?;
        ctx.db
            .delete::<PropertyOption>()
            .filter_eq(PropertyOption::ID, self.option_id)
            .execute()?;
        formulas_stale(ctx, self.folder_id, None)?;
        let property = reread(ctx, &property)?;
        Ok(property.invalidating([Invalidation::DocumentTable(self.folder_id)]))
    }
}

#[cfg(test)]
mod tests;
