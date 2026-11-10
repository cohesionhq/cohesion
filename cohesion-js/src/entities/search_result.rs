//! `SearchResult` in scripts.

use cohesion_core::model::{SearchEntityKind, SearchResult};
use cohesion_db::{GetContactCommand, GetDocumentCommand, GetResourceCommand, GetTaskCommand};
use cohesion_macros::js_entity;
use rquickjs::{Ctx, IntoJs, Value};

use super::contact::JsContact;
use super::document::JsDocument;
use super::resource::JsResource;
use super::task::JsTask;

/// One hit from `Workspace.search()`.
///
/// Use `entityKind` and `entityId` to tell what matched, and `open()` to load
/// it.
#[js_entity(SearchResult)]
impl JsSearchResult {
    /// Loads the item this result points at: a `Document` when `entityKind` is
    /// `"document"`, a `Task` when it is `"task"`, a `Resource` when it is
    /// `"resource"`, a `Contact` when it is `"contact"`.
    ///
    /// Throws if the item no longer exists.
    #[ts(returns = "Task | Document | Resource | Contact")]
    pub fn open<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        match self.inner.entity_kind {
            SearchEntityKind::Task => {
                let task = self.read(
                    &ctx,
                    GetTaskCommand {
                        workspace_id: self.inner.workspace_id,
                        task_id: self.inner.entity_id,
                    },
                )?;
                JsTask::new(task, self.db.clone()).into_js(&ctx)
            }
            SearchEntityKind::Document => {
                let doc = self.read(
                    &ctx,
                    GetDocumentCommand {
                        workspace_id: self.inner.workspace_id,
                        document_id: self.inner.entity_id,
                    },
                )?;
                JsDocument::new(doc, self.db.clone()).into_js(&ctx)
            }
            SearchEntityKind::Resource => {
                let resource = self.read(
                    &ctx,
                    GetResourceCommand {
                        workspace_id: self.inner.workspace_id,
                        resource_id: self.inner.entity_id,
                    },
                )?;
                JsResource::new(resource, self.db.clone()).into_js(&ctx)
            }
            SearchEntityKind::Contact => {
                let contact = self.read(
                    &ctx,
                    GetContactCommand {
                        workspace_id: self.inner.workspace_id,
                        contact_id: self.inner.entity_id,
                    },
                )?;
                JsContact::new(contact, self.db.clone()).into_js(&ctx)
            }
        }
    }
}
