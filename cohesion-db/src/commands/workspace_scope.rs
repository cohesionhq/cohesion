//! The check that scopes a command to one live workspace.

use cohesion_core::Workspace;

use crate::{DatabaseCommandResult, Filtered, Found, ReadableDatabase};

/// Fails with `CommandError::NotFound` unless a live workspace has this id.
pub(crate) fn ensure_workspace<'c>(
    db: impl ReadableDatabase<'c>,
    id: i64,
) -> DatabaseCommandResult<()> {
    db.select_live::<Workspace>()
        .filter_eq(Workspace::ID, id)
        .one()
        .found()
        .map(|_| ())
}

#[cfg(test)]
mod tests;
