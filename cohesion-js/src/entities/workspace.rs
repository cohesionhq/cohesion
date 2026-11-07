//! `Workspace` in scripts. `cohesion.workspaces()` and
//! `cohesion.workspace(id)` live on the `cohesion` global.

use rquickjs::Ctx;
use rquickjs::function::Opt;

use cohesion_core::Workspace;
use cohesion_core::model::{ResourceSort, TagFilter, VersionedItem};
use cohesion_db::{
    CreateContactCommand, CreateDocumentCommand, CreateTagCommand, CreateTaskCommand,
    CreateTaskFolderCommand, DeleteWorkspaceCommand, EnsureJournalCommand, GetContactCommand,
    GetDocumentCommand, GetDocumentIncludingDeletedCommand, GetTaskCommand, GetTaskFolderCommand,
    GetTaskIncludingDeletedCommand, ListContactsCommand, ListDeletedItemsCommand,
    ListDocumentsCommand, ListResourcesCommand, ListTagsCommand, ListTaskFoldersCommand,
    ListTasksCommand, LookupJournalByDateCommand, RenameWorkspaceCommand, SearchWorkspaceCommand,
    SetWorkspaceEmojiCommand,
};
use cohesion_macros::js_entity;

use super::contact::JsContact;
use super::document::JsDocument;
use super::resource::JsResource;
use super::search_result::JsSearchResult;
use super::tag::{JsTag, find_tag};
use super::task::JsTask;
use super::task_folder::JsTaskFolder;
use super::workspace_options::{
    ContactCreateOptions, DocumentListOptions, ResourceListOptions, TaskCreateOptions,
    TaskListOptions,
};
use crate::conversion::OneOrMore;
use crate::entities::workspace_options::{TaskFolderCreateOptions, TaskFolderListOptions};

