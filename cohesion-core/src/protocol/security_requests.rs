//! Requests about the daemon's door: the security settings, the security
//! activity, and revoking every other client at once.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{PairingOpener, SecurityEvent};

/// Answers the security settings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetSecuritySettingsRequest;

/// Replaces the security settings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateSecuritySettingsRequest {
    pub pairing_opened_by: PairingOpener,
    pub alert_new_client: bool,
    /// Never when `None`; at least one day.
    pub revoke_inactive_after_days: Option<i64>,
}

/// One page of the security activity, newest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListSecurityEventsRequest {
    #[serde(default)]
    pub offset: u64,
    pub limit: Option<u64>,
}

/// A page of the security activity.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct SecurityEventPage {
    /// How many events there are in all, on any page.
    pub total: i64,
    pub events: Vec<SecurityEvent>,
}

/// Revokes every client but the one sending this.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RevokeOtherClientsRequest;
