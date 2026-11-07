//! Fixtures shared by the crate's unit tests.

use std::sync::{Arc, Mutex};

use crate::changed::{Changed, Invalidates};
use cohesion_core::LocalIdentity;
use cohesion_core::model::ActorDefinition;
use cohesion_core::protocol::ClientCapabilities;

use crate::{
    CommandContext, Container, CreateActorSessionCommand, DatabaseCommand, DatabaseCommandResult,
    DatabaseContext, DatabaseEvent, DatabaseEventKind, DatabaseEventListener,
    IdentifyLocalClientCommand, ListActorsCommand, ReadableDatabase, Secrets,
    SyncCapabilitiesCommand, Write,
};

/// A context over a fresh, migrated in-memory database that nothing listens to.
pub(crate) fn context() -> DatabaseContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    DatabaseContext::new(container, Secrets::new(), ())
}

/// A new actor session, on the one actor this computer's client offers, for
/// what only needs a session to hang on (an activity).
pub(crate) fn actor_session(db: &DatabaseContext) -> i64 {
    let client_id = db
        .send_blocking(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "MacBook".to_string(),
                name: "MacBook".to_string(),
            },
        })
        .unwrap()
        .id;
    let actors = vec![ActorDefinition {
        identifier: "cohesion".to_string(),
        product: "Claude Code".to_string(),
        scope: "/src/cohesion".to_string(),
        modes: Vec::new(),
        models: Vec::new(),
    }];
    db.send_blocking(SyncCapabilitiesCommand {
        client_id,
        offers: ClientCapabilities { actors },
    })
    .unwrap();
    let actor_id = db.send_blocking(ListActorsCommand).unwrap()[0].id;
    db.send_blocking(CreateActorSessionCommand {
        actor_id,
        prompt: "Fix the build".to_string(),
        title: None,
        mode: None,
        model: None,
        effort: None,
        attachments: Vec::new(),
    })
    .unwrap()
    .0
    .id
}

/// Collects every event a context's commands raise.
#[derive(Clone, Debug, Default)]
pub(crate) struct RecordedEvents(Arc<Mutex<Vec<DatabaseEventKind>>>);
impl RecordedEvents {
    /// Takes the events recorded so far. The worker delivers a command's
    /// events before it replies, so they are here once the command returns.
    pub(crate) fn take(&self) -> Vec<DatabaseEventKind> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}
impl DatabaseEventListener for RecordedEvents {
    fn send(&self, event: DatabaseEvent) {
        self.0.lock().unwrap().push(event.kind);
    }
}

/// Like [`context`], but recording the events its commands raise.
pub(crate) fn recording_context() -> (DatabaseContext, RecordedEvents) {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let events = RecordedEvents::default();
    (
        DatabaseContext::new(container, Secrets::new(), events.clone()),
        events,
    )
}

/// Runs one statement as written, for a test to set up what no command
/// writes, such as a row's age.
pub(crate) struct RawSql(pub(crate) &'static str);
impl DatabaseCommand for RawSql {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db.container().execute_statement(self.0, [])?;
        Ok(().invalidating_nothing())
    }
}
