//! The PKCE verifier of each extension's sign-in under way, kept between
//! the request that opens the sign-in page and the one that brings its code
//! back, so clients send nothing but the code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cohesion_core::auth::Pkce;
use cohesion_core::model::PkceChallenge;

/// The verifier of the latest sign-in started at each extension, by
/// extension id: starting another replaces it, and completing one takes it.
/// Held in memory, so a restart between the two loses it. A cheap clone;
/// clones share it.
#[derive(Clone, Default)]
pub(crate) struct PendingSignIns(Arc<Mutex<HashMap<i64, String>>>);

impl PendingSignIns {
    /// A new PKCE pair for a sign-in at `extension_id`, keeping its verifier
    /// and answering its challenge.
    pub(crate) fn begin(&self, extension_id: i64) -> PkceChallenge {
        let Pkce {
            verifier,
            challenge,
        } = Pkce::generate();
        self.0.lock().unwrap().insert(extension_id, verifier);
        challenge
    }

    /// The verifier of the sign-in under way at `extension_id`, if one is,
    /// no longer kept.
    pub(crate) fn take(&self, extension_id: i64) -> Option<String> {
        self.0.lock().unwrap().remove(&extension_id)
    }
}
