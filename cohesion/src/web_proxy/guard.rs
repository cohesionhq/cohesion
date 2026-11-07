//! What keeps other local users' software and web pages out of the proxy: requests must
//! name the loopback address it bound and carry the launch token.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
};

/// Where the Vite dev server serves the UI from in `--dev`.
const DEV_ORIGINS: [&str; 2] = ["http://localhost:5173", "http://127.0.0.1:5173"];

/// Who the proxy's API routes answer.
#[derive(Clone)]
pub struct Guard {
    token: String,
    host: String,
    origins: Vec<String>,
}

impl Guard {
    /// Admits requests to `addr` carrying `token`, from no page or one `addr` serves.
    pub fn launch(addr: SocketAddr, token: String) -> Self {
        Self {
            token,
            host: addr.to_string(),
            origins: vec![format!("http://{addr}")],
        }
    }

    /// Admits requests to `addr` carrying `token`, also from the Vite dev server's pages.
    pub fn dev(addr: SocketAddr, token: String) -> Self {
        let mut origins = vec![format!("http://{addr}")];
        origins.extend(DEV_ORIGINS.map(String::from));
        Self {
            token,
            host: addr.to_string(),
            origins,
        }
    }

    /// `router`'s routes, each behind this guard.
    pub fn protect<S: Clone + Send + Sync + 'static>(&self, router: Router<S>) -> Router<S> {
        router.route_layer(from_fn_with_state(Arc::new(self.clone()), check))
    }

    /// `router`'s routes, answering only requests to the bound address, with no token:
    /// for the one route that hands the token out.
    pub fn protect_host<S: Clone + Send + Sync + 'static>(&self, router: Router<S>) -> Router<S> {
        router.route_layer(from_fn_with_state(Arc::new(self.clone()), check_host))
    }

    fn admits_origin(&self, headers: &HeaderMap) -> bool {
        let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
        let origin = headers.get(header::ORIGIN).map(|v| v.to_str().ok());
        host == Some(self.host.as_str())
            && match origin {
                None => true,
                Some(origin) => origin.is_some_and(|o| self.origins.iter().any(|a| a == o)),
            }
    }

    fn admits_token(&self, request: &Request) -> bool {
        // Browsers can't set headers on a WebSocket upgrade, so `/connect` takes it in the query.
        let presented = if request.uri().path() == "/connect" {
            request
                .uri()
                .query()
                .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("token=")))
        } else {
            request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        };
        presented == Some(self.token.as_str())
    }
}

async fn check_host(State(guard): State<Arc<Guard>>, request: Request, next: Next) -> Response {
    if !guard.admits_origin(request.headers()) {
        return (StatusCode::FORBIDDEN, "forbidden host or origin").into_response();
    }
    next.run(request).await
}

async fn check(State(guard): State<Arc<Guard>>, request: Request, next: Next) -> Response {
    if !guard.admits_origin(request.headers()) {
        return (StatusCode::FORBIDDEN, "forbidden host or origin").into_response();
    }
    if !guard.admits_token(&request) {
        return (StatusCode::UNAUTHORIZED, "missing or wrong launch token").into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests;
