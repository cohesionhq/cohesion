use std::path::Path;

use cohesion_core::protocol::{
    Call, Command, ListTaskFoldersRequest, ListWorkspacesRequest, Outcome, ReplyData,
};
use cohesion_core::{TaskFolder, Workspace};

use crate::{
    cli_error::CliError,
    client_config::{Configuration, Server},
    client_identity::ClientIdentity,
    client_storage::{ApplicationStorageError, StoragePath},
    connection::Connection,
    local_connection::LocalConnection,
    name_lookup::find,
    remote_connection::RemoteConnection,
};

/// This client's config directory: `config_directory` if given, else the default.
pub fn storage_path(config_directory: Option<&str>) -> Result<StoragePath, CliError> {
    match config_directory {
        None => StoragePath::default(),
        Some(dir) => StoragePath::resolve(dir),
    }
    .map_err(CliError::Storage)
}

/// Everything a command needs: where config lives, the config itself, and a server connection.
pub struct Context {
    storage_path: StoragePath,
    config: Configuration,
    connection: Connection,
    workspace_override: Option<String>,
}

impl Context {
    pub fn load(
        config_directory: Option<&str>,
        workspace_override: Option<String>,
    ) -> Result<Self, CliError> {
        let storage_path = storage_path(config_directory)?;
        let not_configured = |err| match err {
            ApplicationStorageError::NotConfigured => {
                CliError::NotConfigured(storage_path.root().to_path_buf())
            }
            err => CliError::Storage(err),
        };
        let config = Configuration::load(&storage_path).map_err(not_configured)?;
        let connection = match &config.server {
            Server::Local { socket, identity } => {
                Connection::Local(LocalConnection::new(socket.clone(), identity.clone()))
            }
            Server::Remote {
                host,
                port,
                server_fingerprint,
            } => {
                let identity = ClientIdentity::load(&storage_path).map_err(not_configured)?;
                let remote =
                    RemoteConnection::new(host, *port, server_fingerprint.clone(), identity)
                        .map_err(CliError::Setup)?;
                Connection::Remote(remote)
            }
        };
        Ok(Self {
            storage_path,
            config,
            connection,
            workspace_override,
        })
    }

    /// The workspace commands act in: `--workspace` if given, looked up among
    /// `workspaces`, else the one from `switch`.
    pub fn current_workspace(&self, workspaces: &[Workspace]) -> Result<Option<i64>, CliError> {
        match &self.workspace_override {
            Some(query) => find(workspaces, query).map(|w| Some(w.id)),
            None => Ok(self.config.workspace),
        }
    }

    /// The workspace commands act in, failing when none is selected.
    pub async fn workspace(&self) -> Result<i64, CliError> {
        match &self.workspace_override {
            Some(query) => Ok(self.find_workspace(query).await?.id),
            None => self.config.workspace.ok_or(CliError::NoWorkspace),
        }
    }

    /// The workspace whose id or title is `query`.
    pub async fn find_workspace(&self, query: &str) -> Result<Workspace, CliError> {
        let workspaces = self.call(ListWorkspacesRequest).await?;
        find(&workspaces, query).cloned()
    }

    /// The task folder in `workspace_id` whose id or title is `query`.
    pub async fn find_folder(
        &self,
        workspace_id: i64,
        query: &str,
    ) -> Result<TaskFolder, CliError> {
        let folders = self.call(ListTaskFoldersRequest { workspace_id }).await?;
        find(&folders, query).cloned()
    }

    /// The workspace selected with `switch`, ignoring `--workspace`.
    pub fn selected_workspace(&self) -> Option<i64> {
        self.config.workspace
    }

    pub fn select_workspace(&mut self, id: Option<i64>) -> Result<(), CliError> {
        self.config.workspace = id;
        self.config
            .save(&self.storage_path)
            .map_err(CliError::Storage)
    }

    /// Where this client's identity and settings live.
    pub fn config_root(&self) -> &Path {
        self.storage_path.root()
    }

    /// The files in this client's config directory.
    pub fn storage_path(&self) -> &StoragePath {
        &self.storage_path
    }

    /// How this client reaches cohesiond.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Where cohesiond is expected.
    pub fn target(&self) -> String {
        self.connection.target()
    }

    /// Sends `command`, returning the server's reply whether it succeeded or not.
    pub async fn send(&self, command: Command) -> Result<Outcome, CliError> {
        self.connection
            .send(command)
            .await
            .map_err(|err| CliError::Unreachable(self.connection.target(), err))
    }

    /// Sends `request` and returns its typed reply, treating an error reply as a failure.
    pub async fn call<C: Call>(&self, request: C) -> Result<C::Reply, CliError> {
        expect_reply(self.send(request.into()).await?)
    }
}

/// The payload of a successful `outcome`, if it is the kind of reply expected.
pub fn expect_reply<T: TryFrom<ReplyData, Error = ReplyData>>(
    outcome: Outcome,
) -> Result<T, CliError> {
    match outcome {
        Outcome::Ok(data) => T::try_from(data).map_err(|_| CliError::UnexpectedReply),
        Outcome::Error(err) => Err(CliError::Server(err)),
    }
}
