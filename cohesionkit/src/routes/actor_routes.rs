//! Requests that reach an actor through its client: starting a session,
//! saying something in one, stopping or resolving one, and switching its
//! mode; and how much of its limits each actor has used, as its client
//! last said.

use cohesion_core::model::{ActorSession, ActorSessionMessage};
use cohesion_core::protocol::{
    ListActorUsageRequest, ListedActorUsage, Local, ResolveActorSessionRequest, Route,
    SendActorSessionMessageRequest, ServerError, SetActorSessionModeRequest,
    StartActorSessionRequest, StopActorSessionRequest,
};
use cohesion_db::ListActorsCommand;

use crate::cohesion_context::CohesionContext;

impl Route<Local, StartActorSessionRequest> for CohesionContext {
    async fn route(&self, request: StartActorSessionRequest) -> Result<ActorSession, ServerError> {
        self.actor_sessions.start(request).await
    }
}

impl Route<Local, SendActorSessionMessageRequest> for CohesionContext {
    async fn route(
        &self,
        request: SendActorSessionMessageRequest,
    ) -> Result<ActorSessionMessage, ServerError> {
        let SendActorSessionMessageRequest {
            session_id,
            content,
            attachments,
        } = request;
        self.actor_sessions
            .send(session_id, content, attachments.unwrap_or_default())
            .await
    }
}

impl Route<Local, StopActorSessionRequest> for CohesionContext {
    async fn route(&self, request: StopActorSessionRequest) -> Result<ActorSession, ServerError> {
        self.actor_sessions.stop(request.session_id).await
    }
}

impl Route<Local, ResolveActorSessionRequest> for CohesionContext {
    async fn route(
        &self,
        request: ResolveActorSessionRequest,
    ) -> Result<ActorSession, ServerError> {
        self.actor_sessions.resolve(request.session_id).await
    }
}

impl Route<Local, SetActorSessionModeRequest> for CohesionContext {
    async fn route(
        &self,
        request: SetActorSessionModeRequest,
    ) -> Result<ActorSession, ServerError> {
        self.actor_sessions
            .set_mode(request.session_id, request.mode)
            .await
    }
}

/// The usage the clients served now hold, one entry a pool, naming its
/// actors by Cohesion's ids rather than their clients' identifiers; the
/// pools in the order of their least actor id.
impl Route<Local, ListActorUsageRequest> for CohesionContext {
    async fn route(&self, _: ListActorUsageRequest) -> Result<Vec<ListedActorUsage>, ServerError> {
        let pools = self.capability_clients.usage();
        if pools.is_empty() {
            return Ok(Vec::new());
        }
        let actors = self.database.send(ListActorsCommand).await?;
        let mut listed: Vec<ListedActorUsage> = pools
            .into_iter()
            .filter_map(|pool| {
                let mut actor_ids: Vec<i64> = actors
                    .iter()
                    .filter(|actor| {
                        pool.actors.iter().any(|(client_id, identifier)| {
                            *client_id == actor.client_id && *identifier == actor.identifier
                        })
                    })
                    .map(|actor| actor.id)
                    .collect();
                actor_ids.sort_unstable();
                (!actor_ids.is_empty()).then_some(ListedActorUsage {
                    actor_ids,
                    usage: pool.usage,
                })
            })
            .collect();
        listed.sort_by_key(|pool| pool.actor_ids[0]);
        Ok(listed)
    }
}
