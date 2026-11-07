//! The web UI's SvelteKit build, fetched from cohesiond and held in memory.

use std::{collections::HashMap, io::Read, sync::Arc};

use axum::{
    body::Bytes,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use cohesion_core::protocol::WebBundle;
use flate2::read::DeflateDecoder;

use crate::cli_error::CliError;

/// Every file in the build, by its path.
pub struct Assets {
    files: HashMap<String, Bytes>,
}

impl Assets {
    /// Inflates every file in `bundle`.
    pub fn unpack(bundle: &WebBundle) -> Result<Self, CliError> {
        let mut files = HashMap::with_capacity(bundle.files.len());
        for file in &bundle.files {
            let deflated = file.deflated().map_err(|_| CliError::UnexpectedReply)?;
            let mut data = Vec::new();
            DeflateDecoder::new(deflated.as_slice())
                .read_to_end(&mut data)
                .map_err(CliError::InvalidWebBundle)?;
            files.insert(file.path.clone(), Bytes::from(data));
        }
        Ok(Self { files })
    }

    /// The file at `path`, or `index.html` for any path the build doesn't have, so the
    /// UI's client-side routes load on refresh.
    fn find(&self, path: &str) -> Option<(&str, &Bytes)> {
        self.files
            .get_key_value(path)
            .or_else(|| self.files.get_key_value("index.html"))
            .map(|(path, data)| (path.as_str(), data))
    }
}

/// Serves the file at `uri`'s path.
pub async fn serve(State(assets): State<Arc<Assets>>, uri: Uri) -> Response {
    let Some((path, data)) = assets.find(uri.path().trim_start_matches('/')) else {
        return (StatusCode::NOT_FOUND, "the web UI has no index.html").into_response();
    };
    // SvelteKit content-hashes everything under `_app/immutable/`.
    let cache = if path.starts_with("_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    (
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        data.clone(),
    )
        .into_response()
}
