//! `Tag` in scripts, and the lookups `Task` and `Workspace` share.

use cohesion_core::model::Tag;
use cohesion_db::{
    CreateTagCommand, DeleteTagCommand, GuardedDatabaseContext, ListTagsCommand, RenameTagCommand,
};
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use crate::traced_command;

/// A name a workspace puts on its tasks, to group them across folders.
///
/// Get them from `Workspace.tags()`, `Workspace.createTag()` or `Task.tags()`.
///
/// Each workspace has its own tags; names are unique within it, ignoring case.
/// `Task.tag()` creates a tag the first time it uses its name.
#[js_entity(Tag)]
impl JsTag {
    /// Changes the name, everywhere the tag is used.
    ///
    /// Saved immediately, and this object's `name` updates to match. An empty
    /// name, or one another tag in the workspace has (ignoring case), throws. Not available in read-only contexts.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameTagCommand {
                workspace_id: self.inner.workspace_id,
                tag_id: self.inner.id,
                name,
            },
        )?;
        Ok(())
    }

    /// Deletes the tag, taking it off everything it was on.
    ///
    /// Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteTagCommand {
                workspace_id: self.inner.workspace_id,
                tag_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

/// The workspace's tag named `name` (ignoring case, as the schema does), if
/// it has one.
pub(crate) fn find_tag<'js>(
    ctx: &Ctx<'js>,
    db: &GuardedDatabaseContext,
    workspace_id: i64,
    name: &str,
) -> rquickjs::Result<Option<Tag>> {
    let tags = traced_command::read(ctx, db, ListTagsCommand { workspace_id })?;
    Ok(tags
        .into_iter()
        .find(|tag| tag.name.eq_ignore_ascii_case(name)))
}

/// Like [`find_tag`], creating the tag when the workspace has none by that
/// name.
pub(crate) fn find_or_create_tag<'js>(
    ctx: &Ctx<'js>,
    db: &GuardedDatabaseContext,
    workspace_id: i64,
    name: String,
) -> rquickjs::Result<Tag> {
    match find_tag(ctx, db, workspace_id, &name)? {
        Some(tag) => Ok(tag),
        None => traced_command::write(ctx, db, CreateTagCommand { workspace_id, name }),
    }
}
