//! A table's columns in scripts: `DocumentFolderProperty` and the
//! `PropertyOption`s a choice column picks from, and reading and writing a
//! row's values (`Document.values()`, `Document.setValue()`).

use cohesion_core::model::{
    DocumentFolderProperty, FormulaUpdateMode, IntelligenceAgentColor, PropertyDataType,
    PropertyOption, PropertyValue,
};
use cohesion_core::protocol::FormulaDefinition;
use cohesion_db::{
    CreatePropertyOptionCommand, DeleteDocumentFolderPropertyCommand, DeletePropertyOptionCommand,
    FindDocumentFolderCommand, MoveDocumentFolderPropertyCommand, MovePropertyOptionCommand,
    RenameDocumentFolderPropertyCommand, RenamePropertyOptionCommand,
    SetDocumentFolderPropertyFormulaCommand, SetDocumentFolderPropertyTypeCommand,
    SetPropertyOptionColorCommand,
};
use cohesion_macros::{JsOptions, js_entity};
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx, Exception, Value};

use crate::conversion::FromJsField;
use crate::formula::typed_value;

/// A column of a table folder: a name and the type of value it holds, of
/// one `PropertyDataType`, set by hand or computed by a formula.
///
/// Get them from `DocumentFolder.properties()`, in their order, or
/// `DocumentFolder.createProperty()`. Read and write a row's value for one
/// with `Document.values()` and `Document.setValue()`. Every change saves
/// immediately and updates this object.
#[js_entity(DocumentFolderProperty)]
impl JsDocumentFolderProperty {
    /// The type of value the column holds.
    #[qjs(get, rename = "dataType")]
    #[ts(returns = "PropertyDataType")]
    pub fn data_type(&self) -> String {
        self.inner.data_type().as_str().to_string()
    }

    /// A choice column's options, in their order; empty for every other
    /// type.
    #[qjs(get)]
    pub fn options(&self) -> Vec<JsPropertyOption> {
        self.inner
            .kind
            .options()
            .iter()
            .map(|option| JsPropertyOption::new(option.clone(), self.db.clone()))
            .collect()
    }

    /// The function computing the column's values, such as
    /// `(row) => row.Price * row.Quantity`, or `null` for a column set by
    /// hand.
    #[qjs(get)]
    pub fn formula(&self) -> Option<String> {
        self.inner
            .formula
            .as_ref()
            .map(|formula| formula.source.clone())
    }

    /// When a formula's values are computed again, or `null` for a column
    /// set by hand.
    #[qjs(get, rename = "formulaUpdateMode")]
    #[ts(returns = "FormulaUpdateMode | null")]
    pub fn formula_update_mode(&self) -> Option<String> {
        self.inner
            .formula
            .as_ref()
            .map(|formula| formula.update_mode.as_str().to_string())
    }

    /// Why a formula last failed for a row, leaving its cell empty, or
    /// `null` once every row computes.
    #[qjs(get, rename = "formulaError")]
    pub fn formula_error(&self) -> Option<String> {
        self.inner
            .formula
            .as_ref()
            .and_then(|formula| formula.error.clone())
    }

