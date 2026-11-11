//! Document folders: a tree of them in each workspace, and filing a document
//! into one. A document in no folder is in the Inbox.

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::DocumentFolderProperty;
use cohesion_core::protocol::{
    CreateDocumentFolderRequest, DeleteDocumentFolderRequest, ListDocumentFoldersRequest,
    MoveDocumentFolderRequest, MoveDocumentRequest, RenameDocumentFolderRequest,
};
use cohesion_core::{Document, DocumentFolder, DocumentType, FromRequest};

use crate::changed::{Changed, Invalidates};
use crate::commands::document::{GetDocumentCommand, changed, row_changed};
use crate::commands::document_table::{TableChange, rows_left};
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// One live document folder by id; a deleted one, or one of another
/// workspace, is not found.
pub struct GetDocumentFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for GetDocumentFolderCommand {
    type Response = DocumentFolder;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<DocumentFolder> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select_live()
            .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(DocumentFolder::ID, self.folder_id)
            .one()
            .found()
    }
}

/// Every live document folder in a workspace, ordered by their sort order;
/// each names its parent, so the list is the tree.
#[derive(FromRequest)]
#[request(ListDocumentFoldersRequest)]
pub struct ListDocumentFoldersCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListDocumentFoldersCommand {
    type Response = Vec<DocumentFolder>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<DocumentFolder>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live()
            .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .order("sort_order, id")
            .all()?)
    }
}

/// The live ordinary documents in one folder of a workspace (not the folders
/// inside it), or in the Inbox when `folder_id` is `None`, most recently
/// modified first.
pub struct ListFiledDocumentsCommand {
    pub workspace_id: i64,
    pub folder_id: Option<i64>,
}
impl DatabaseCommand for ListFiledDocumentsCommand {
    type Response = Vec<Document>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Document>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live::<Document>()
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::DOCUMENT_TYPE, DocumentType::Normal)
            .filter_eq(Document::FOLDER_ID, self.folder_id)
            .order("modified DESC")
            .all()?)
    }
}

/// `root` and every folder inside it, at any depth, from `folders`.
fn subtree(folders: &[DocumentFolder], root: i64) -> Vec<i64> {
    let mut ids = vec![root];
    let mut next = 0;
    while let Some(&id) = ids.get(next) {
        ids.extend(
            folders
                .iter()
                .filter(|folder| folder.parent_id == Some(id))
                .map(|folder| folder.id),
        );
        next += 1;
    }
    ids
}

/// Checks that `parent_id`, when set, is a live folder of the workspace.
fn ensure_parent(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    parent_id: Option<i64>,
) -> DatabaseCommandResult<()> {
    if let Some(folder_id) = parent_id {
        GetDocumentFolderCommand {
            workspace_id,
            folder_id,
        }
        .handle(&mut ctx.as_read())?;
    }
    Ok(())
}

/// Creates a document folder with a non-empty title, at the top level or
/// inside a live folder of the same workspace, last among its siblings.
#[derive(FromRequest)]
#[request(CreateDocumentFolderRequest)]
pub struct CreateDocumentFolderCommand {
    pub workspace_id: i64,
    pub title: String,
    pub parent_id: Option<i64>,
}
impl DatabaseCommand for CreateDocumentFolderCommand {
    type Response = DocumentFolder;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolder>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        ensure_parent(ctx, self.workspace_id, self.parent_id)?;
        let folder = ctx
            .db
            .insert::<DocumentFolder>()
            .set(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .set(DocumentFolder::PARENT_ID, self.parent_id)
            .set(DocumentFolder::TITLE, self.title)
            .set_max_incremented_within(
                DocumentFolder::SORT_ORDER,
                DocumentFolder::WORKSPACE_ID,
                self.workspace_id,
            )
            .returning_one()
            .found()?;
        Ok(folder.invalidating([Invalidation::DocumentFolders(self.workspace_id)]))
    }
}

/// Changes a live document folder's title to a non-empty one.
#[derive(FromRequest)]
#[request(RenameDocumentFolderRequest)]
pub struct RenameDocumentFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameDocumentFolderCommand {
    type Response = DocumentFolder;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolder>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let folder = ctx
            .db
            .update_live()
            .set(DocumentFolder::TITLE, self.title)
            .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(DocumentFolder::ID, self.folder_id)
            .returning_one()
            .found()?;
        Ok(folder.invalidating([Invalidation::DocumentFolders(self.workspace_id)]))
    }
}

