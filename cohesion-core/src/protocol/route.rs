//! How a context executes a request: [`Command::dispatch`](super::Command::dispatch)
//! asks the context for `Route<Kind, Request>`, where the kind
//! ([`Database`](super::Database), [`Script`](super::Script),
//! [`Intelligence`](super::Intelligence) or [`Local`](super::Local)) is picked
//! by the command's `via`.

use std::future::Future;

use super::call::Call;
use super::server_error::ServerError;

/// How a context carries out requests of type `R` that route via `Kind`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot route `{R}` via `{Kind}`",
    note = "for `Database`, the request needs a command in cohesion-db with `#[derive(FromRequest)] #[request({R})]`",
    note = "for any other kind, write `impl Route<{Kind}, {R}> for {Self}` in the request's domain file under cohesionkit/src/routes/ (agent, extension, connector, ...)"
)]
pub trait Route<Kind, R: Call> {
    /// Runs `request` and produces its typed reply.
    fn route(&self, request: R) -> impl Future<Output = Result<R::Reply, ServerError>> + Send;
}
