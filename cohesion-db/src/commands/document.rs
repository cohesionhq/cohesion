use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::protocol::{
    AppendToJournalRequest, CreateDocumentRequest, DeleteDocumentRequest, EditDocumentRequest,
    EditJournalRequest, GetDocumentRequest, GetJournalRequest, ListDocumentsRequest,
    RenameDocumentRequest, SearchDocumentRequest,
};
use cohesion_core::{Document, DocumentType, FromRequest};

use crate::changed::{Changed, Invalidates};
use crate::commands::backlink::FollowLinksCommand;
use crate::commands::document_folder::GetDocumentFolderCommand;
use crate::commands::document_formula::formulas_stale;
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write, WriteContainer,
};

/// The journal for `date`, a `YYYY-MM-DD`; the column's CHECK rejects
/// anything else when a new journal is stored.
fn find_or_create_journal(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    date: String,
) -> DatabaseCommandResult<Changed<Document>> {
    let existing: Option<Document> = ctx
        .db
        .select_live()
        .filter("workspace_id = ? AND document_type = ? AND journal_date = ?")
        .bind(workspace_id)
        .bind(DocumentType::Journal)
        .bind(date.clone())
        .one()?;
    match existing {
        Some(document) => Ok(document.invalidating_nothing()),
        None => {
            let journal = create_journal(ctx.db, workspace_id, date)?;
            Ok(journal.invalidating([Invalidation::Documents(workspace_id)]))
        }
    }
}

/// Invalidates `document`'s workspace and raises `JournalSaved` for a
/// journal and `DocumentSaved` for an ordinary document.
pub(crate) fn saved(
    ctx: &mut CommandContext<'_>,
    document: Document,
) -> DatabaseCommandResult<Changed<Document>> {
    let document_id = document.id;
    let workspace_id = document.workspace_id;
    let event = match document.document_type {
        DocumentType::Journal => DatabaseEventKind::JournalSaved {
            document_id,
            workspace_id,
        },
        DocumentType::Normal => DatabaseEventKind::DocumentSaved {
            document_id,
            workspace_id,
        },
    };
    ctx.events.send(ctx.trace.event(event));
    changed(ctx, document)
}

/// Invalidates the workspace `document`, just written, belongs to, and
/// keeps what it links to ([`FollowLinksCommand`]). Every write of a
/// document ends here.
pub(crate) fn changed(
    ctx: &mut CommandContext<'_>,
    document: Document,
) -> DatabaseCommandResult<Changed<Document>> {
    ctx.run(FollowLinksCommand::of(&document))?;
    let key = Invalidation::Documents(document.workspace_id);
    Ok(document.invalidating([key]))
}

/// The table `document` is a row of, as its folder's key, which a document
/// coming, going, being renamed or deleted changes; `None` in the Inbox or
/// for a journal. Every folder has the key, a table or not.
pub(crate) fn table_of(document: &Document) -> Option<Invalidation> {
    document.folder_id.map(Invalidation::DocumentTable)
}

fn create_journal(
    db: WriteContainer<'_>,
    workspace_id: i64,
    date: String,
) -> DatabaseCommandResult<Document> {
    db.insert()
        .set(Document::WORKSPACE_ID, workspace_id)
        .set(Document::TITLE, date.clone())
        .set(Document::CONTENTS, "")
        .set(Document::DOCUMENT_TYPE, DocumentType::Journal)
        .set(Document::JOURNAL_DATE, date)
        .returning_one()
        .found()
}

/// Live documents in a workspace, most recently modified first, optionally
/// only of `document_type`, only journals dated in
/// `journal_from..journal_to`, and only those carrying every one of the
/// `metadata` pairs and `has_metadata` keys. Documents in a table are
/// listed like any other; a client that keeps them apart reads which
/// folders are tables from `DocumentFolder::is_table`.
#[derive(FromRequest)]
#[request(ListDocumentsRequest)]
pub struct ListDocumentsCommand {
    pub workspace_id: i64,
    pub document_type: Option<DocumentType>,
    pub journal_from: Option<String>,
    pub journal_to: Option<String>,
    pub metadata: HashMap<String, String>,
    pub has_metadata: Vec<String>,
}
impl DatabaseCommand for ListDocumentsCommand {
    type Response = Vec<Document>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Document>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let mut query = ctx
            .db
            .select_live::<Document>()
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .with_metadata(self.metadata)
            .with_metadata_keys(self.has_metadata);
        if let Some(document_type) = self.document_type {
            query = query.filter("document_type = ?").bind(document_type);
        }
        if let Some(from) = self.journal_from {
            query = query.filter("journal_date >= ?").bind(from);
        }
        if let Some(to) = self.journal_to {
            query = query.filter("journal_date < ?").bind(to);
        }
        Ok(query.order("modified DESC").all()?)
    }
}

/// Creates an ordinary document with a non-empty title in a workspace, filed
/// in a live folder of it or in the Inbox. In a table it is a new row, with
/// no values until its formulas are computed.
#[derive(FromRequest)]
#[request(CreateDocumentRequest)]
pub struct CreateDocumentCommand {
    pub workspace_id: i64,
    pub title: String,
    pub contents: String,
    pub folder_id: Option<i64>,
}
impl DatabaseCommand for CreateDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        if let Some(folder_id) = self.folder_id {
            GetDocumentFolderCommand {
                workspace_id: self.workspace_id,
                folder_id,
            }
            .handle(&mut ctx.as_read())?;
        }
        let document = ctx
            .db
            .insert()
            .set(Document::WORKSPACE_ID, self.workspace_id)
            .set(Document::TITLE, self.title)
            .set(Document::CONTENTS, self.contents)
            .set(Document::DOCUMENT_TYPE, DocumentType::Normal)
            .set(Document::FOLDER_ID, self.folder_id)
            .returning_one()
            .found()?;
        if let Some(folder_id) = document.folder_id {
            formulas_stale(ctx, folder_id, Some(document.id))?;
        }
        let table = table_of(&document);
        Ok(changed(ctx, document)?.and(table))
    }
}

