//! A scratch daemon: the `CohesionContext` cohesiond serves, built on a
//! database of the bench's own, reached through the same routes and
//! commands a client's requests reach. Nothing periodic is started, and
//! nothing outside the database file is touched.

use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::Duration;

use cohesionkit::CohesionContext;
use cohesionkit::ServerAddress;
use cohesionkit::db::{
    AddEndpointModelCommand, Container, CreateIntelligenceAgentCommand,
    CreateIntelligenceEndpointCommand, CreateWorkspaceCommand, DatabaseCommand, DatabaseContext,
    ListAgentMessagesCommand, Secrets, SetIntelligenceDefaultCommand,
};
use cohesionkit::model::{
    AgentMessageContent, IntelligenceAgentColor, IntelligenceAgentMessageSender, IntelligenceTier,
};
use cohesionkit::protocol::{
    Intelligence, ModelPrices, RespondToAgentChoiceRequest, Route, RunScriptRequest, Script,
    SendAgentMessageRequest, ServerError, StopAgentTurnRequest, UpdateModelCatalogRequest,
};

use crate::model_spec::ModelUnderTest;

/// What a stopped turn gets to wind down before the case gives up on it.
const STOP_GRACE: Duration = Duration::from_secs(30);

pub struct Scratch {
    context: CohesionContext,
}

/// A scratch daemon's agent, on the model under test.
pub struct BenchAgent {
    pub id: i64,
}

impl Scratch {
    /// A daemon on a new database at `path`, or in memory when `None`.
    pub async fn open(path: Option<&Path>) -> Result<Self, String> {
        let mut container = match path {
            Some(path) => Container::open(path),
            None => Container::in_memory(),
        }
        .map_err(|e| format!("opening the database: {e:?}"))?;
        container
            .migrate()
            .map_err(|e| format!("migrating the database: {e:?}"))?;
        let address = ServerAddress {
            host: IpAddr::from(Ipv4Addr::LOCALHOST).into(),
            port: 0,
            fingerprint: String::new(),
        };
        let scratch = Self {
            context: CohesionContext::new(container, Secrets::new(), address),
        };
        Ok(scratch)
    }

    /// Makes a workspace for each of `titles`, as a client would.
    pub async fn create_workspaces(&self, titles: &[String]) -> Result<(), String> {
        for title in titles {
            self.send(CreateWorkspaceCommand {
                title: title.clone(),
                emoji: None,
            })
            .await?;
        }
        Ok(())
    }

    pub fn database(&self) -> &DatabaseContext {
        &self.context.database
    }

    /// Runs a database command, its error as text.
    pub async fn send<C: DatabaseCommand>(&self, command: C) -> Result<C::Response, String> {
        self.context
            .database
            .send(command)
            .await
            .map_err(|e| format!("{e:?}"))
    }

    /// Makes `model` the model of every tier, as a user would set it in
    /// settings, and creates the agent that answers the case.
    pub async fn agent_on(&self, model: &ModelUnderTest) -> Result<BenchAgent, String> {
        let endpoint = self
            .send(CreateIntelligenceEndpointCommand {
                name: Some(model.endpoint.name.clone()),
                api_key: model.endpoint.api_key.clone(),
                usage_url: None,
                cache_minutes: None,
                long_cache: None,
                preset: model.endpoint.preset.clone(),
            })
            .await?;
        let endpoint_model = self
            .send(AddEndpointModelCommand {
                endpoint_id: endpoint.id,
                model_identifier: model.model.clone(),
                model_name: None,
                context_size: None,
                // Cohesion records no cost event for a call it can't price,
                // so an unpriced model is priced at nothing, which keeps its
                // calls and tokens on record.
                prices: Some(model.prices.clone().unwrap_or(ModelPrices {
                    input_token_cost: 0.0,
                    output_token_cost: 0.0,
                    cache_read_token_cost: None,
                    cache_write_token_cost: None,
                })),
            })
            .await?;
        for tier in [
            IntelligenceTier::Agent,
            IntelligenceTier::Low,
            IntelligenceTier::Medium,
            IntelligenceTier::High,
        ] {
            self.send(SetIntelligenceDefaultCommand {
                tier,
                endpoint_model_id: endpoint_model.id,
            })
            .await?;
        }
        let agent = self
            .send(CreateIntelligenceAgentCommand {
                name: Some("Bench".to_string()),
                color: IntelligenceAgentColor::Blue,
                emoji: None,
                personality: None,
                verbosity: None,
                role: None,
                directive: None,
                team_id: None,
            })
            .await?;
        Ok(BenchAgent { id: agent.id })
    }

