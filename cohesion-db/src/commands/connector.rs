use cohesion_core::Named;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    Connector, ConnectorDefinition, Resource, ResourceKind, ResourceKindDescription,
};
use cohesion_core::value::Json;

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase,
};

/// Stores `definition` as its connector's, the first time one with its
/// identifier is served, or in place of the stored one when its version is
/// higher; a lower or equal version leaves the stored one. A kind the new
/// definition leaves out goes, and its resources with it. Returns the
/// connector as it is now stored, having changed what the write did.
pub(crate) fn upsert_connector(
    ctx: &mut CommandContext<'_>,
    definition: ConnectorDefinition,
) -> DatabaseCommandResult<Changed<Connector>> {
    let ConnectorDefinition {
        identifier,
        icon,
        version,
        description,
    } = definition;
    let written = ctx
        .db
        .insert::<Connector>()
        .set(Connector::IDENTIFIER, identifier.as_str())
        .set(Connector::VERSION, version)
        .set(Connector::TITLE, description.title)
        .set(Connector::ICON, icon)
        .set(Connector::UNIT, description.unit)
        .set(Connector::METADATA_KEYS, Json(description.metadata_keys))
        .on_conflict_update(&["identifier"])
        .when_greater(Connector::VERSION)
        .execute()?;
    let connector_id = ctx
        .db
        .select_column::<Connector, i64>("id")
        .filter("identifier = ?")
        .bind(identifier.as_str())
        .one()?
        .ok_or(CommandError::NotFound(Connector::NOUN))?;
    let mut changes = ().invalidating_nothing();
    if written > 0 {
        changes = replace_kinds(ctx, connector_id, description.kinds)?
            .and([Invalidation::ConnectorAccounts]);
    }
    let connector = ctx
        .db
        .select::<Connector>()
        .filter_eq(Connector::ID, connector_id)
        .one()
        .found()?;
    Ok(changes.map(|()| connector))
}

/// Makes connector `connector_id`'s kinds exactly `kinds`, keeping the id
/// of each kind it already had.
fn replace_kinds(
    ctx: &mut CommandContext<'_>,
    connector_id: i64,
    kinds: Vec<ResourceKindDescription>,
) -> DatabaseCommandResult<Changed<()>> {
    let kept: Vec<String> = kinds.iter().map(|k| k.identifier.clone()).collect();
    let dropped = ctx
        .db
        .select_column::<ResourceKind, i64>("id")
        .filter("connector_id = ?")
        .bind(connector_id)
        .filter_not_in_values("identifier", kept.clone());
    let emptied = ctx
        .db
        .select_column::<Resource, i64>("workspace_id")
        .distinct()
        .filter_in("kind_id", dropped)
        .all()?;
    ctx.db
        .delete::<ResourceKind>()
        .filter_eq(ResourceKind::CONNECTOR_ID, connector_id)
        .filter_not_in_values("identifier", kept)
        .execute()?;
    for kind in kinds {
        ctx.db
            .insert::<ResourceKind>()
            .set(ResourceKind::CONNECTOR_ID, connector_id)
            .set(ResourceKind::IDENTIFIER, kind.identifier)
            .set(ResourceKind::TITLE, kind.title)
            .set(ResourceKind::PLURAL, kind.plural)
            .set(ResourceKind::CONTENTS, kind.contents)
            .on_conflict_update(&["connector_id", "identifier"])
            .execute()?;
    }
    Ok(().invalidating(emptied.into_iter().map(Invalidation::Resources)))
}

/// Every connector, by identifier.
pub struct ListConnectorsCommand {}
impl DatabaseCommand for ListConnectorsCommand {
    type Response = Vec<Connector>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select::<Connector>().order("identifier").all()?)
    }
}
