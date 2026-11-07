//! `RevokedClient`, the plain object `onClientRevoke` receives.

use cohesion_core::ts_documentation::TsDocumentation;
use serde::Serialize;

/// A revoked client, serialized to the object a script receives.
#[derive(Serialize)]
pub struct RevokedClient {
    pub id: i64,
    pub name: String,
}

impl TsDocumentation for RevokedClient {
    const TS: &'static str = r#"/**
 * A client that was just revoked, as `onClientRevoke` receives it: plain
 * data, since the client can no longer be looked up.
 */
interface RevokedClient {
  /** The client's id, the one a service request from it names as `clientId`. */
  id: number;
  /** The client's name. */
  name: string;
}"#;
    const TS_BRIEF: &'static str = Self::TS;
}
