//! Requests about the daemon itself and its clients: who it is, the web UI
//! it was built with and that build's hash, opening pairing (to the clients
//! the security settings let), listing clients with whether each is online
//! and which is asking, and revoking one or every other, which also ends
//! their sessions.

use cohesion_core::log::Module;
use cohesion_core::model::{PairingOpener, SecurityEventKind};
use cohesion_core::protocol::{
    AboutRequest, AboutResponse, BackupDatabaseRequest, BeginPairingRequest, DatabaseBackup,
    DeleteClientRequest, FromClient, GetPairingAccessRequest, ListClientsRequest, ListedClient,
    Local, PairingAccess, PairingCode, RevokeOtherClientsRequest, Route, ServerError, WebBundle,
    WebBundleHash, WebBundleHashRequest, WebBundleRequest,
};
use cohesion_core::{Client, ClientKeyKind};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    DeleteClientCommand, GetSecuritySettingsCommand, ListClientsCommand,
    RecordSecurityEventCommand, RevokeOtherClientsCommand,
};

use crate::client_request::ClientRequest;
use crate::cohesion_context::CohesionContext;
use crate::web_bundle::{web_bundle, web_bundle_hash};

impl Route<Local, AboutRequest> for CohesionContext {
    async fn route(&self, _: AboutRequest) -> Result<AboutResponse, ServerError> {
        Ok(AboutResponse {
            hostname: self.hostname.to_string(),
            version: cohesion_core::VERSION.to_string(),
        })
    }
}

impl Route<Local, BackupDatabaseRequest> for CohesionContext {
    async fn route(&self, _: BackupDatabaseRequest) -> Result<DatabaseBackup, ServerError> {
        self.database.back_up().await.map_err(|error| {
            log_error!(Module::Database, "A backup failed: {error}");
            ServerError::internal()
        })
    }
}

impl Route<Local, WebBundleRequest> for CohesionContext {
    async fn route(&self, _: WebBundleRequest) -> Result<WebBundle, ServerError> {
        web_bundle().cloned().ok_or_else(|| {
            ServerError::bad_request(
                "This cohesiond was built without the web UI; run npm run build in web/ and rebuild it.",
            )
        })
    }
}

impl Route<Local, WebBundleHashRequest> for CohesionContext {
    async fn route(&self, _: WebBundleHashRequest) -> Result<WebBundleHash, ServerError> {
        Ok(WebBundleHash {
            hash: web_bundle_hash(),
        })
    }
}

impl ClientRequest<'_> {
    /// Why the client asking can't open pairing now, if it can't: the
    /// security settings keep it to clients on this computer and it isn't
    /// one. The one place the rule is read, for opening pairing and for
    /// saying whether it could.
    async fn pairing_refusal(&self) -> Result<Option<&'static str>, ServerError> {
        let settings = self
            .context
            .database
            .send(GetSecuritySettingsCommand {})
            .await?;
        let refused = settings.pairing_opened_by == PairingOpener::LocalClients
            && self.client.key_kind != ClientKeyKind::Local;
        Ok(refused.then_some(
            "Clients can only be added from a client on the computer Cohesion runs on. Change this under Add clients from in Settings › Security.",
        ))
    }
}

/// Opens pairing, recorded, unless the security settings refuse the client
/// asking, as [`pairing_refusal`](ClientRequest::pairing_refusal) says.
impl Route<FromClient, BeginPairingRequest> for ClientRequest<'_> {
    async fn route(&self, _: BeginPairingRequest) -> Result<PairingCode, ServerError> {
        if let Some(refusal) = self.pairing_refusal().await? {
            return Err(ServerError::bad_request(refusal));
        }
        let context = self.context;
        context
            .database
            .send(RecordSecurityEventCommand {
                kind: SecurityEventKind::PairingOpened,
                client: Some(self.client.clone()),
                address: self.client.last_address.clone(),
            })
            .await?;
        let key = context.secrets.begin_pairing();
        log_info!(Module::Server, "Pairing is open.");
        Ok(PairingCode {
            code: context.server.setup_code(key).encode(),
        })
    }
}

/// Whether the client asking may open pairing now, so a client can say so
/// before it tries.
impl Route<FromClient, GetPairingAccessRequest> for ClientRequest<'_> {
    async fn route(&self, _: GetPairingAccessRequest) -> Result<PairingAccess, ServerError> {
        Ok(PairingAccess {
            refusal: self.pairing_refusal().await?.map(str::to_string),
        })
    }
}

impl Route<Local, DeleteClientRequest> for CohesionContext {
    async fn route(&self, request: DeleteClientRequest) -> Result<Client, ServerError> {
        let client = self
            .database
            .send(DeleteClientCommand { id: request.id })
            .await?;
        self.presence.revoke(client.id);
        Ok(client)
    }
}

/// Revokes every client but the one asking, and ends their sessions.
impl Route<FromClient, RevokeOtherClientsRequest> for ClientRequest<'_> {
    async fn route(&self, _: RevokeOtherClientsRequest) -> Result<Vec<Client>, ServerError> {
        let revoked = self
            .context
            .database
            .send(RevokeOtherClientsCommand {
                keep: self.client.id,
            })
            .await?;
        for client in &revoked {
            self.context.presence.revoke(client.id);
        }
        Ok(revoked)
    }
}

/// The clients, each online while it has a session open, and the one
/// asking marked.
impl Route<FromClient, ListClientsRequest> for ClientRequest<'_> {
    async fn route(&self, _: ListClientsRequest) -> Result<Vec<ListedClient>, ServerError> {
        let context = self.context;
        let clients = context.database.send(ListClientsCommand {}).await?;
        Ok(clients
            .into_iter()
            .map(|client| ListedClient {
                online: context.presence.online(client.id),
                current: client.id == self.client.id,
                client,
            })
            .collect())
    }
}
