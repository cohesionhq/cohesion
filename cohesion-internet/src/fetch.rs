//! The one HTTP client every read of the web goes through.

use std::io::Read;
use std::sync::LazyLock;
use std::time::Duration;

use cohesion_core::model::ArticleFetchFailure;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;

/// The most of a body read; a longer one is `TooLarge`.
const BODY_LIMIT: u64 = 5 * 1024 * 1024;

/// Follows redirects and gives up after 20 seconds, like `network.fetch`.
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("cohesion")
        .build()
        .expect("the web client builds")
});

/// A body read from the web.
pub(crate) struct Fetched {
    /// The address it was read from, after redirects.
    pub final_url: Url,
    /// Its `Content-Type`'s media type, lowercased, without parameters.
    pub media_type: String,
    /// The body, invalid UTF-8 replaced.
    pub body: String,
}

/// Fetches `url`, failing on an error status, a media type `accepts` turns
/// down, or a body over 5 MiB.
pub(crate) fn fetch(
    url: &Url,
    accepts: impl Fn(&str) -> bool,
) -> Result<Fetched, ArticleFetchFailure> {
    // reqwest's own message only names the URL; the cause is in its sources.
    let unreachable = |error: &dyn std::error::Error| {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        ArticleFetchFailure::Unreachable { message }
    };
    let response = CLIENT
        .get(url.clone())
        .send()
        .map_err(|e| unreachable(&e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(ArticleFetchFailure::Status {
            code: status.as_u16().into(),
        });
    }
    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
        .unwrap_or_default();
    let media_type = media_type(&content_type);
    if !accepts(&media_type) {
        return Err(ArticleFetchFailure::UnsupportedContent { content_type });
    }
    let mut body = Vec::new();
    response
        .take(BODY_LIMIT + 1)
        .read_to_end(&mut body)
        .map_err(|e| unreachable(&e))?;
    if body.len() as u64 > BODY_LIMIT {
        return Err(ArticleFetchFailure::TooLarge);
    }
    Ok(Fetched {
        final_url,
        media_type,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// `Content-Type`'s media type, lowercased, without its parameters.
fn media_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}
