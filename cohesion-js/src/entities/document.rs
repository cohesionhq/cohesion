//! `Document` in scripts.

use cohesion_core::Document;
use cohesion_core::model::VersionTarget;
use cohesion_db::{AppendToDocumentCommand, ListVersionsCommand, commands::EditDocumentCommand};
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use super::rewind::JsVersionSummary;

/// A piece of text in a workspace: either a normal document or a daily journal
/// entry.
///
/// Get them from `Workspace.documents()`, `Workspace.journal()`,
/// `Workspace.getJournal()`, or `SearchResult.open()`.
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
}
