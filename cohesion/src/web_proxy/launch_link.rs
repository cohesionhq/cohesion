//! The link `cohesion open` hands the browser. It carries a one-time nonce rather
//! than the launch token, so the token never appears on a command line or in
//! output: the nonce is exchanged, once and within seconds, for a redirect to the
//! UI with the token in the URL fragment.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use super::{ProxyState, launch_token};

/// How long a minted link works.
const LINK_LIFETIME: Duration = Duration::from_secs(30);

/// The one link that can currently be redeemed for the launch token.
pub struct LaunchLink {
    token: String,
    nonce: Mutex<Option<(String, Instant)>>,
}

impl LaunchLink {
    pub fn new(token: String) -> Arc<Self> {
        Arc::new(Self {
            token,
            nonce: Mutex::new(None),
        })
    }

    /// A new nonce, replacing any unredeemed one, good until `now` plus its lifetime.
    pub fn mint(&self, now: Instant) -> String {
        let nonce = launch_token::fresh();
        *self.slot() = Some((nonce.clone(), now + LINK_LIFETIME));
        nonce
    }

    /// The token, if `presented` is the minted nonce and it hasn't expired. Redeeming
    /// spends the nonce; a wrong guess leaves it for its owner.
    fn redeem(&self, presented: &str, now: Instant) -> Option<&str> {
        let mut slot = self.slot();
        match slot.as_ref() {
            Some((nonce, deadline)) if nonce == presented => {
                let live = now < *deadline;
                *slot = None;
                live.then_some(self.token.as_str())
            }
            _ => None,
        }
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, Option<(String, Instant)>> {
        self.nonce.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `GET /launch/{nonce}`: sends the browser to the UI with the token in the fragment.
pub async fn redeem(State(state): State<ProxyState>, Path(nonce): Path<String>) -> Response {
    match state.launch.redeem(&nonce, Instant::now()) {
        Some(token) => (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, format!("/#token={token}")),
                (header::CACHE_CONTROL, "no-store".to_string()),
                (header::REFERRER_POLICY, "no-referrer".to_string()),
            ],
        )
            .into_response(),
        None => (
            StatusCode::GONE,
            "This link has expired. Run `cohesion open` again for a new one.",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests;
