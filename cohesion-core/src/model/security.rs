//! Who may add clients, and the record of what happened at the daemon's
//! door: the security settings and the security activity.

use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// The daemon's security settings; there is always exactly one row.
#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "security_settings", backup = Included)]
pub struct SecuritySettings {
    pub id: i64,
    /// Which clients may open pairing.
    pub pairing_opened_by: PairingOpener,
    /// Whether a new client pairing sends a notification.
    pub alert_new_client: bool,
    /// Revokes a client once it hasn't connected for this many days; never
    /// when `None`. A connected client is never revoked.
    pub revoke_inactive_after_days: Option<i64>,
}

/// Which clients may open pairing.
///
/// - `"anyClient"`: any paired client.
/// - `"localClients"`: only clients on the same computer as Cohesion.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum PairingOpener {
    AnyClient,
    LocalClients,
}

/// Something that happened at the daemon's door.
#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "security_events", backup = Included)]
pub struct SecurityEvent {
    pub id: i64,
    /// When it happened, in Unix seconds.
    pub created_at: i64,
    pub kind: SecurityEventKind,
    /// The client it was about, if any.
    pub client_id: Option<i64>,
    /// That client's name at the time.
    pub client_name: Option<String>,
    /// The address it came from, as the daemon saw it, if it came over the
    /// network.
    pub address: Option<String>,
}

/// What a security event records.
///
/// - `"pairingOpened"`: a client opened pairing.
/// - `"clientPaired"`: a new client paired.
/// - `"clientRenamed"`: a client was renamed.
/// - `"clientRevoked"`: a client was revoked.
/// - `"inactiveClientRevoked"`: a client was revoked for not connecting.
/// - `"settingsChanged"`: the security settings changed.
/// - `"pairingRefused"`: a pairing attempt was refused; recorded once an
///   hour per address.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum SecurityEventKind {
    PairingOpened,
    ClientPaired,
    ClientRenamed,
    ClientRevoked,
    InactiveClientRevoked,
    SettingsChanged,
    PairingRefused,
}
