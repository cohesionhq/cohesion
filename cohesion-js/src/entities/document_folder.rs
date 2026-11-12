//! `DocumentFolder` in scripts.

use cohesion_core::DocumentFolder;
use cohesion_core::model::{DocumentFolderRepresentation, FormulaUpdateMode, PropertyDataType};
use cohesion_core::protocol::{FormulaDefinition, NewPropertyOption};
use cohesion_db::{
    CreateDocumentCommand, CreateDocumentFolderCommand, CreateDocumentFolderPropertyCommand,
    DeleteDocumentFolderCommand, GetDocumentFolderCommand, ListDocumentFolderPropertiesCommand,
    ListDocumentFoldersCommand, ListFiledDocumentsCommand, MoveDocumentFolderCommand,
    RenameDocumentFolderCommand, SetDocumentFolderRepresentationCommand,
};
use cohesion_macros::js_entity;
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx, Value};

use super::document::JsDocument;
use super::document_folder_property::{JsDocumentFolderProperty, PropertyCreateOptions};
use crate::conversion::FromJsField;

/// A named group of documents in a workspace, which may sit inside another
/// folder (`parentId`); folders nest without limit.
///
/// Get them from `Workspace.documentFolders()`, `Workspace.documentFolder(id)`
/// or `Document.folder()`. A document in no folder is in the Inbox; journals
/// are never in a folder.
///
/// A folder is also a table: its `properties()` are the columns and its
/// documents the rows, whose values `Document.values()` reads and
/// `Document.setValue()` writes. `representation` says whether the apps
/// show it as a list or a table.
#[js_entity(DocumentFolder)]
impl JsDocumentFolder {
    /// Assigning saves immediately, e.g.
    /// `folder.representation = DocumentFolderRepresentation.table` (or the
    /// string `"table"`). Its columns and values stay either way.
    ///
    /// Any other value throws a `TypeError`.
    #[qjs(set, rename = "representation")]
    pub fn set_representation<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "DocumentFolderRepresentation")] representation: Value<'js>,
    ) -> rquickjs::Result<()> {
        let representation = DocumentFolderRepresentation::from_js_field(&ctx, representation)?;
        self.inner = self.write(
            &ctx,
            SetDocumentFolderRepresentationCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
                representation,
            },
        )?;
        Ok(())
    }

    /// The folder's table columns, in their order; empty when it has none.
    pub fn properties<'js>(
        &self,
        ctx: Ctx<'js>,
    ) -> rquickjs::Result<Vec<JsDocumentFolderProperty>> {
        Ok(self
            .read(
                &ctx,
                ListDocumentFolderPropertiesCommand {
                    workspace_id: self.inner.workspace_id,
                    folder_id: self.inner.id,
                },
            )?
            .into_iter()
            .map(|property| JsDocumentFolderProperty::new(property, self.db.clone()))
            .collect())
    }

    /// Adds a column named `name` holding values of `dataType`, after the
    /// others, and returns it, e.g.
    /// `folder.createProperty("Status", "choice", { options: ["To do", "Done"] })`.
    ///
    /// Saved immediately; a folder showing as a list shows as a table from
    /// then on. An empty name throws, as do options on a column that isn't
    /// a choice and a choice that is a formula.
    #[qjs(rename = "createProperty")]
    pub fn create_property<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: String,
        #[ts(type = "PropertyDataType")] data_type: Value<'js>,
        options: Opt<PropertyCreateOptions>,
    ) -> rquickjs::Result<JsDocumentFolderProperty> {
        let data_type = PropertyDataType::from_js_field(&ctx, data_type)?;
        let options = options.0.unwrap_or_default();
        let property = self.write(
            &ctx,
            CreateDocumentFolderPropertyCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
                name,
                data_type,
                formula: options.formula.map(|source| FormulaDefinition {
                    source,
                    update_mode: options
                        .update_mode
                        .unwrap_or(FormulaUpdateMode::WhenChanged),
                }),
                options: options
                    .options
                    .unwrap_or_default()
                    .into_iter()
                    .map(|name| NewPropertyOption { name, color: None })
                    .collect(),
            },
        )?;
        // The folder may have just become a table.
        self.inner = self.read(
            &ctx,
            GetDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
            },
        )?;
        Ok(JsDocumentFolderProperty::new(property, self.db.clone()))
    }

    /// The live documents filed directly in this folder, not in the folders
    /// inside it, most recently modified first.
    pub fn documents<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsDocument>> {
        Ok(self
            .read(
                &ctx,
                ListFiledDocumentsCommand {
                    workspace_id: self.inner.workspace_id,
                    folder_id: Some(self.inner.id),
                },
            )?
            .into_iter()
            .map(|document| JsDocument::new(document, self.db.clone()))
            .collect())
    }

    /// The folders directly inside this one, in their order.
    pub fn folders<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsDocumentFolder>> {
        Ok(self
            .read(
                &ctx,
                ListDocumentFoldersCommand {
                    workspace_id: self.inner.workspace_id,
                },
            )?
            .into_iter()
            .filter(|folder| folder.parent_id == Some(self.inner.id))
            .map(|folder| JsDocumentFolder::new(folder, self.db.clone()))
            .collect())
    }

    /// The folder this one sits in, or `null` at the top level.
    pub fn parent<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Option<JsDocumentFolder>> {
        let Some(folder_id) = self.inner.parent_id else {
            return Ok(None);
        };
        let parent = self.read(
            &ctx,
            GetDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id,
            },
        )?;
        Ok(Some(JsDocumentFolder::new(parent, self.db.clone())))
    }

    /// Creates a document titled `title` in this folder, starting with
    /// `contents` (empty when left out), and returns it.
    ///
    /// Saved immediately. An empty title throws.
    #[qjs(rename = "createDocument")]
    pub fn create_document<'js>(
        &self,
        ctx: Ctx<'js>,
        title: String,
        contents: Opt<String>,
    ) -> rquickjs::Result<JsDocument> {
        let document = self.write(
            &ctx,
            CreateDocumentCommand {
                workspace_id: self.inner.workspace_id,
                title,
                contents: contents.0.unwrap_or_default(),
                folder_id: Some(self.inner.id),
            },
        )?;
        Ok(JsDocument::new(document.into(), self.db.clone()))
    }

    /// Creates a folder titled `title` inside this one, last among its
    /// folders, and returns it.
    ///
    /// Saved immediately. An empty title throws.
    #[qjs(rename = "createFolder")]
    pub fn create_folder<'js>(
        &self,
        ctx: Ctx<'js>,
        title: String,
    ) -> rquickjs::Result<JsDocumentFolder> {
        let folder = self.write(
            &ctx,
            CreateDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                title,
                parent_id: Some(self.inner.id),
            },
        )?;
        Ok(JsDocumentFolder::new(folder, self.db.clone()))
    }

    /// Changes the folder's title. Saved immediately; an empty title throws.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// Moves this folder, with everything in it, into `folder`, or to the
    /// top level when `null`, last among its new siblings.
    ///
    /// Saved immediately. Moving a folder into itself or a folder inside it
    /// throws.
    #[qjs(rename = "moveTo")]
    pub fn move_to<'js>(
        &mut self,
        ctx: Ctx<'js>,
        folder: Option<Class<'js, JsDocumentFolder>>,
    ) -> rquickjs::Result<()> {
        let folders = self.write(
            &ctx,
            MoveDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
                parent_id: folder.map(|folder| folder.borrow().inner().id),
                before_id: None,
            },
        )?;
        if let Some(moved) = folders.into_iter().find(|f| f.id == self.inner.id) {
            self.inner = moved;
        }
        Ok(())
    }

    /// Deletes this folder and every folder inside it; the documents they
    /// held move to the Inbox.
    ///
    /// Saved immediately.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteDocumentFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}
