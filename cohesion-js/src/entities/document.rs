//! `Document` in scripts.

use cohesion_core::Document;
use cohesion_core::model::{DocumentFolderProperty, PropertyValue, VersionTarget};
use cohesion_db::{
    AppendToDocumentCommand, GetDocumentFolderCommand, GetTableRowCommand,
    ListDocumentFolderPropertiesCommand, ListVersionsCommand, MoveDocumentCommand,
    SetDocumentValueCommand,
    commands::{DeleteDocumentCommand, EditDocumentCommand, RenameDocumentCommand},
};
use cohesion_macros::js_entity;
use rquickjs::{Class, Ctx, Exception, FromJs, Object, Value};

use super::document_folder::JsDocumentFolder;
use super::document_folder_property::{JsDocumentFolderProperty, property_value};
use super::rewind::JsVersionSummary;
use crate::formula::{FormulaRow, row_object};

/// A piece of text in a workspace: either a normal document or a daily journal
/// entry.
///
/// Get them from `Workspace.documents()`, `Workspace.journal()`,
/// `Workspace.getJournal()`, `DocumentFolder.documents()`, or
/// `SearchResult.open()`. A normal document is filed in a `DocumentFolder`
/// (`folderId`) or, when `folderId` is `null`, in the Inbox.
///
/// Link one in a message as `[Launch plan](cohesion://document/3/7)`, its
/// title, `workspaceId` and `id`; the user opens it with a tap.
#[js_entity(Document, metadata)]
impl JsDocument {
    /// The first 500 characters of `contents`, ending in `…` when there is
    /// more. Use it to tell items apart without reading each one in full.
    pub fn excerpt(&self) -> String {
        super::excerpt(&self.inner.contents)
    }
    /// Assigning replaces the whole text and saves immediately; `modified`
    /// updates too.
    #[qjs(set, rename = "contents")]
    pub fn set_contents<'js>(&mut self, ctx: Ctx<'js>, contents: String) -> rquickjs::Result<()> {
        let updated = self.write(
            &ctx,
            EditDocumentCommand {
                workspace_id: self.inner.workspace_id,
                document_id: self.inner.id,
                contents,
            },
        )?;
        self.inner = updated;
        Ok(())
    }
    /// This document's kept versions, newest first: what it held, when, and who
    /// changed it. Restoring one undoes changes; a deleted document has them
    /// too.
    pub fn versions<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsVersionSummary>> {
        let versions = self.read(
            &ctx,
            ListVersionsCommand {
                workspace_id: self.inner.workspace_id,
                item: VersionTarget::Document {
                    document_id: self.inner.id,
                },
            },
        )?;
        Ok(versions
            .into_iter()
            .map(|version| JsVersionSummary::new(version, self.db.clone()))
            .collect())
    }
    /// Adds `contents` to the end of the current text exactly as given (include
    /// a leading `"\n"` to start a new line) and saves immediately; `modified`
    /// updates too.
    pub fn append<'js>(&mut self, ctx: Ctx<'js>, contents: String) -> rquickjs::Result<()> {
        let updated = self.write(
            &ctx,
            AppendToDocumentCommand {
                workspace_id: self.inner.workspace_id,
                document_id: self.inner.id,
                contents,
            },
        )?;
        self.inner = updated;
        Ok(())
    }
    /// The folder this document is filed in, or `null` when it is in the
    /// Inbox (always for a journal).
    pub fn folder<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Option<JsDocumentFolder>> {
        let Some(folder_id) = self.inner.folder_id else {
            return Ok(None);
        };
        let folder = self.read(
            &ctx,
            GetDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id,
            },
        )?;
        Ok(Some(JsDocumentFolder::new(folder, self.db.clone())))
    }
    /// The document's values in its folder's table, by column name, e.g.
    /// `{ Pages: 412, Status: "Read", Due: null }`: text and dates
    /// (`"YYYY-MM-DD"`) as strings, whole numbers, decimals and amounts as
    /// numbers, a checkbox as a boolean, a choice as its option's name, and
    /// `null` when empty.
    ///
    /// Every column of the folder is there, formulas too; a document in the
    /// Inbox or a folder with no columns has none.
    #[ts(returns = "Record<string, string | number | boolean | null>")]
    pub fn values<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        let Some(folder_id) = self.inner.folder_id else {
            return Object::new(ctx);
        };
        let properties = self.properties(&ctx, folder_id)?;
        let row = self.read(
            &ctx,
            GetTableRowCommand {
                workspace_id: self.inner.workspace_id,
                folder_id,
                document_id: self.inner.id,
            },
        )?;
        let values: FormulaRow = properties
            .iter()
            .map(|property| {
                let value = row
                    .cells
                    .iter()
                    .find(|cell| cell.property_id == property.id)
                    .and_then(|cell| match &cell.value {
                        PropertyValue::Choice(option_id) => property
                            .option(*option_id)
                            .map(|option| PropertyValue::Text(option.name.clone())),
                        value => Some(value.clone()),
                    });
                (property.name.clone(), value)
            })
            .collect();
        row_object(&ctx, &values)
    }

    /// Sets the document's value for `property`, a column of its folder by
    /// name or as a `DocumentFolderProperty`, e.g.
    /// `doc.setValue("Pages", 412)`; `null` empties it.
    ///
    /// Saved immediately. The value is as `values()` reads it: a string for
    /// text and dates (`"YYYY-MM-DD"`), a number, a boolean for a checkbox,
    /// and an option's name (or id) for a choice. A value of another type, a
    /// column the folder doesn't have, and a formula's column throw.
    #[qjs(rename = "setValue")]
    pub fn set_value<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "string | DocumentFolderProperty")] property: Value<'js>,
        #[ts(type = "string | number | boolean | null")] value: Value<'js>,
    ) -> rquickjs::Result<()> {
        let Some(folder_id) = self.inner.folder_id else {
            return Err(Exception::throw_type(
                &ctx,
                "This document isn’t in a folder, so it has no columns.",
            ));
        };
        let properties = self.properties(&ctx, folder_id)?;
        let found = match Class::<JsDocumentFolderProperty>::from_js(&ctx, property.clone()) {
            Ok(property) => {
                let id = property.borrow().inner().id;
                properties.iter().find(|p| p.id == id)
            }
            Err(_) => {
                let name = String::from_js(&ctx, property)?;
                properties.iter().find(|p| p.name == name)
            }
        };
        let property = found.ok_or_else(|| {
            Exception::throw_type(&ctx, "The document’s folder has no such column.")
        })?;
        let value = property_value(&ctx, property, value)?;
        self.write(
            &ctx,
            SetDocumentValueCommand {
                workspace_id: self.inner.workspace_id,
                folder_id,
                document_id: self.inner.id,
                property_id: property.id,
                value,
            },
        )?;
        Ok(())
    }
    /// Files this document in `folder`, from the same workspace, or in the
    /// Inbox when `null`.
    ///
    /// Saved immediately. Filing a journal throws.
    #[qjs(rename = "moveTo")]
    pub fn move_to<'js>(
        &mut self,
        ctx: Ctx<'js>,
        folder: Option<Class<'js, JsDocumentFolder>>,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            MoveDocumentCommand {
                workspace_id: self.inner.workspace_id,
                document_id: self.inner.id,
                folder_id: folder.map(|folder| folder.borrow().inner().id),
            },
        )?;
        Ok(())
    }
    /// Changes the document's title. Saved immediately; `modified` updates
    /// too. An empty title throws.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameDocumentCommand {
                workspace_id: self.inner.workspace_id,
                document_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }
    /// Deletes this document; it disappears from listings and `deleted`
    /// becomes `true`.
    ///
    /// Saved immediately. Rewind can bring it back, except a database row's
    /// document, which is removed for good.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteDocumentCommand {
                workspace_id: self.inner.workspace_id,
                document_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

impl JsDocument {
    /// The columns of folder `folder_id`, this document's, in their order.
    fn properties(
        &self,
        ctx: &Ctx<'_>,
        folder_id: i64,
    ) -> rquickjs::Result<Vec<DocumentFolderProperty>> {
        self.read(
            ctx,
            ListDocumentFolderPropertiesCommand {
                workspace_id: self.inner.workspace_id,
                folder_id,
            },
        )
    }
}
