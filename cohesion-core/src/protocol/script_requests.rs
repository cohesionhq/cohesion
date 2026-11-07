//! Requests the script engine carries out, and their replies.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// The JSON text of a script's completion value. A newtype so that other
/// string replies can have their own `ReplyData` variant.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, TsWire)]
#[serde(transparent)]
pub struct ScriptResult(pub String);

/// Evaluates a script on the server.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RunScriptRequest {
    pub script: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DocumentationRequest;
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DocumentationResponse {
    pub documentation: String,
}