    /// Changes the column's name. An empty name throws.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            RenameDocumentFolderPropertyCommand {
                workspace_id,
                folder_id,
                property_id,
                name,
            },
        )?;
        Ok(())
    }

    /// Changes the type of value the column holds.
    ///
    /// Each value whose text reads as the new type is kept (`"42"` becomes
    /// the number 42); every other value is emptied. Becoming a choice makes
    /// an option of each distinct value; a choice's options are deleted when
    /// it becomes another type. A formula can't become a choice.
    #[qjs(rename = "setDataType")]
    pub fn set_data_type<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "PropertyDataType")] data_type: Value<'js>,
    ) -> rquickjs::Result<()> {
        let data_type = PropertyDataType::from_js_field(&ctx, data_type)?;
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            SetDocumentFolderPropertyTypeCommand {
                workspace_id,
                folder_id,
                property_id,
                data_type,
            },
        )?;
        Ok(())
    }

    /// Makes the column a formula computing its values with `source`, a
    /// function of the row such as `(row) => row.Price * row.Quantity`, or
    /// one set by hand again when `null`, keeping the last values.
    ///
    /// `updateMode` is `"whenChanged"` when left out. A column becoming a
    /// formula loses the values set by hand; a choice can't be one. The row
    /// maps each column that isn't a formula, by name, to its value.
    #[qjs(rename = "setFormula")]
    pub fn set_formula<'js>(
        &mut self,
        ctx: Ctx<'js>,
        source: Option<String>,
        #[ts(type = "FormulaUpdateMode")] update_mode: Opt<Value<'js>>,
    ) -> rquickjs::Result<()> {
        let update_mode = match update_mode.0 {
            Some(mode) => Option::<FormulaUpdateMode>::from_js_field(&ctx, mode)?,
            None => None,
        };
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            SetDocumentFolderPropertyFormulaCommand {
                workspace_id,
                folder_id,
                property_id,
                formula: source.map(|source| FormulaDefinition {
                    source,
                    update_mode: update_mode.unwrap_or(FormulaUpdateMode::WhenChanged),
                }),
            },
        )?;
        Ok(())
    }

    /// Moves the column just before `property`, another of its folder's, or
    /// last when `null`.
    #[qjs(rename = "moveBefore")]
    pub fn move_before<'js>(
        &mut self,
        ctx: Ctx<'js>,
        property: Option<Class<'js, JsDocumentFolderProperty>>,
    ) -> rquickjs::Result<()> {
        let before_id = property.map(|property| property.borrow().inner().id);
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        let properties = self.write(
            &ctx,
            MoveDocumentFolderPropertyCommand {
                workspace_id,
                folder_id,
                property_id,
                before_id,
            },
        )?;
        if let Some(moved) = properties.into_iter().find(|p| p.id == property_id) {
            self.inner = moved;
        }
        Ok(())
    }

    /// Deletes the column, with every row's value for it. The folder still
    /// shows as a table.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.write(
            &ctx,
            DeleteDocumentFolderPropertyCommand {
                workspace_id,
                folder_id,
                property_id,
            },
        )?;
        Ok(())
    }

    /// Adds an option to a choice column, after its others, in `color` (none
    /// when left out), and returns it.
    ///
    /// A name that is empty or another option's, ignoring case, throws, as
    /// does a column that isn't a choice.
    #[qjs(rename = "addOption")]
    pub fn add_option<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
        #[ts(type = "IntelligenceAgentColor | null")] color: Opt<Value<'js>>,
    ) -> rquickjs::Result<JsPropertyOption> {
        let color = color_of(&ctx, color)?;
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        let before: Vec<i64> = self.inner.kind.options().iter().map(|o| o.id).collect();
        self.inner = self.write(
            &ctx,
            CreatePropertyOptionCommand {
                workspace_id,
                folder_id,
                property_id,
                name,
                color,
            },
        )?;
        let added = self
            .inner
            .kind
            .options()
            .iter()
            .find(|option| !before.contains(&option.id))
            .cloned()
            .ok_or_else(|| Exception::throw_internal(&ctx, "The option wasn’t added."))?;
        Ok(JsPropertyOption::new(added, self.db.clone()))
    }

    /// Renames `option`, one of this choice column's, by name or id; the
    /// values that hold it show the new name.
    #[qjs(rename = "renameOption")]
    pub fn rename_option<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "string | number")] option: Value<'js>,
        name: String,
    ) -> rquickjs::Result<()> {
        let option_id = self.option_id(&ctx, option)?;
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            RenamePropertyOptionCommand {
                workspace_id,
                folder_id,
                property_id,
                option_id,
                name,
            },
        )?;
        Ok(())
    }

    /// Changes the color of `option`, by name or id, or takes it away when
    /// `null`.
    #[qjs(rename = "setOptionColor")]
    pub fn set_option_color<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "string | number")] option: Value<'js>,
        #[ts(type = "IntelligenceAgentColor | null")] color: Value<'js>,
    ) -> rquickjs::Result<()> {
        let option_id = self.option_id(&ctx, option)?;
        let color = color_of(&ctx, Opt(Some(color)))?;
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            SetPropertyOptionColorCommand {
                workspace_id,
                folder_id,
                property_id,
                option_id,
                color,
            },
        )?;
        Ok(())
    }

    /// Moves `option`, by name or id, just before the option `before`, or
    /// last when `null`.
    #[qjs(rename = "moveOption")]
    pub fn move_option<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "string | number")] option: Value<'js>,
        #[ts(type = "string | number | null")] before: Value<'js>,
    ) -> rquickjs::Result<()> {
        let option_id = self.option_id(&ctx, option)?;
        let before_id = if before.is_null() || before.is_undefined() {
            None
        } else {
            Some(self.option_id(&ctx, before)?)
        };
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            MovePropertyOptionCommand {
                workspace_id,
                folder_id,
                property_id,
                option_id,
                before_id,
            },
        )?;
        Ok(())
    }

    /// Deletes `option`, by name or id; the values that held it are emptied.
    #[qjs(rename = "deleteOption")]
    pub fn delete_option<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "string | number")] option: Value<'js>,
    ) -> rquickjs::Result<()> {
        let option_id = self.option_id(&ctx, option)?;
        let (workspace_id, folder_id, property_id) = self.keys(&ctx)?;
        self.inner = self.write(
            &ctx,
            DeletePropertyOptionCommand {
                workspace_id,
                folder_id,
                property_id,
                option_id,
            },
        )?;
        Ok(())
    }
}