    /// Refreshes the model catalog from models.dev, as the daemon does at
    /// start.
    pub async fn refresh_catalog(&self) -> Result<(), String> {
        Route::<Intelligence, UpdateModelCatalogRequest>::route(
            &self.context,
            UpdateModelCatalogRequest,
        )
        .await
        .map(|_| ())
        .map_err(server_error)
    }

    /// Runs `script` as `cohesion run` would, then waits for the hooks its
    /// writes queued; its last expression's value as JSON.
    pub async fn run_script(&self, script: String) -> Result<serde_json::Value, String> {
        let result =
            Route::<Script, RunScriptRequest>::route(&self.context, RunScriptRequest { script })
                .await
                .map_err(server_error)?;
        self.context.work.idle().await;
        // The engine reports a completion value of `undefined` as such.
        if result.0 == "undefined" {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&result.0).map_err(|e| format!("reading the script's result: {e}"))
    }

    /// Sends the user's message to `agent` and waits until the turn, and
    /// whatever it set going, has ended, or `timeout` has passed, when the
    /// turn is stopped as a user would stop it; whether it ended in time.
    pub async fn converse(
        &self,
        agent: &BenchAgent,
        message: String,
        timeout: Duration,
    ) -> Result<bool, String> {
        Route::<Intelligence, SendAgentMessageRequest>::route(
            &self.context,
            SendAgentMessageRequest {
                agent_id: agent.id,
                message,
                attachments: None,
                forward: None,
                skill: None,
            },
        )
        .await
        .map_err(server_error)?;
        self.wait(agent, timeout).await
    }

    /// Answers the turn that just ended as the user would: picks the
    /// option of the agent's last message, when that is a choice, that
    /// contains `answer` (ignoring case), and otherwise sends `answer` as
    /// a message; then waits as `converse` does.
    pub async fn answer(
        &self,
        agent: &BenchAgent,
        answer: String,
        timeout: Duration,
    ) -> Result<bool, String> {
        let messages = self
            .send(ListAgentMessagesCommand {
                agent_id: agent.id,
                before: None,
                limit: i64::from(u32::MAX),
            })
            .await?;
        let last = messages
            .into_iter()
            .filter(|m| m.sender != IntelligenceAgentMessageSender::User)
            .max_by_key(|m| m.id);
        let wanted = answer.to_lowercase();
        let pick = last.and_then(|message| match message.message_content {
            AgentMessageContent::Choice(choice) => choice
                .options
                .iter()
                .position(|option| option.to_lowercase().contains(&wanted))
                .map(|option| (message.id, option)),
            _ => None,
        });
        match pick {
            Some((message_id, option)) => {
                Route::<Intelligence, RespondToAgentChoiceRequest>::route(
                    &self.context,
                    RespondToAgentChoiceRequest { message_id, option },
                )
                .await
                .map(|_| ())
                .map_err(server_error)?
            }
            None => Route::<Intelligence, SendAgentMessageRequest>::route(
                &self.context,
                SendAgentMessageRequest {
                    agent_id: agent.id,
                    message: answer,
                    attachments: None,
                    forward: None,
                    skill: None,
                },
            )
            .await
            .map(|_| ())
            .map_err(server_error)?,
        }
        self.wait(agent, timeout).await
    }

    /// Waits until the agent's turn, and whatever it set going, has ended,
    /// or `timeout` has passed, when the turn is stopped as a user would
    /// stop it; whether it ended in time.
    async fn wait(&self, agent: &BenchAgent, timeout: Duration) -> Result<bool, String> {
        if tokio::time::timeout(timeout, self.context.work.idle())
            .await
            .is_ok()
        {
            return Ok(true);
        }
        Route::<Intelligence, StopAgentTurnRequest>::route(
            &self.context,
            StopAgentTurnRequest { agent_id: agent.id },
        )
        .await
        .map_err(server_error)?;
        _ = tokio::time::timeout(STOP_GRACE, self.context.work.idle()).await;
        Ok(false)
    }

    /// Shuts the daemon down as cohesiond does, closing the database.
    pub async fn close(self) {
        self.context.shutdown().await;
    }
}

fn server_error(error: ServerError) -> String {
    error.message
}
