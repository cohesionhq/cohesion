//! Rewind's kept versions in scripts: read-only rows of a task's or document's
//! history, from `Task.versions()` and `Document.versions()`. A script can
//! read a version and restore from it, never change or remove one.

use cohesion_core::model::{RestoredItem, Version, VersionSummary};
use cohesion_db::{GetVersionCommand, RestoreVersionCommand};
use cohesion_macros::js_entity;
use rquickjs::{Ctx, IntoJs};

use super::document::JsDocument;
use super::task::JsTask;

/// A row of a task's or document's history: when it changed and who changed
/// it, without the text.
///
/// Get them from `Task.versions()` or `Document.versions()`, newest first, and
/// read one's text with `open()`. Versions last 7 days by default, and nothing
/// can change or remove one.
#[js_entity(VersionSummary)]
impl JsVersionSummary {
    /// Loads this version's title and text.
    pub fn open<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<JsVersion> {
        let version = self.read(
            &ctx,
            GetVersionCommand {
                workspace_id: self.inner.workspace_id,
                version_id: self.inner.id,
            },
        )?;
        Ok(JsVersion::new(version, self.db.clone()))
    }
}

/// A task's or document's title and text at one moment, and a task's status.
///
/// Get one from `VersionSummary.open()`.
#[js_entity(Version)]
impl JsVersion {
    /// Puts the item back to this version, bringing it back if it was deleted,
    /// and returns it.
    ///
    /// Saved immediately, as a new version, so the user can undo it. A task's
    /// folder, tags, priority, size, days and assignee stay as they are; a
    /// deleted task whose folder is gone comes back to the inbox. Throws for a
    /// deleted journal entry whose day has a new entry.
    #[ts(returns = "Task | Document")]
    pub fn restore<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let restored = self.write(
            &ctx,
            RestoreVersionCommand {
                workspace_id: self.inner.workspace_id,
                version_id: self.inner.id,
            },
        )?;
        match restored {
            RestoredItem::Task { task } => JsTask::new(task, self.db.clone()).into_js(&ctx),
            RestoredItem::Document { document } => {
                JsDocument::new(document.into(), self.db.clone()).into_js(&ctx)
            }
        }
    }
}