impl JsDocumentFolderProperty {
    /// The workspace, folder and id every command on the column takes. A
    /// column knows its folder but not its workspace, so this reads it.
    fn keys(&self, ctx: &Ctx<'_>) -> rquickjs::Result<(i64, i64, i64)> {
        let folder = self.read(
            ctx,
            FindDocumentFolderCommand {
                folder_id: self.inner.folder_id,
            },
        )?;
        Ok((folder.workspace_id, folder.id, self.inner.id))
    }

    /// The id of the option `option` names: a number is its id, a string
    /// its name, ignoring case. Any other option throws a `TypeError`.
    fn option_id<'js>(&self, ctx: &Ctx<'js>, option: Value<'js>) -> rquickjs::Result<i64> {
        find_option(ctx, &self.inner, option).map(|option| option.id)
    }
}

/// The option of `property` that `option` names, by id (a number) or name
/// (a string, ignoring case); anything else throws a `TypeError`.
fn find_option<'a, 'js>(
    ctx: &Ctx<'js>,
    property: &'a DocumentFolderProperty,
    option: Value<'js>,
) -> rquickjs::Result<&'a PropertyOption> {
    let found = if let Some(id) = option.as_number() {
        property.kind.options().iter().find(|o| o.id as f64 == id)
    } else if let Some(name) = option.as_string() {
        let name = name.to_string()?.to_lowercase();
        property
            .kind
            .options()
            .iter()
            .find(|o| o.name.to_lowercase() == name)
    } else {
        None
    };
    found.ok_or_else(|| {
        Exception::throw_type(ctx, &format!("“{}” has no such option.", property.name))
    })
}

/// A color argument: `null`, `undefined` or left out is none.
fn color_of<'js>(
    ctx: &Ctx<'js>,
    color: Opt<Value<'js>>,
) -> rquickjs::Result<Option<IntelligenceAgentColor>> {
    match color.0 {
        Some(color) => Option::<IntelligenceAgentColor>::from_js_field(ctx, color),
        None => Ok(None),
    }
}

/// `value`, a script's value for a row's cell in `property`, as the value
/// to store: `null` or `undefined` empties it, a choice names its option by
/// name or id, and every other type takes its plain JavaScript value (see
/// `Document.setValue()`). A value of another type throws a `TypeError`.
pub(crate) fn property_value<'js>(
    ctx: &Ctx<'js>,
    property: &DocumentFolderProperty,
    value: Value<'js>,
) -> rquickjs::Result<Option<PropertyValue>> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let data_type = property.data_type();
    if data_type == PropertyDataType::Choice {
        let option = find_option(ctx, property, value)?;
        return Ok(Some(PropertyValue::Choice(option.id)));
    }
    typed_value(&value, data_type).map(Some).ok_or_else(|| {
        Exception::throw_type(
            ctx,
            &format!("“{}” takes {}.", property.name, expected(data_type)),
        )
    })
}

/// What a script sets a cell of `data_type` to, in words.
fn expected(data_type: PropertyDataType) -> &'static str {
    match data_type {
        PropertyDataType::Text => "text",
        PropertyDataType::Integer => "a whole number",
        PropertyDataType::Decimal => "a number",
        PropertyDataType::Currency => "an amount, as a number",
        PropertyDataType::Checkbox => "true or false",
        PropertyDataType::Date => "a date, as YYYY-MM-DD",
        PropertyDataType::Choice => "one of its options, by name",
    }
}

/// One of the options a choice column's values pick from: a name, unique
/// ignoring case among its column's, and a color.
///
/// Get them from `DocumentFolderProperty.options`; change them with that
/// column's `addOption()`, `renameOption()`, `setOptionColor()`,
/// `moveOption()` and `deleteOption()`.
#[js_entity(PropertyOption)]
impl JsPropertyOption {}

/// Options for `DocumentFolder.createProperty()`.
#[derive(JsOptions, Default)]
pub struct PropertyCreateOptions {
    /// A choice column's first options, by name, in order
    /// (`{ options: ["To read", "Read"] }`); only a choice takes any.
    pub options: Option<Vec<String>>,
    /// A function of the row computing the column's values, such as
    /// `"(row) => row.Price * row.Quantity"`; set by hand when left out. A
    /// choice is never a formula.
    pub formula: Option<String>,
    /// When the formula's values are computed again; `"whenChanged"` when
    /// left out.
    pub update_mode: Option<FormulaUpdateMode>,
}
