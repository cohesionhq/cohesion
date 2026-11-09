# Design principles

How a change to Cohesion should be shaped. Where things are is in
[architecture.md](architecture.md); where a rule lives and the coding
conventions are in [conventions.md](conventions.md); who is trusted with what
is in [security-model.md](security-model.md).

Think like a systems architect: before writing code, decide where the
change belongs in the crate graph and the request flow, and leave the
system with fewer special cases than it had, not more.

- **Extend what exists.** Most features are a new case of something the
  daemon already does. A new trigger is a hook event on `HookManager`'s
  channel (timed ones come from `day_clock.rs`). One that waits for quiet,
  a settled hook like `journal:settled`, restarts its timer on each change
  and is flushed by `HookManager::settle_all` at shutdown: a new settled
  hook adds its case there, never its own shutdown step. State a stopped
  daemon must not leave behind (work marked running) is cleaned by one
  more `ctx.run` in `ShutdownCleanupCommand`, which closing the database
  runs as its last write and startup runs again after a crash. Something that wakes an
  agent is a cue; a new script capability is a `Mount` behind an extension
  permission (`Mount::for_permission`); state a client shows is a model
  whose commands raise invalidations. Reach for the existing seam (`Route`
  kinds, `DatabaseCommand`, `#[js_entity]`, `#[derive(Commands)]`, hook
  events) before adding a manager, channel, thread, table or dependency. If
  a second mechanism really is needed, say why in the commit.
- **SOLID, the way this codebase does it.** One reason to change per type
  and file: a command does one database thing, a route only adapts a
  request to the context. Grow behaviour through traits, derives and new
  types, not by widening a central `match` or adding flags to an existing
  function. A lower crate that needs something from a higher one declares a
  small trait for exactly what it uses and the higher crate implements it
  (`ConnectorAccess`, `ServiceAccess`), so dependencies keep pointing one
  way. Depend on the narrowest view that works (`ReadContainer`,
  `impl ReadableDatabase`), not the widest.
- **Lean on the type system.** Make invalid states unrepresentable: an enum
  rather than a string or a pair of bools, a type that says which case it
  is (`SandboxKey`) rather than a string with a convention, a typed marker
  rather than a runtime flag (`Access = Read`). Prefer an error the
  compiler raises (an exhaustive `match`, `FromRequest`'s exhaustive
  destructuring, a missing `ToJs` impl) to one a test or a reviewer has to
  catch, and when you add a rule, enforce it that way rather than
  documenting it. Fields that only make sense for one case live in that
  case's variant: a tagged enum for wire data (`AgentMessageContent`), a
  `#[derive(ColumnGroup)]` for nullable columns that come in sets
  (`EndpointBackend`). A value no caller may choose is not a variant of the
  enum callers choose from: an endpoint that signs in with an extension's
  account is `EndpointBackend::Account`, not an `IntelligenceEndpointPreset`
  that every create path must refuse. When you find a
  check for a case that shouldn't exist, look for the type that removes it
  before writing the check.
- **No speculative generality.** Build what the task needs: no options,
  traits or layers with one caller "for later". Simple and correct beats
  flexible.
- **Compatibility.** "Compatibility" in
  [product-direction.md](product-direction.md#compatibility) is the policy; in short, while the major version is `001`:
  - **Stored data is kept.** What a daemon has stored moves forward by a
    migration, never by asking the user to start again. Never edit or
    reorder a shipped migration.
  - **The wire moves with the daemon.** The web UI ships inside `cohesiond`,
    and the clients in the `cohesionhq` organization are updated alongside
    it, so nothing is built or kept for an older client, its saved
    configuration or its setup codes: no plumbing or fallback, no alias for
    an old name, no default or `Option` whose only reason is that an older
    client leaves the field out, no branching on a client's version. Change
    the shape outright and delete what it replaced in the same commit. A
    breaking wire change names each client it breaks and what that client
    needs, in the report and under "Unreleased" in `CHANGELOG.md`.
  - **The protocol version stays 1.** `PROTOCOL_VERSION` is 1 and isn't
    bumped, whatever the change: the web shows its out-of-date notice only
    when its number differs from the daemon's, so a bump would only block
    open pages. It moves when the project leaves active development and
    starts keeping older clients working.
