use cohesion_core::protocol::ClientCapabilities;

use super::actor::sync_actors;
use crate::changed::Changed;
use crate::{CommandContext, DatabaseCommand, DatabaseCommandResult, Write};

/// Makes client `client_id`'s actors what `offers` lists, as
/// [`sync_actors`] says.
pub struct SyncCapabilitiesCommand {
    pub client_id: i64,
    pub offers: ClientCapabilities,
}
impl DatabaseCommand for SyncCapabilitiesCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let ClientCapabilities { actors } = self.offers;
        sync_actors(ctx, self.client_id, actors)
    }
}