/// Moves a live document folder into another live folder of its workspace,
/// or the top level, placing it before its new sibling `before_id`, or last.
/// Its new siblings are renumbered in their order. A folder can't move into
/// itself or a folder inside it.
#[derive(FromRequest)]
#[request(MoveDocumentFolderRequest)]
pub struct MoveDocumentFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub parent_id: Option<i64>,
    pub before_id: Option<i64>,
}
impl DatabaseCommand for MoveDocumentFolderCommand {
    type Response = Vec<DocumentFolder>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<DocumentFolder>>> {
        let list = |ctx: &mut CommandContext<'_>| {
            ListDocumentFoldersCommand {
                workspace_id: self.workspace_id,
            }
            .handle(&mut ctx.as_read())
        };
        GetDocumentFolderCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
        }
        .handle(&mut ctx.as_read())?;
        ensure_parent(ctx, self.workspace_id, self.parent_id)?;
        let folders = list(ctx)?;
        if let Some(parent_id) = self.parent_id
            && subtree(&folders, self.folder_id).contains(&parent_id)
        {
            return Err(ValidationError::new(
                "A folder can’t move into itself or a folder inside it.",
            )
            .into());
        }
        let mut siblings: Vec<i64> = folders
            .iter()
            .filter(|folder| folder.parent_id == self.parent_id && folder.id != self.folder_id)
            .map(|folder| folder.id)
            .collect();
        let position = match self.before_id {
            None => siblings.len(),
            Some(before_id) => {
                siblings
                    .iter()
                    .position(|&id| id == before_id)
                    .ok_or_else(|| {
                        ValidationError::new("The folder to go before isn’t in that folder.")
                    })?
            }
        };
        siblings.insert(position, self.folder_id);
        ctx.db
            .update_live::<DocumentFolder>()
            .set(DocumentFolder::PARENT_ID, self.parent_id)
            .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(DocumentFolder::ID, self.folder_id)
            .returning_one()
            .found()?;
        for (sort_order, folder_id) in siblings.into_iter().enumerate() {
            ctx.db
                .update_live::<DocumentFolder>()
                .set(DocumentFolder::SORT_ORDER, sort_order as i64)
                .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
                .filter_eq(DocumentFolder::ID, folder_id)
                .returning_one()
                .found()?;
        }
        Ok(list(ctx)?.invalidating([Invalidation::DocumentFolders(self.workspace_id)]))
    }
}

/// Soft-deletes a live document folder and every folder inside it, and
/// moves the documents they held, deleted ones too, to the Inbox, so none
/// is lost with its folder. A table among them loses its properties and
/// its documents their values, which belong to it; the documents stay.
#[derive(FromRequest)]
#[request(DeleteDocumentFolderRequest)]
pub struct DeleteDocumentFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for DeleteDocumentFolderCommand {
    type Response = DocumentFolder;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<DocumentFolder>> {
        let mut folder = GetDocumentFolderCommand {
            workspace_id: self.workspace_id,
            folder_id: self.folder_id,
        }
        .handle(&mut ctx.as_read())?;
        let folders = ListDocumentFoldersCommand {
            workspace_id: self.workspace_id,
        }
        .handle(&mut ctx.as_read())?;
        let doomed = subtree(&folders, self.folder_id);
        let tables = rows_left(ctx.db, &doomed, None)?;
        _ = ctx
            .db
            .delete::<DocumentFolderProperty>()
            .filter_in_values("folder_id", doomed.clone())
            .execute()?;
        _ = ctx
            .db
            .update::<Document>()
            .set_null(Document::FOLDER_ID)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_in_values("folder_id", doomed.clone())
            .execute()?;
        _ = ctx
            .db
            .update::<DocumentFolder>()
            .set_true(DocumentFolder::DELETED)
            .filter_eq(DocumentFolder::WORKSPACE_ID, self.workspace_id)
            .filter_in_values("id", doomed)
            .execute()?;
        folder.deleted = true;
        Ok(folder
            .invalidating([
                Invalidation::DocumentFolders(self.workspace_id),
                Invalidation::Documents(self.workspace_id),
            ])
            .and(tables))
    }
}

/// Files a live ordinary document into a live folder of its workspace, or
/// the Inbox when `folder_id` is `None`. A journal is never filed.
///
/// A document's values belong to its folder's table, so one that leaves a
/// folder lets go of them in the same write: moving out of a table, or from
/// one table into another, deletes them; moving into a table gives it no
/// values until its formulas are computed. Moving into the folder it is
/// already in changes nothing.
#[derive(FromRequest)]
#[request(MoveDocumentRequest)]
pub struct MoveDocumentCommand {
    pub workspace_id: i64,
    pub document_id: i64,
    pub folder_id: Option<i64>,
}
impl DatabaseCommand for MoveDocumentCommand {
    type Response = Document;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Document>> {
        let document = GetDocumentCommand {
            workspace_id: self.workspace_id,
            document_id: self.document_id,
        }
        .handle(&mut ctx.as_read())?;
        if matches!(document.document_type, DocumentType::Journal) {
            return Err(ValidationError::new("A journal can’t go in a folder.").into());
        }
        ensure_parent(ctx, self.workspace_id, self.folder_id)?;
        if document.folder_id == self.folder_id {
            return Ok(document.invalidating_nothing());
        }
        let left = match document.folder_id {
            Some(from) => rows_left(ctx.db, &[from], Some(document.id))?,
            None => Vec::new(),
        };
        let document = ctx
            .db
            .update_live()
            .set(Document::FOLDER_ID, self.folder_id)
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .returning_one()
            .found()?;
        let joined = row_changed(ctx, &document, TableChange::RowInputs(document.id))?;
        Ok(changed(ctx, document)?.and(left).and(joined))
    }
}

#[cfg(test)]
mod tests;
