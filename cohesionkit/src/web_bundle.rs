//! The web UI this daemon was built with, which `build.rs` compresses file by
//! file. Clients learn its hash from `Hello` or `WebBundleHash` and fetch it
//! with `WebBundle` (see `docs/serving-the-webui.md`).

use std::sync::LazyLock;

use cohesion_core::protocol::WebBundle;

/// Each file's path and raw-deflated bytes; empty when the UI wasn't built.
const WEB_FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/web_files.rs"));

static BUNDLE: LazyLock<Option<WebBundle>> =
    LazyLock::new(|| (!WEB_FILES.is_empty()).then(|| WebBundle::new(WEB_FILES.iter().copied())));

/// The web UI, unless this daemon was built without one.
pub fn web_bundle() -> Option<&'static WebBundle> {
    BUNDLE.as_ref()
}

/// The web UI's hash, as `Hello` and `WebBundleHash` report it.
pub fn web_bundle_hash() -> Option<String> {
    web_bundle().map(|bundle| bundle.hash.clone())
}
