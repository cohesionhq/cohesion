use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// A client trusted to talk to cohesiond. Any client may show Cohesion to a
/// person, offer actors, or both.
#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "clients", backup = Included)]
pub struct Client {
    pub id: i64,
    pub deleted: bool,
    /// What identifies the client, and so what `key` is the fingerprint of.
    pub key_kind: ClientKeyKind,
    /// The SHA-256 of what identifies the client; unique among live clients.
    pub key: String,
    /// How the client is listed; it can be renamed.
    pub client_name: String,
    /// When it paired, in Unix seconds; `None` for a client paired before
    /// this was recorded.
    pub created_at: Option<i64>,
    /// When it last connected, in Unix seconds; `None` until it does. A
    /// connected client is not updated while its connection lasts.
    pub last_seen_at: Option<i64>,
    /// The address it last connected from; `None` for a client on the
    /// same computer as Cohesion, or one not yet seen.
    pub last_address: Option<String>,
}

/// How a client proves who it is.
///
/// - `"certificate"`: the certificate it registered with a setup code,
///   presented over mutual TLS.
/// - `"local"`: the local key it presents on the daemon's Unix socket,
///   whose file permissions trust it.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ClientKeyKind {
    Certificate,
    Local,
}
