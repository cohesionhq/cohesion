use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Thinking {
    Standard(StandardThinking),
    Redacted(RedactedThinking),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StandardThinking {
    pub thought: String,
    /// Opaque verification token some providers (Anthropic) require when the
    /// thought is sent back.
    pub signature: Option<String>,
    /// Whether the provider returned the thought inside the message content
    /// (Mistral) rather than a separate reasoning field; it goes back the
    /// same way. Defaulted for transcripts saved before it existed.
    #[serde(default)]
    pub in_content: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedactedThinking {
    pub trace: String,
    pub summary: Option<String>,
}