/// Changes a live document's title to a non-empty one.
#[derive(FromRequest)]
#[request(RenameDocumentRequest)]
pub struct RenameDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let document = ctx
            .db
            .update_live()
            .set(Document::TITLE, self.title)
            .set_now(Document::MODIFIED)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .returning_one()
            .found()?;
        let table = table_of(&document);
        Ok(changed(ctx, document)?.and(table))
    }
}

/// Soft-deletes a document in a workspace, where Rewind can bring it back.
/// A document in a table keeps its folder and values, so it comes back as
/// the row it was.
#[derive(FromRequest)]
#[request(DeleteDocumentRequest)]
pub struct DeleteDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
}
impl DatabaseCommand for DeleteDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let document = ctx
            .db
            .update()
            .set_true(Document::DELETED)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .returning_one()
            .found()?;
        let table = table_of(&document);
        Ok(changed(ctx, document)?.and(table))
    }
}

/// The journal for `date` in a workspace, if it exists; never creates one.
#[derive(FromRequest)]
#[request(GetJournalRequest)]
pub struct LookupJournalByDateCommand {
    pub workspace_id: i64,
    pub date: String,
}
impl DatabaseCommand for LookupJournalByDateCommand {
    type Response = Option<Document>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live()
            .filter("workspace_id = ? AND document_type = ? AND journal_date = ?")
            .bind(self.workspace_id)
            .bind(DocumentType::Journal)
            .bind(self.date)
            .one()?)
    }
}

/// The journal for `date` in a workspace, created empty if it does not exist
/// yet.
pub struct EnsureJournalCommand {
    pub workspace_id: i64,
    pub date: String,
}
impl DatabaseCommand for EnsureJournalCommand {
    type Response = Document;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        find_or_create_journal(ctx, self.workspace_id, self.date)
    }
}

/// Adds a line to the journal for `date`, creating it if it does not exist yet.
#[derive(FromRequest)]
#[request(AppendToJournalRequest)]
pub struct AppendToJournalCommand {
    pub workspace_id: i64,
    pub date: String,
    pub content: String,
}
impl DatabaseCommand for AppendToJournalCommand {
    type Response = Document;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let journal = find_or_create_journal(ctx, self.workspace_id, self.date)?;
        journal.and_then(|journal| {
            let document = ctx
                .db
                .update()
                .append_line("contents", self.content)
                .set_now(Document::MODIFIED)
                .filter_eq(Document::ID, journal.id)
                .returning_one()
                .found()?;
            saved(ctx, document)
        })
    }
}

/// Replaces the contents of the journal for `date`, creating it if it does
/// not exist yet.
#[derive(FromRequest)]
#[request(EditJournalRequest)]
pub struct EditJournalCommand {
    pub workspace_id: i64,
    pub date: String,
    pub content: String,
}
impl DatabaseCommand for EditJournalCommand {
    type Response = Document;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let journal = find_or_create_journal(ctx, self.workspace_id, self.date)?;
        journal.and_then(|journal| {
            let document = ctx
                .db
                .update()
                .set(Document::CONTENTS, self.content)
                .set_now(Document::MODIFIED)
                .filter_eq(Document::ID, journal.id)
                .returning_one()
                .found()?;
            saved(ctx, document)
        })
    }
}

/// Replaces the contents of a live document in the workspace.
#[derive(FromRequest)]
#[request(EditDocumentRequest)]
pub struct EditDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
    pub contents: String,
}
impl DatabaseCommand for EditDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let document = ctx
            .db
            .update_live()
            .set(Document::CONTENTS, self.contents)
            .set_now(Document::MODIFIED)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .returning_one()
            .found()?;
        saved(ctx, document)
    }
}

/// Appends to the contents of a live document in the workspace.
pub struct AppendToDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
    pub contents: String,
}
impl DatabaseCommand for AppendToDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let document = ctx
            .db
            .update_live()
            .append("contents", self.contents)
            .set_now(Document::MODIFIED)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .returning_one()
            .found()?;
        saved(ctx, document)
    }
}

/// Live documents in a workspace whose title contains `query`.
#[derive(FromRequest)]
#[request(SearchDocumentRequest)]
pub struct SearchDocumentsCommand {
    pub workspace_id: i64,
    pub query: String,
}
impl DatabaseCommand for SearchDocumentsCommand {
    type Response = Vec<Document>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live()
            .filter("workspace_id = ? AND title LIKE ?")
            .bind(self.workspace_id)
            .bind(format!("%{}%", self.query))
            .all()?)
    }
}

/// A live document in a workspace by id.
#[derive(FromRequest)]
#[request(GetDocumentRequest)]
pub struct GetDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
}
impl DatabaseCommand for GetDocumentCommand {
    type Response = Document;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Document> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select_live()
            .filter("workspace_id = ? AND id = ?")
            .bind(self.workspace_id)
            .bind(self.document_id)
            .one()
            .found()
    }
}

#[cfg(test)]
mod tests;
