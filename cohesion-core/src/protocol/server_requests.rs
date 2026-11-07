//! Requests about the server itself, and the reply for requests that only
//! acknowledge.

use base64::{DecodeError, Engine, engine::general_purpose::STANDARD};
use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::auth::fingerprint;

#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct AboutRequest;
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct AboutResponse {
    pub hostname: String,
    pub version: String,
}

/// Asks for the web UI the daemon was built with.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WebBundleRequest;
/// The web UI's static build, for a client to serve.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WebBundle {
    /// Identifies this build: any change to the files changes it, so a client
    /// may keep what it unpacked for as long as the hash is unchanged.
    pub hash: String,
    pub files: Vec<WebFile>,
}

/// Asks for the hash of the web UI the daemon was built with, without the
/// files: a client checks it against what it is showing, then fetches the
/// [`WebBundle`] only when the two differ.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WebBundleHashRequest;
/// The hash of the web UI the daemon was built with.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WebBundleHash {
    /// The [`WebBundle`]'s hash, the same value as `Hello`'s
    /// `web_bundle_hash`; `None` when the daemon was built without the UI.
    pub hash: Option<String>,
}

/// One file of the web UI's build.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WebFile {
    /// Relative to the build's root, with `/` separators, e.g. `_app/version.json`.
    pub path: String,
    /// The file's bytes, compressed as raw deflate (RFC 1951, no zlib or gzip
    /// wrapper), base64-encoded.
    pub data: String,
}

impl WebBundle {
    /// The bundle of `files`, each a path and its raw-deflated bytes, in a fixed order.
    pub fn new<'a>(files: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> Self {
        let mut hashed = Vec::new();
        let files = files
            .into_iter()
            .map(|(path, deflated)| {
                // Lengths keep one file's path or bytes from running into the next's.
                for part in [path.as_bytes(), deflated] {
                    hashed.extend_from_slice(&(part.len() as u64).to_le_bytes());
                    hashed.extend_from_slice(part);
                }
                WebFile {
                    path: path.to_string(),
                    data: STANDARD.encode(deflated),
                }
            })
            .collect();
        Self {
            hash: fingerprint(&hashed),
            files,
        }
    }
}

impl WebFile {
    /// The file's raw-deflated bytes.
    pub fn deflated(&self) -> Result<Vec<u8>, DecodeError> {
        STANDARD.decode(&self.data)
    }
}

/// Opens a pairing window.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct BeginPairingRequest;
/// Answers whether the client asking may open pairing now.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetPairingAccessRequest;
/// Whether the client asking may open pairing now.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct PairingAccess {
    /// Why it can't, as `BeginPairing` would refuse it; `None` when it can.
    pub refusal: Option<String>,
}

/// The setup code a new client pairs with.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct PairingCode {
    /// A [`SetupCode`](crate::SetupCode), encoded: it works for one
    /// registration, for ten minutes.
    pub code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct AcceptedResponse;
impl From<()> for AcceptedResponse {
    fn from(_value: ()) -> Self {
        Self {}
    }
}
