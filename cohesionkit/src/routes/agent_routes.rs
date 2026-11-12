//! Agent requests that go through the agent coordinator: changing or
//! deleting an agent, removing one of its cues, chatting with it, stopping
//! its turn and answering its proposals and choices.

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::IntelligenceAgent;
use cohesion_core::protocol::{
    AcceptedResponse, DeleteIntelligenceAgentRequest, Intelligence, RemoveAgentCueRequest,
    ResetAgentAgendaRequest, RespondToAgentChoiceRequest, Route, SendAgentMessageRequest,
    ServerError, SetAgentMessageReactionRequest, StopAgentTurnRequest,
    UpdateIntelligenceAgentRequest,
};
use cohesion_db::{DeleteIntelligenceAgentCommand, UpdateIntelligenceAgentCommand};
use cohesion_intelligence::{AgentError, SessionConstructionError};

use crate::CohesionContext;

impl Route<Intelligence, UpdateIntelligenceAgentRequest> for CohesionContext {
    async fn route(
        &self,
        request: UpdateIntelligenceAgentRequest,
    ) -> Result<IntelligenceAgent, ServerError> {
        self.agents.evict(request.agent_id).await;
        self.database
            .send(UpdateIntelligenceAgentCommand::from(request))
            .await
            .map_err(ServerError::from)
    }
}

impl Route<Intelligence, DeleteIntelligenceAgentRequest> for CohesionContext {
    async fn route(
        &self,
        request: DeleteIntelligenceAgentRequest,
    ) -> Result<IntelligenceAgent, ServerError> {
        self.agents.evict(request.agent_id).await;
        self.database
            .send(DeleteIntelligenceAgentCommand::from(request))
            .await
            .map_err(ServerError::from)
    }
}

impl Route<Intelligence, RemoveAgentCueRequest> for CohesionContext {
    async fn route(&self, request: RemoveAgentCueRequest) -> Result<AcceptedResponse, ServerError> {
        self.agents
            .remove_cue(request.cue_id)
            .await
            .map_err(agent_error)?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Intelligence, ResetAgentAgendaRequest> for CohesionContext {
    async fn route(
        &self,
        request: ResetAgentAgendaRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.agents
            .reset_agenda(request.agent_id)
            .await
            .map_err(agent_error)?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Intelligence, SendAgentMessageRequest> for CohesionContext {
    async fn route(
        &self,
        request: SendAgentMessageRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.agents
            .send_message(
                request.agent_id,
                request.message,
                request.attachments.unwrap_or_default(),
                request.forward,
                request.skill,
                request.workspace_id,
            )
            .await
            .map_err(agent_error)?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Intelligence, StopAgentTurnRequest> for CohesionContext {
    async fn route(&self, request: StopAgentTurnRequest) -> Result<AcceptedResponse, ServerError> {
        self.agents.stop_turn(request.agent_id);
        Ok(AcceptedResponse {})
    }
}

impl Route<Intelligence, SetAgentMessageReactionRequest> for CohesionContext {
    async fn route(
        &self,
        request: SetAgentMessageReactionRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.agents
            .react(request.message_id, request.reaction)
            .await
            .map_err(agent_error)?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Intelligence, RespondToAgentChoiceRequest> for CohesionContext {
    async fn route(
        &self,
        request: RespondToAgentChoiceRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.agents
            .respond_to_choice(request.message_id, request.option)
            .await
            .map_err(agent_error)?;
        Ok(AcceptedResponse {})
    }
}

fn agent_error(error: AgentError) -> ServerError {
    match error {
        AgentError::Command(error) => error.into(),
        AgentError::SessionConstruction(SessionConstructionError::NoDefaultModel) => {
            ServerError::bad_request("No model is set for agents; choose one in Settings.")
        }
        AgentError::SessionConstruction(error) => {
            log_error!(
                Module::Intelligence,
                "Cannot create an agent session: {error:?}"
            );
            ServerError::internal()
        }
        AgentError::Intelligence(error) => {
            log_error!(
                Module::Intelligence,
                "An error occurred during an agent's turn: {error:?}"
            );
            ServerError::upstream(error.to_string())
        }
        AgentError::NotAChoice => ServerError::bad_request(error.to_string()),
        AgentError::Panicked => ServerError::upstream(error.to_string()),
    }
}
