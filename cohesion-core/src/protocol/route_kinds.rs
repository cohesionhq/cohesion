//! The kinds a command can route `via`, each naming what carries it out.

/// Runs on the database worker through a `DatabaseCommand` (the default kind).
pub struct Database;

/// Runs on the script engine.
pub struct Script;

/// Runs on the intelligence features.
pub struct Intelligence;

/// Runs anywhere, has no dependancies.
pub struct Local;

/// Runs knowing which client sent it; a route of this kind is the only one
/// that may treat the sender differently from any other client.
pub struct FromClient;
