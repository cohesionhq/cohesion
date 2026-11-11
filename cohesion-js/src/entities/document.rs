//! `Document` in scripts.

use cohesion_core::Document;
use cohesion_core::model::VersionTarget;
use cohesion_db::{
    AppendToDocumentCommand, GetDocumentFolderCommand, ListVersionsCommand, MoveDocumentCommand,
    commands::{DeleteDocumentCommand, EditDocumentCommand, RenameDocumentCommand},
};
use cohesion_macros::js_entity;
use rquickjs::{Class, Ctx};

use super::document_folder::JsDocumentFolder;
use super::rewind::JsVersionSummary;

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
