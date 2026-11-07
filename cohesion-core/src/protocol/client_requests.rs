//! Requests about trusted clients.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::Client;

/// Lists every client that is not deleted, each saying whether it is
/// connected now and whether it is the one asking.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListClientsRequest;

/// A client as the clients list shows it. Presence is said here and only
/// here: a client's actors are reachable while the client is online.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct ListedClient {
    #[serde(flatten)]
    pub client: Client,
    /// Whether the client has a session open with the daemon now.
    pub online: bool,
    /// Whether it is the client that asked for the list.
    pub current: bool,
}

/// Revokes client `id`; its certificate is no longer trusted.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteClientRequest {
    pub id: i64,
}

/// Renames client `id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameClientRequest {
    pub id: i64,
    pub client_name: String,
}
