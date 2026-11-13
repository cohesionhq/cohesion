use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use cohesion_core::TextEnum;
use cohesion_macros::{JsOptions, js_global};
use reqwest::{Method, blocking::Client};
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};
use serde::{Deserialize, Serialize};

use super::global::Global;
use super::network_response::NetworkResponse;
use crate::sandbox::ScriptDeadline;

/// How long one `network.fetch` may take, if the script has the time.
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// The client every sandbox's `network.fetch` goes through. One for the
/// process, so scripts reuse open connections and no sandbox pays for
/// building a client (and its runtime thread) it may never use.
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent("cohesion")
        .build()
        .expect("the script HTTP client builds")
});

#[derive(Serialize, Deserialize, Clone, Debug, TextEnum)]
#[serde(rename_all = "UPPERCASE")]
#[text_enum(rename_all = "UPPERCASE")]
pub enum NetworkMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// How `network.fetch` returns a response's body.
///
/// - `"text"`: as text, the default.
/// - `"base64"`: its bytes base64-encoded, for an image or other binary
///   body; an image so fetched can go to `images.attach()`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum NetworkBodyEncoding {
    Text,
    Base64,
}

#[derive(JsOptions)]
pub struct NetworkFetchOptions {
    pub url: String,
    /// The HTTP method: a `NetworkMethod` value, or any other method token such
    /// as `"PROPFIND"`, `"REPORT"` or `"MKCALENDAR"`. Case matters, and a
    /// method is conventionally upper case.
    pub method: String,
    pub headers: Option<HashMap<String, String>>,
    /// Sent as JSON. Leave it out when sending `formBody` or `textBody`.
    pub json_body: Option<serde_json::Value>,
    /// Sent as a URL-encoded form, as OAuth token endpoints take it. Leave
    /// it out when sending `jsonBody` or `textBody`.
    pub form_body: Option<HashMap<String, String>>,
    /// Sent as is, as UTF-8 text: XML for WebDAV, iCalendar, plain text.
    /// Set the `Content-Type` in `headers`; none is added. Leave it out when
    /// sending `jsonBody` or `formBody`.
    pub text_body: Option<String>,
    /// How to return the response's body; text when left out.
    pub encoding: Option<NetworkBodyEncoding>,
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct NetworkGlobal {
    #[qjs(skip_trace)]
    client: Client,
}
#[js_global("network")]
#[rquickjs::methods]
impl NetworkGlobal {
    /// Sends one HTTP request, with any method, and returns its response. Throws only when no
    /// response arrives (DNS, connection, a 20-second timeout, or the
    /// script's own time running out) or the body is over 32 MiB; an error
    /// status such as 404 is returned like any other.
    pub fn fetch<'js>(
        &self,
        ctx: Ctx<'js>,
        options: NetworkFetchOptions,
    ) -> rquickjs::Result<NetworkResponse> {
        let deadline = ScriptDeadline::remaining_until(&ctx)?;
        let encoding = options.encoding.unwrap_or(NetworkBodyEncoding::Text);
        let method = Method::from_bytes(options.method.as_bytes()).map_err(|_| {
            Exception::throw_type(
                &ctx,
                &format!(
                    "`{}` is not a valid HTTP method; use a token such as \"GET\" or \"PROPFIND\".",
                    options.method
                ),
            )
        })?;

        // The client's own timeout is the ceiling; the script's remaining
        // time is the tighter bound when it is shorter.
        let mut request = self.client.request(method, options.url).timeout(
            deadline
                .saturating_duration_since(Instant::now())
                .min(FETCH_TIMEOUT),
        );
        if let Some(headers) = options.headers {
            for (header, value) in headers {
                request = request.header(header, value);
            }
        }
        request = match (options.json_body, options.form_body, options.text_body) {
            (None, None, None) => request,
            (Some(json_body), None, None) => request.json(&json_body),
            (None, Some(form_body), None) => request.form(&form_body),
            (None, None, Some(text_body)) => request.body(text_body),
            _ => {
                return Err(Exception::throw_type(
                    &ctx,
                    "Send only one of `jsonBody`, `formBody` and `textBody`.",
                ));
            }
        };

        match request
            .send()
            .map_err(|e| e.to_string())
            .and_then(|response| {
                NetworkResponse::read(response, encoding).map_err(|e| e.to_string())
            }) {
            Ok(response) => Ok(response),
            Err(e) => Err(Exception::throw_message(
                &ctx,
                &format!("An error occured during network: {}", e),
            )),
        }
    }
}

impl Global for NetworkGlobal {
    fn install<'a>(
        context: &rquickjs::prelude::Ctx<'a>,
        _database: cohesion_db::GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set(
            "network",
            NetworkGlobal {
                client: CLIENT.clone(),
            },
        )?;
        Ok(())
    }
}
