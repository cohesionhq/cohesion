//! What `network.fetch` hands back: the status, headers and body of one
//! HTTP response.

use std::collections::HashMap;
use std::io::{self, Read};

use base64::{Engine, engine::general_purpose::STANDARD};
use cohesion_macros::TsInterface;
use reqwest::blocking::Response;
use rquickjs::{Ctx, IntoJs, Object, Value};

use super::network_global::NetworkBodyEncoding;

/// The response to a `network.fetch` call. Any status is returned, including
/// 4xx and 5xx; check `code` before reading `body`.
#[derive(TsInterface)]
pub struct NetworkResponse {
    /// The HTTP status code, such as `200` or `404`.
    pub code: u16,
    /// The response headers, keyed by lowercase name. A header sent more
    /// than once has its values joined with `", "`.
    pub headers: HashMap<String, String>,
    /// The response body: as text, or base64-encoded when fetched with
    /// `encoding: "base64"`.
    pub body: String,
}

impl NetworkResponse {
    /// The most of a response body that is read.
    pub const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

    /// Reads `response`, its body in `encoding`, failing if the body is over
    /// [`MAX_BODY_BYTES`](Self::MAX_BODY_BYTES).
    pub fn read(mut response: Response, encoding: NetworkBodyEncoding) -> io::Result<Self> {
        let code = response.status().as_u16();
        let mut headers: HashMap<String, String> = HashMap::new();
        for (name, value) in response.headers() {
            let value = String::from_utf8_lossy(value.as_bytes());
            headers
                .entry(name.as_str().to_string())
                .and_modify(|joined| {
                    joined.push_str(", ");
                    joined.push_str(&value);
                })
                .or_insert_with(|| value.into_owned());
        }
        let mut body = Vec::new();
        (&mut response)
            .take(Self::MAX_BODY_BYTES + 1)
            .read_to_end(&mut body)?;
        if body.len() as u64 > Self::MAX_BODY_BYTES {
            return Err(io::Error::other("the response body is over 32 MiB"));
        }
        Ok(Self {
            code,
            headers,
            body: match encoding {
                NetworkBodyEncoding::Text => String::from_utf8_lossy(&body).into_owned(),
                NetworkBodyEncoding::Base64 => STANDARD.encode(&body),
            },
        })
    }
}

impl<'js> IntoJs<'js> for NetworkResponse {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let object = Object::new(ctx.clone())?;
        object.set("code", self.code)?;
        object.set("headers", self.headers)?;
        object.set("body", self.body)?;
        Ok(object.into_value())
    }
}
