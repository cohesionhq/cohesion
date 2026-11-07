//! `Connector` in scripts.

use cohesion_core::model::Connector;
use cohesion_macros::js_entity;

/// A connected service as every account of it shares it: its `title`, what
/// it calls an account (`unit`), the `kinds` of item it imports and the
/// `metadataKeys` its items carry. `identifier` prefixes its resources'
/// `kind`.
#[js_entity(Connector)]
impl JsConnector {}