/// A top-level container for documents, tasks and resources.
///
/// Get one from `cohesion.workspaces()` or `cohesion.workspace(idOrTitle)`.
///
/// List tasks with `tasks()`, read one by id with `task(id)`, find them by
/// title with `search()`, and make new ones with `createTask()`. Read a
/// document or journal entry by id with `document(id)`, and make documents
/// with `createDocument()`.
/// Read items imported from connected services with `resources()`. Keep people
/// with `contacts()` and `createContact()`.
#[js_entity(Workspace, metadata)]
impl JsWorkspace {
    /// Soft-deletes the workspace; it disappears from listings and lookups.
    ///
    /// Saved immediately. This object's `deleted` property is not updated.
    pub fn delete<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.write(&ctx, DeleteWorkspaceCommand { id: self.inner.id })?;
        Ok(())
    }

    /// Changes the title.
    ///
    /// Saved immediately, and this object's `title` updates to match.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameWorkspaceCommand {
                id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// Assigning saves immediately, e.g. `workspace.emoji = "🌱"`; `null` or
    /// `""` clears it.
    ///
    /// More than a single emoji throws.
    #[qjs(set, rename = "emoji")]
    pub fn set_emoji<'js>(&mut self, ctx: Ctx<'js>, emoji: Option<String>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            SetWorkspaceEmojiCommand {
                id: self.inner.id,
                emoji,
            },
        )?;
        Ok(())
    }

    /// Every document in the workspace that has not been deleted, including
    /// past journal entries (check `documentType`), most recently modified
    /// first.
    pub fn documents<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<DocumentListOptions>,
    ) -> rquickjs::Result<Vec<JsDocument>> {
        let options = options.0.unwrap_or_default();
        let documents = self.read(
            &ctx,
            ListDocumentsCommand {
                workspace_id: self.inner.id,
                document_type: None,
                journal_from: None,
                journal_to: None,
                metadata: options.metadata.unwrap_or_default(),
                has_metadata: options
                    .has_metadata
                    .map(OneOrMore::into_vec)
                    .unwrap_or_default(),
            },
        )?;
        Ok(documents
            .into_iter()
            .map(|document| JsDocument::new(document, self.db.clone()))
            .collect())
    }

    /// Resources imported into this workspace from connected services, most
    /// recently updated first.
    pub fn resources<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<ResourceListOptions>,
    ) -> rquickjs::Result<Vec<JsResource>> {
        let options = options.0.unwrap_or_default();
        let resources = self.read(
            &ctx,
            ListResourcesCommand {
                workspace_id: self.inner.id,
                kind: options.kind,
                search: options.search,
                sort: ResourceSort::LastUpdated,
                descending: true,
            },
        )?;
        Ok(resources
            .into_iter()
            .map(|resource| JsResource::new(resource, self.db.clone()))
            .collect())
    }

    /// The contacts in this workspace that have not been deleted, by last
    /// name, then first name, ignoring case.
    pub fn contacts<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsContact>> {
        let contacts = self.read(
            &ctx,
            ListContactsCommand {
                workspace_id: self.inner.id,
            },
        )?;
        Ok(contacts
            .into_iter()
            .map(|contact| JsContact::new(contact, self.db.clone()))
            .collect())
    }

    /// One of this workspace's contacts, by id.
    ///
    /// Throws a `ReferenceError` if this workspace has no live contact with
    /// that id.
    pub fn contact<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsContact> {
        let contact = self.read(
            &ctx,
            GetContactCommand {
                workspace_id: self.inner.id,
                contact_id: id,
            },
        )?;
        Ok(JsContact::new(contact, self.db.clone()))
    }

    /// Adds a contact to this workspace and returns it.
    ///
    /// Saved immediately, with its `methods` in their order. An empty first
    /// or last name, or a method with an empty `value`, throws and saves
    /// nothing. Not available in read-only contexts.
    #[qjs(rename = "createContact")]
    pub fn create_contact<'js>(
        &self,
        ctx: Ctx<'js>,
        options: ContactCreateOptions,
    ) -> rquickjs::Result<JsContact> {
        let contact = self.write(
            &ctx,
            CreateContactCommand {
                workspace_id: self.inner.id,
                first_name: options.first_name,
                middle_name: options.middle_name,
                last_name: options.last_name,
                notes: options.notes.unwrap_or_default(),
                methods: options.methods.unwrap_or_default(),
            },
        )?;
        Ok(JsContact::new(contact, self.db.clone()))
    }

    /// The journal entry for `date` (`"2026-10-20"`), or today's (by the
    /// daemon's local date) when left out, or `null` if nothing was written
    /// that day.
    ///
    /// Only reads, so it works without write access.
    #[qjs(rename = "getJournal")]
    pub fn get_journal<'js>(
        &self,
        ctx: Ctx<'js>,
        date: Opt<String>,
    ) -> rquickjs::Result<Option<JsDocument>> {
        let document = self.read(
            &ctx,
            LookupJournalByDateCommand {
                workspace_id: self.inner.id,
                date: journal_date(date),
            },
        )?;
        Ok(document.map(|document| JsDocument::new(document, self.db.clone())))
    }

    /// Creates the journal entry for a date if it is missing and returns it,
    /// so it needs write access even for an existing entry; use `getJournal`
    /// to read one.
    ///
    /// `date` is `"2026-10-20"`, or today when left out; any other text
    /// throws. To add to the entry, read `contents` and assign the new text
    /// back.
    pub fn journal<'js>(&self, ctx: Ctx<'js>, date: Opt<String>) -> rquickjs::Result<JsDocument> {
        let document = self.write(
            &ctx,
            EnsureJournalCommand {
                workspace_id: self.inner.id,
                date: journal_date(date),
            },
        )?;
        Ok(JsDocument::new(document, self.db.clone()))
    }

    /// The deleted tasks Rewind can still bring back, most recently deleted
    /// first. Bring one back by restoring one of its `versions()`.
    #[qjs(rename = "deletedTasks")]
    pub fn deleted_tasks<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsTask>> {
        let deleted = self.read(
            &ctx,
            ListDeletedItemsCommand {
                workspace_id: self.inner.id,
            },
        )?;
        deleted
            .into_iter()
            .filter_map(|deleted| match deleted.item {
                VersionedItem::Task { task_id, .. } => Some(task_id),
                VersionedItem::Document { .. } => None,
            })
            .map(|task_id| {
                let task = self.read(
                    &ctx,
                    GetTaskIncludingDeletedCommand {
                        workspace_id: self.inner.id,
                        task_id,
                    },
                )?;
                Ok(JsTask::new(task, self.db.clone()))
            })
            .collect()
    }

    /// The deleted documents and journal entries Rewind can still bring back,
    /// most recently deleted first. Bring one back by restoring one of its
    /// `versions()`.
    #[qjs(rename = "deletedDocuments")]
    pub fn deleted_documents<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsDocument>> {
        let deleted = self.read(
            &ctx,
            ListDeletedItemsCommand {
                workspace_id: self.inner.id,
            },
        )?;
        deleted
            .into_iter()
            .filter_map(|deleted| match deleted.item {
                VersionedItem::Document { document_id } => Some(document_id),
                VersionedItem::Task { .. } => None,
            })
            .map(|document_id| {
                let document = self.read(
                    &ctx,
                    GetDocumentIncludingDeletedCommand {
                        workspace_id: self.inner.id,
                        document_id,
                    },
                )?;
                Ok(JsDocument::new(document, self.db.clone()))
            })
            .collect()
    }

    /// Full-text search over this workspace's documents (title and contents),
    /// tasks (title) and resources (title, and contents for Markdown resources; a
    /// JSON resource is found by its title only). Articles aren't a workspace's:
    /// `cohesion.searchCorpus()` searches them.
    ///
    /// Words are stemmed and case/accent-insensitive, so `"meeting"` also finds
    /// "Meetings".
    ///
    /// `query` uses SQLite FTS5 syntax: space-separated words must all match
    /// (`"budget 2025"`), `OR` between words matches either, a trailing `*`
    /// matches a prefix (`"proj*"`), and double quotes match an exact phrase
    /// (`'"weekly review"'`). Punctuation such as `-`, `:`, `.` or an
    /// unbalanced quote is query syntax and makes the search throw a
    /// validation error ("The search query is not valid"), so quote search text
    /// containing it (`'"re-auth"'`).
    ///
    /// Results come best match first, a title match ahead of a mention in a
    /// document's text, and may mix documents, tasks and resources; call
    /// `open()` on a result to get the full `Document`, `Task` or `Resource`.
    pub fn search<'js>(
        &self,
        ctx: Ctx<'js>,
        query: String,
    ) -> rquickjs::Result<Vec<JsSearchResult>> {
        let results = self.read(
            &ctx,
            SearchWorkspaceCommand {
                workspace_id: self.inner.id,
                query,
            },
        )?;
        Ok(results
            .into_iter()
            .map(|x| JsSearchResult::new(x, self.db.clone()))
            .collect())
    }

    /// The workspace's tags, which it puts on its tasks, by name.
    pub fn tags<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsTag>> {
        let tags = self.read(
            &ctx,
            ListTagsCommand {
                workspace_id: self.inner.id,
            },
        )?;
        Ok(tags
            .into_iter()
            .map(|tag| JsTag::new(tag, self.db.clone()))
            .collect())
    }

    /// Adds a tag and returns it.
    ///
    /// Saved immediately. An empty name, or one the workspace already has
    /// (ignoring case), throws. Not available in read-only contexts.
    #[qjs(rename = "createTag")]
    pub fn create_tag<'js>(&self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<JsTag> {
        let tag = self.write(
            &ctx,
            CreateTagCommand {
                workspace_id: self.inner.id,
                name,
            },
        )?;
        Ok(JsTag::new(tag, self.db.clone()))
    }

    /// Every task in the workspace that has not been deleted, across all
    /// folders, in creation order unless `sort` says otherwise.
    ///
    /// Without a `status` option, only `"incomplete"` tasks.
    #[qjs(rename = "tasks")]
    pub fn list_tasks<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<TaskListOptions>,
    ) -> rquickjs::Result<Vec<JsTask>> {
        let options = options.0.unwrap_or_default();
        let folder = options.folder_filter();
        let tags = match &options.tag {
            Some(name) => match find_tag(&ctx, &self.db, self.inner.id, name)? {
                Some(tag) => TagFilter {
                    with: vec![tag.id],
                    without: Vec::new(),
                },
                None => return Ok(Vec::new()),
            },
            None => TagFilter::default(),
        };
        let result = self.read(
            &ctx,
            ListTasksCommand(options.into_listing(Some(self.inner.id), folder, tags)),
        )?;
        Ok(result
            .into_iter()
            .map(|task| JsTask::new(task, self.db.clone()))
            .collect())
    }

    /// Creates a new task with this title and status `"incomplete"`, in
    /// `options.folder` or else in no folder.
    ///
    /// Saved immediately; returns the new task.
    #[qjs(rename = "createTask")]
    pub fn create_task<'js>(
        &self,
        ctx: Ctx<'js>,
        options: TaskCreateOptions,
    ) -> rquickjs::Result<JsTask> {
        let result = self.write(
            &ctx,
            CreateTaskCommand {
                workspace_id: self.inner.id,
                title: options.title,
                folder_id: options.folder.map(|folder| folder.inner().id),
                priority: options.priority,
                size: options.size,
                assignee: options.assignee,
                description: options.description,
                start_date: options.start_date,
                due_date: options.due_date,
                tag_ids: Vec::new(),
            },
        )?;
        Ok(JsTask::new(result, self.db.clone()))
    }

    /// Creates a document titled `title` in this workspace, starting with
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
        let result = self.write(
            &ctx,
            CreateDocumentCommand {
                workspace_id: self.inner.id,
                title,
                contents: contents.0.unwrap_or_default(),
            },
        )?;
        Ok(JsDocument::new(result, self.db.clone()))
    }

    #[qjs(rename = "createTaskFolder")]
    pub fn create_task_folder<'js>(
        &self,
        ctx: Ctx<'js>,
        opt: TaskFolderCreateOptions,
    ) -> rquickjs::Result<JsTaskFolder> {
        let result = self.write(
            &ctx,
            CreateTaskFolderCommand {
                workspace_id: self.inner.id,
                title: opt.title,
            },
        )?;
        Ok(JsTaskFolder::new(result, self.db.clone()))
    }

    /// One of this workspace's task folders, by id, such as a `Task`'s
    /// `taskFolderId`.
    ///
    /// Reads only that folder, so prefer it to searching `taskFolders()`.
    /// Throws a `ReferenceError` if this workspace has no live folder with
    /// that id.
    #[qjs(rename = "taskFolder")]
    pub fn task_folder<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsTaskFolder> {
        let folder = self.read(
            &ctx,
            GetTaskFolderCommand {
                workspace_id: self.inner.id,
                folder_id: id,
            },
        )?;
        Ok(JsTaskFolder::new(folder, self.db.clone()))
    }

    /// One of this workspace's tasks, by id, in any status.
    ///
    /// Reads only that task, so prefer it to filtering `tasks()`. Throws a
    /// `ReferenceError` if this workspace has no live task with that id.
    pub fn task<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsTask> {
        let task = self.read(
            &ctx,
            GetTaskCommand {
                workspace_id: self.inner.id,
                task_id: id,
            },
        )?;
        Ok(JsTask::new(task, self.db.clone()))
    }

    /// One of this workspace's documents or journal entries, by id.
    ///
    /// Reads only that document, so prefer it to searching `documents()`.
    /// Throws a `ReferenceError` if this workspace has no live document with
    /// that id.
    pub fn document<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsDocument> {
        let document = self.read(
            &ctx,
            GetDocumentCommand {
                workspace_id: self.inner.id,
                document_id: id,
            },
        )?;
        Ok(JsDocument::new(document, self.db.clone()))
    }

    #[qjs(rename = "taskFolders")]
    pub fn list_task_folders<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<TaskFolderListOptions>,
    ) -> rquickjs::Result<Vec<JsTaskFolder>> {
        let options = options.0.unwrap_or_default();
        let result = self.read(
            &ctx,
            ListTaskFoldersCommand {
                workspace_id: self.inner.id,
                metadata: options.metadata.unwrap_or_default(),
                has_metadata: options
                    .has_metadata
                    .map(OneOrMore::into_vec)
                    .unwrap_or_default(),
            },
        )?;
        Ok(result
            .into_iter()
            .map(|folder| JsTaskFolder::new(folder, self.db.clone()))
            .collect())
    }
}

/// `date`, or the daemon's local date today when left out.
fn journal_date(date: Opt<String>) -> String {
    date.0
        .unwrap_or_else(|| jiff::Zoned::now().date().to_string())
}
