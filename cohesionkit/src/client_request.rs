//! A request as the client that sent it: what every transport dispatches.
//! Every route but a [`FromClient`](cohesion_core::protocol::FromClient) one runs on the [`CohesionContext`]
//! alone, as if no client had asked; a `FromClient` route also sees which
//! client did, for what treats the sender differently from the others
//! (opening pairing, marking the client asking in the clients list,
//! revoking every other client).

use cohesion_core::Client;
use cohesion_core::protocol::{Call, Database, Intelligence, Local, Route, Script, ServerError};

use crate::CohesionContext;

/// The context a request runs against, and the client that sent it.
pub struct ClientRequest<'a> {
    pub context: &'a CohesionContext,
    pub client: &'a Client,
}

/// Routes of every kind but `FromClient` run on the context.
macro_rules! on_the_context {
    ($($kind:ty),*) => {$(
        impl<R: Call> Route<$kind, R> for ClientRequest<'_>
        where
            CohesionContext: Route<$kind, R>,
        {
            fn route(
                &self,
                request: R,
            ) -> impl Future<Output = Result<R::Reply, ServerError>> + Send {
                <CohesionContext as Route<$kind, R>>::route(self.context, request)
            }
        }
    )*};
}

on_the_context!(Database, Script, Intelligence, Local);
