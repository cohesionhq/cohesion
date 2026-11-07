//! Types shared by every cohesion crate: the domain models, the wire
//! protocol (clients' frames and offers among it), keys, and the
//! storage-agnostic value layer. Nothing here depends
//! on a database or script engine. `ts_wire` declares the protocol as
//! TypeScript for the web UI.

// Lets `#[derive(Model)]`'s `::cohesion_core::...` paths resolve inside this crate too.
extern crate self as cohesion_core;

// Lets `#[derive(TextEnum)]` implement serde's traits without each user
// naming serde.
#[doc(hidden)]
pub use serde as __serde;

mod core_macros;

pub mod auth;
pub mod caller;
pub mod invalidation;
pub mod log;
pub mod model;
pub mod paths;
pub mod private_files;
pub mod protocol;
pub mod stored_model;
pub mod ts_documentation;
pub mod ts_wire;
pub mod value;

/// What this build is: the `vXXXYYYYY` the image workflow gave it, or the crate
/// version in a build it didn't make.
pub const VERSION: &str = match option_env!("COHESION_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

pub use auth::{
    Authentication, Certificate, DEFAULT_PORT, Host, InvalidHost, KeyError, LocalHandshakeReply,
    LocalIdentity, ServerAddress, SetupCode, SetupCodeError, fingerprint, key_fingerprint,
};
pub use caller::{Caller, CallerClient, Transport};
pub use cohesion_macros::{
    ColumnGroup, Fields, FromRequest, Model, Projection, TextEnum, TsInterface, TsWire,
};
pub use invalidation::{Invalidation, InvalidationBatch, InvalidationContext};
pub use model::{
    Client, ClientKeyKind, DayFilter, DayRange, Document, DocumentType, Task, TaskAssignee,
    TaskFolder, TaskFolderFilter, TaskPriority, TaskSize, TaskSort, TaskStatus, Workspace,
};
pub use stored_model::{Column, ColumnGroup, Model, Named, Projection, SingleTable, SoftDelete};
pub use ts_documentation::TsDocumentation;
pub use ts_wire::TsWire;
pub use value::Value;
