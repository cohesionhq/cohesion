//! The command trait, what a command may do ([`Read`] or [`Write`], and
//! which [`Area`] a write changes) and sees while it runs, and the link from
//! a wire request to the command that carries it out.

use std::marker::PhantomData;

use cohesion_core::invalidation::InvalidationContext;
use cohesion_core::model::ExtensionPermissionKind;

use crate::changed::Changed;
use crate::command_error::DatabaseCommandResult;
use crate::container::{Container, ReadContainer, WriteContainer};
use crate::database_event::{DatabaseEventTrace, EventContext};
use crate::query::ReadableDatabase;
use crate::secrets::Secrets;

/// Everything a command can use while it runs. Its database work happens
/// inside a savepoint that is rolled back if the command returns `Err`.
pub struct CommandContext<'a, A: Access = Write> {
    /// The connection, inside the command's savepoint, seen through the view
    /// `A` picks.
    pub db: A::Database<'a>,
    /// Keys to publish once the command succeeds; dropped if it fails. A
    /// write adds to it only through what it returns ([`Changed`]) and what
    /// [`run`](CommandContext::run) collects.
    invalidations: &'a mut InvalidationContext,
    /// Server secrets, e.g. the setup key.
    pub secrets: &'a Secrets,
    /// Unix seconds when the command started.
    pub now: i64,
    /// Events to deliver once the command succeeds; dropped if it fails.
    pub events: &'a mut EventContext,
    /// The current event trace.
    pub trace: &'a DatabaseEventTrace,
}

impl<'a, A: Access> CommandContext<'a, A> {
    /// The context a command runs in, collecting its keys in `invalidations`.
    pub(crate) fn new(
        db: A::Database<'a>,
        invalidations: &'a mut InvalidationContext,
        secrets: &'a Secrets,
        now: i64,
        events: &'a mut EventContext,
        trace: &'a DatabaseEventTrace,
    ) -> Self {
        Self {
            db,
            invalidations,
            secrets,
            now,
            events,
            trace,
        }
    }

    /// Runs `cmd` as this context's command, keeping the keys it changed
    /// to publish once it commits.
    pub(crate) fn handle<C: DatabaseCommand<Access = A>>(
        &mut self,
        cmd: C,
    ) -> DatabaseCommandResult<C::Response> {
        let output = cmd.handle(self)?;
        Ok(A::finish(output, self.invalidations))
    }
}

impl<A: Area> CommandContext<'_, Write<A>> {
    /// Runs write command `cmd`, of any area, inline, as part of this one:
    /// its keys join this command's and its response is returned. Read
    /// commands run inline through [`as_read`](Self::as_read).
    pub fn run<B: Area, C: DatabaseCommand<Access = Write<B>>>(
        &mut self,
        cmd: C,
    ) -> DatabaseCommandResult<C::Response> {
        let mut inner = CommandContext::<Write<B>> {
            db: self.db,
            invalidations: self.invalidations,
            secrets: self.secrets,
            now: self.now,
            events: self.events,
            trace: self.trace,
        };
        inner.handle(cmd)
    }

    /// This context narrowed to reads, for running a read command inline.
    pub fn as_read(&mut self) -> CommandContext<'_, Read> {
        CommandContext {
            db: self.db.read(),
            invalidations: self.invalidations,
            secrets: self.secrets,
            now: self.now,
            events: self.events,
            trace: self.trace,
        }
    }
}

/// Access for a command that only reads; its `ctx.db` is a [`ReadContainer`].
pub struct Read;

/// Access for a command that may write to area `A`; its `ctx.db` is a
/// [`WriteContainer`]. A plain `Write` changes the user's data.
pub struct Write<A: Area = UserData>(PhantomData<A>);

/// The area of a write that changes the user's data: workspaces, their
/// contents, agents and everything else not set apart as configuration.
pub struct UserData;

/// The area of a write that changes intelligence settings: endpoints, the
/// models they offer and the models each tier uses.
pub struct IntelligenceSettings;

/// The area of a write that changes what an agent keeps of its own: its
/// memories and cues. No extension permission grants it, and a turn an
/// event started may not write it.
pub struct AgentSettings;

/// The area of a write that sends the user a notification: it changes no
/// rows, so it takes no lock, and only raises its hook and invalidation. The
/// one area a background run may write.
pub struct Notifications;

/// Which area a write command changes, as a value, for a guard deciding
/// whether a script may run it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteArea {
    UserData,
    IntelligenceSettings,
    AgentSettings,
    Notifications,
}

impl WriteArea {
    /// Every area: what a guard that may write anything holds.
    pub const ALL: &[WriteArea] = &[
        WriteArea::UserData,
        WriteArea::IntelligenceSettings,
        WriteArea::AgentSettings,
        WriteArea::Notifications,
    ];

