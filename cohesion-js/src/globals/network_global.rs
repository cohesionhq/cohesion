use std::collections::HashMap;
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
    pub method: NetworkMethod,
    pub headers: Option<HashMap<String, String>>,
    /// Sent as JSON. Leave it out when sending `formBody`.
    pub json_body: Option<serde_json::Value>,
    /// Sent as a URL-encoded form, as OAuth token endpoints take it. Leave
    /// it out when sending `jsonBody`.
    pub form_body: Option<HashMap<String, String>>,
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
    /// Sends one HTTP request and returns its response. Throws only when no
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
        let method = match options.method {
            NetworkMethod::Get => Method::GET,
            NetworkMethod::Post => Method::POST,
            NetworkMethod::Put => Method::PUT,
            NetworkMethod::Patch => Method::PATCH,
            NetworkMethod::Delete => Method::DELETE,
        };

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
        request = match (options.json_body, options.form_body) {
            (Some(_), Some(_)) => {
                return Err(Exception::throw_type(
                    &ctx,
                    "Send either `jsonBody` or `formBody`, not both.",
                ));
            }
            (Some(json_body), None) => request.json(&json_body),
            (None, Some(form_body)) => request.form(&form_body),
            (None, None) => request,
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
                client: Client::builder()
                    .timeout(FETCH_TIMEOUT)
                    .user_agent("cohesion")
                    .build()
                    .unwrap(),
            },
        )?;
        Ok(())
    }
}
