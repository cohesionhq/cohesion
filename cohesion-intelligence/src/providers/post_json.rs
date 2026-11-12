//! The HTTP exchange every backend makes for a turn.

use std::sync::LazyLock;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::intelligence_error::IntelligenceError;

/// How long a provider may take to accept the connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a whole turn may take, a streamed reply read to its end
/// included. This covers a long, high-effort answer; past it the turn fails
/// rather than hanging.
const TURN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The HTTP client every backend sends its turns with. One for the process,
/// so a new thread or fork reuses the open connection to its provider
/// rather than paying for a new TLS handshake.
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TURN_TIMEOUT)
        .build()
        .expect("the HTTP client builds")
});

/// The HTTP client a backend sends its turns with: a handle on [`CLIENT`].
pub(super) fn client() -> reqwest::Client {
    CLIENT.clone()
}

/// Sends `body` as JSON with `request` (its URL and credentials already
/// set) and parses a successful reply. A failure status becomes its
/// `IntelligenceError`: 401/403 and 429 for every provider, and a full
/// context window when `is_context_limit` recognises the status and body.
pub(super) async fn post_json<R: DeserializeOwned>(
    request: reqwest::RequestBuilder,
    body: &impl Serialize,
    is_context_limit: impl Fn(u16, &str) -> bool,
) -> Result<R, IntelligenceError> {
    let body = post(request, body, is_context_limit).await?;
    serde_json::from_slice(&body).map_err(IntelligenceError::InvalidResponse)
}

/// [`post_json`]'s exchange, answering a successful reply's whole body
/// unparsed, such as a stream of server-sent events read to its end.
pub(super) async fn post(
    request: reqwest::RequestBuilder,
    body: &impl Serialize,
    is_context_limit: impl Fn(u16, &str) -> bool,
) -> Result<Vec<u8>, IntelligenceError> {
    let body = serde_json::to_vec(body).map_err(IntelligenceError::Serialization)?;
    let response = request
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
        .map_err(IntelligenceError::Network)?;
    let status = response.status();
    let body = response.bytes().await.map_err(IntelligenceError::Network)?;

    if !status.is_success() {
        let body = String::from_utf8_lossy(&body);
        return Err(match status.as_u16() {
            401 | 403 => IntelligenceError::InvalidAuthorization,
            429 => IntelligenceError::RateLimited,
            code if is_context_limit(code, &body) => IntelligenceError::ContextLimitExceeded,
            code => IntelligenceError::UnexpectedStatus(code, body.into_owned()),
        });
    }
    Ok(body.to_vec())
}
