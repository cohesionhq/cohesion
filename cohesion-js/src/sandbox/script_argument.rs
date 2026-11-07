use rquickjs::{Ctx, IntoJs, Value};

use cohesion_core::model::{self, ActorSession, ActorSessionMove, Feed, Resource, Task};
use cohesion_db::{
    GetAgentMessageCommand, GetArticleCommand, GetDocumentCommand, GetIntelligenceAgentCommand,
    GetResourceCommand, GetTaskCommand, GetTaskFolderIncludingDeletedCommand,
    GetWorkspaceIncludingDeletedCommand, GuardedDatabaseContext,
};

use super::activity::Activity;
use super::agent_message::AgentMessage;
use super::caught_error::internal;
use super::notification::Notification;
use super::revoked_client::RevokedClient;
use crate::entities::actor_session::JsActorSession;
use crate::entities::agent::JsAgent;
use crate::entities::article::JsArticle;
use crate::entities::document::JsDocument;
use crate::entities::feed::JsFeed;
use crate::entities::resource::JsResource;
use crate::entities::task::JsTask;
use crate::entities::task_folder::JsTaskFolder;
use crate::entities::workspace::JsWorkspace;
use crate::script_error::ScriptError;

/// A value handed to a script function: an entity by reference, which the
/// sandbox loads and passes as the matching entity, a deleted entity as it
/// was, which can no longer be loaded, a plain string, or plain data passed
/// as the JS value its JSON parses to.
#[derive(Clone, Debug)]
pub enum ScriptArgument {
    /// Passed as a `Task`.
    Task {
        workspace_id: i64,
        task_id: i64,
    },
    TaskFolder {
        workspace_id: i64,
        folder_id: i64,
    },
    Workspace {
        workspace_id: i64,
    },
    /// Passed as a `Document`.
    Document {
        workspace_id: i64,
        document_id: i64,
    },
    /// Passed as an `Resource`.
    Resource {
        workspace_id: i64,
        resource_id: i64,
    },
    /// Passed as a `Task`.
    DeletedTask {
        task: Task,
    },
    /// Passed as a `Resource`.
    DeletedResource {
        resource: Resource,
    },
    /// Passed as an `Article`.
    Article {
        article_id: i64,
    },
    /// Passed as a `Feed`.
    DeletedFeed {
        feed: Feed,
    },
    /// Passed as an `Agent`.
    Agent {
        agent_id: i64,
    },
    /// Passed as an `AgentMessage`, plain data rather than an entity.
    AgentMessage {
        message_id: i64,
    },
    /// Passed as an `Activity`, plain data as it stood when the event was
    /// raised.
    Activity {
        activity: model::Activity,
    },
    /// Passed as an `ActorSession`, as it stood when the event was raised;
    /// `moved` says why it moved, which an agent following it reads.
    ActorSession {
        actor_session: ActorSession,
        moved: ActorSessionMove,
    },
    /// Passed as a `Notification`, plain data that nothing stores.
    Notification {
        title: String,
        message: String,
    },
    /// Passed as a `RevokedClient`, plain data, since the client can no
    /// longer be looked up.
    RevokedClient {
        client_id: i64,
        name: String,
    },
    /// Passed as a `string`.
    String(String),
    /// Passed as the object, array or primitive the JSON describes.
    Json(serde_json::Value),
}
impl ScriptArgument {
    pub(super) fn inflate<'js>(
        &self,
        ctx: &Ctx<'js>,
        database: &GuardedDatabaseContext,
    ) -> Result<Value<'js>, ScriptError> {
        let entity = match self {
            &Self::Task {
                workspace_id,
                task_id,
            } => {
                let task = database.send_read_blocking(GetTaskCommand {
                    workspace_id,
                    task_id,
                })?;
                JsTask::new(task, database.clone()).into_js(ctx)
            }
            &Self::TaskFolder {
                workspace_id,
                folder_id,
            } => {
                let folder = database.send_read_blocking(GetTaskFolderIncludingDeletedCommand {
                    workspace_id,
                    folder_id,
                })?;
                JsTaskFolder::new(folder, database.clone()).into_js(ctx)
            }
            &Self::Workspace { workspace_id } => {
                let workspace = database
                    .send_read_blocking(GetWorkspaceIncludingDeletedCommand { id: workspace_id })?;
                JsWorkspace::new(workspace, database.clone()).into_js(ctx)
            }
            &Self::Document {
                workspace_id,
                document_id,
            } => {
                let document = database.send_read_blocking(GetDocumentCommand {
                    workspace_id,
                    document_id,
                })?;
                JsDocument::new(document, database.clone()).into_js(ctx)
            }
            &Self::Resource {
                workspace_id,
                resource_id,
            } => {
                let resource = database.send_read_blocking(GetResourceCommand {
                    workspace_id,
                    resource_id,
                })?;
                JsResource::new(resource, database.clone()).into_js(ctx)
            }
            Self::DeletedTask { task } => JsTask::new(task.clone(), database.clone()).into_js(ctx),
            Self::ActorSession { actor_session, .. } => {
                JsActorSession::new(actor_session.clone(), database.clone()).into_js(ctx)
            }
            Self::DeletedResource { resource } => {
                JsResource::new(resource.clone(), database.clone()).into_js(ctx)
            }
            &Self::Article { article_id } => {
                let article = database.send_read_blocking(GetArticleCommand { article_id })?;
                JsArticle::new(article, database.clone()).into_js(ctx)
            }
            Self::DeletedFeed { feed } => JsFeed::new(feed.clone(), database.clone()).into_js(ctx),
            &Self::Agent { agent_id } => {
                let agent =
                    database.send_read_blocking(GetIntelligenceAgentCommand { agent_id })?;
                JsAgent::new(agent, database.clone()).into_js(ctx)
            }
            &Self::AgentMessage { message_id } => {
                let message = database.send_read_blocking(GetAgentMessageCommand { message_id })?;
                let message = serde_json::to_string(&AgentMessage::from(message))
                    .expect("an agent message serializes");
                ctx.json_parse(message)
            }
            Self::Activity { activity } => {
                let activity = serde_json::to_string(&Activity::from(activity.clone()))
                    .expect("an activity serializes");
                ctx.json_parse(activity)
            }
            Self::Notification { title, message } => {
                let notification = serde_json::to_string(&Notification {
                    title: title.clone(),
                    message: message.clone(),
                })
                .expect("a notification serializes");
                ctx.json_parse(notification)
            }
            Self::RevokedClient { client_id, name } => {
                let client = serde_json::to_string(&RevokedClient {
                    id: *client_id,
                    name: name.clone(),
                })
                .expect("a revoked client serializes");
                ctx.json_parse(client)
            }
            Self::String(text) => text.as_str().into_js(ctx),
            Self::Json(value) => {
                ctx.json_parse(serde_json::to_string(value).expect("JSON values serialize"))
            }
        };
        entity.map_err(internal)
    }
}
