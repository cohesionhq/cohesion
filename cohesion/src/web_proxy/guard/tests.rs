use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    routing::get,
};
use tower::ServiceExt;

use super::Guard;

const ADDR: &str = "127.0.0.1:4000";
const TOKEN: &str = "abc123";

fn router(guard: Guard) -> Router {
    guard.protect(
        Router::new()
            .route("/client", get(|| async { "ok" }))
            .route("/connect", get(|| async { "ok" })),
    )
}

fn launch() -> Router {
    router(Guard::launch(
        ADDR.parse::<SocketAddr>().unwrap(),
        TOKEN.into(),
    ))
}

async fn status(router: Router, request: Request<Body>) -> StatusCode {
    router.oneshot(request).await.unwrap().status()
}

fn client(host: &str) -> axum::http::request::Builder {
    Request::get("/client").header(header::HOST, host)
}

#[tokio::test]
async fn admits_the_token_on_the_bound_host() {
    let request = client(ADDR)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, format!("http://{ADDR}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), request).await, StatusCode::OK);

    let connect = Request::get(format!("/connect?token={TOKEN}"))
        .header(header::HOST, ADDR)
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), connect).await, StatusCode::OK);
}

#[tokio::test]
async fn rejects_a_wrong_or_missing_token() {
    let wrong = client(ADDR)
        .header(header::AUTHORIZATION, "Bearer nope")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), wrong).await, StatusCode::UNAUTHORIZED);

    // The query token only counts on `/connect`.
    let in_query = Request::get(format!("/client?token={TOKEN}"))
        .header(header::HOST, ADDR)
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), in_query).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_a_foreign_host() {
    let request = client("evil.test")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), request).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rejects_a_foreign_origin() {
    let request = client(ADDR)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, "http://evil.test")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), request).await, StatusCode::FORBIDDEN);

    // The dev server's origin is only trusted in `--dev`.
    let dev_origin = client(ADDR)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, "http://localhost:5173")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(launch(), dev_origin).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn dev_admits_the_vite_origin_with_the_token() {
    let dev = || router(Guard::dev(ADDR.parse().unwrap(), TOKEN.into()));
    let request = client(ADDR)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, "http://localhost:5173")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(dev(), request).await, StatusCode::OK);

    let tokenless = client(ADDR)
        .header(header::ORIGIN, "http://localhost:5173")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(dev(), tokenless).await, StatusCode::UNAUTHORIZED);
}
