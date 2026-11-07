//! `Notification`, the plain object `onNotification` receives.

use cohesion_core::ts_documentation::TsDocumentation;
use serde::Serialize;

/// A notification, serialized to the object a script receives.
#[derive(Serialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
}

impl TsDocumentation for Notification {
    const TS: &'static str = r#"/**
 * A notification a script or an agent sent the user, as `onNotification`
 * receives it: plain data, kept nowhere but this call.
 */
interface Notification {
  /** A few words saying what it is about. */
  title: string;
  /** What the user should know. */
  message: string;
}"#;
    const TS_BRIEF: &'static str = Self::TS;
}