    /// The area an extension permission lets the extension's scripts write,
    /// if any.
    pub fn for_permission(permission: &ExtensionPermissionKind) -> Option<WriteArea> {
        match permission {
            ExtensionPermissionKind::GlobalReadWrite => Some(WriteArea::UserData),
            ExtensionPermissionKind::Intelligence => Some(WriteArea::IntelligenceSettings),
            ExtensionPermissionKind::Notifications => Some(WriteArea::Notifications),
            ExtensionPermissionKind::GlobalReadOnly
            | ExtensionPermissionKind::Network
            | ExtensionPermissionKind::Timer
            | ExtensionPermissionKind::Registry
            | ExtensionPermissionKind::OAuth
            | ExtensionPermissionKind::Connector
            | ExtensionPermissionKind::CalendarAccount
            | ExtensionPermissionKind::Service
            | ExtensionPermissionKind::ServiceIsolated
            | ExtensionPermissionKind::Services
            | ExtensionPermissionKind::Usage
            // Starts and talks to actor sessions through the daemon, which
            // records them itself; no area of the script's own.
            | ExtensionPermissionKind::Actors => None,
        }
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Read {}
    impl<A: super::Area> Sealed for super::Write<A> {}
    impl Sealed for super::UserData {}
    impl Sealed for super::IntelligenceSettings {}
    impl Sealed for super::AgentSettings {}
    impl Sealed for super::Notifications {}
}

/// Sealed: the area a [`Write`] command changes, which decides who may run
/// it from a script.
pub trait Area: sealed::Sealed + Send + 'static {
    const AREA: WriteArea;
}

impl Area for UserData {
    const AREA: WriteArea = WriteArea::UserData;
}

impl Area for IntelligenceSettings {
    const AREA: WriteArea = WriteArea::IntelligenceSettings;
}

impl Area for AgentSettings {
    const AREA: WriteArea = WriteArea::AgentSettings;
}

impl Area for Notifications {
    const AREA: WriteArea = WriteArea::Notifications;
}

/// Sealed: what a command may do. Picks the view of the connection its
/// `handle` sees. A [`Read`] command's view has no `insert`, `update` or
/// `delete`, so a command that writes cannot be declared `Read`.
///
/// Known limit: inside this crate a `Read` command could still write through
/// raw SQL, by way of `ctx.db.container()`. The guarantee covers the builder
/// API and everything outside the crate.
pub trait Access: sealed::Sealed + Send + 'static {
    /// The view a command with this access gets.
    type Database<'c>: ReadableDatabase<'c>;
    /// What `handle` returns for a `T` response: the response for a read,
    /// and the response with the keys it changed ([`Changed`]) for a write,
    /// so no write can leave them out.
    type Output<T>;
    /// Whether a command with this access changes the user's data, the only
    /// writes that can reach what Rewind keeps, so the worker notes who it
    /// writes as first.
    const CHANGES_USER_DATA: bool;
    /// Whether a command with this access only reads, so it may run on a
    /// read-only connection beside the writer.
    const READ_ONLY: bool;
    /// The view over `container`.
    fn view(container: &Container) -> Self::Database<'_>;
    /// The response in `output`, its keys added to `invalidations`.
    fn finish<T>(output: Self::Output<T>, invalidations: &mut InvalidationContext) -> T;
}

impl Access for Read {
    type Database<'c> = ReadContainer<'c>;
    type Output<T> = T;
    const CHANGES_USER_DATA: bool = false;
    const READ_ONLY: bool = true;

    fn view(container: &Container) -> ReadContainer<'_> {
        ReadContainer::new(container)
    }

    fn finish<T>(output: T, _: &mut InvalidationContext) -> T {
        output
    }
}

impl<A: Area> Access for Write<A> {
    type Database<'c> = WriteContainer<'c>;
    type Output<T> = Changed<T>;
    const CHANGES_USER_DATA: bool = matches!(A::AREA, WriteArea::UserData);
    const READ_ONLY: bool = false;

    fn view(container: &Container) -> WriteContainer<'_> {
        WriteContainer::new(container)
    }

    fn finish<T>(output: Changed<T>, invalidations: &mut InvalidationContext) -> T {
        output.finish(invalidations)
    }
}

/// A unit of database work run on the `DatabaseContext` worker thread.
pub trait DatabaseCommand: Send + 'static {
    /// What the command produces on success.
    type Response: Send + 'static;
    /// Whether the command only reads ([`Read`]) or may write ([`Write`]).
    type Access: Access;
    /// Runs the command. Returning `Err` rolls back everything it wrote. A
    /// [`Write`] command returns its response as [`Changed`], naming the
    /// keys it changed.
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<<Self::Access as Access>::Output<Self::Response>>;
}

/// A request that runs as a database command. Implemented by
/// `#[derive(FromRequest)]` on the command, so each request has one. The
/// `Reply: Send` bound restates what `DatabaseCommand::Response` already
/// requires, so generic routes can rely on it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no database command",
    note = "add `#[derive(FromRequest)] #[request({Self})]` to its command in cohesion-db/src/commands/"
)]
pub trait DbRequest: cohesion_core::protocol::Call<Reply: Send> + Send + 'static {
    /// The command that carries out this request.
    type Command: DatabaseCommand<Response = Self::Reply> + From<Self>;
}
