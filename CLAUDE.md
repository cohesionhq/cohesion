# CLAUDE.md

Rust workspace (edition 2024) for Cohesion: `cohesiond`, a daemon owning a
SQLite database, a QuickJS runtime and LLM connections, and `cohesion`, its
CLI. Start with `README.md`, then `docs/architecture.md` (crate graph,
request flow) and `CONTRIBUTING.md` (recipes for models, commands, script
entities). Follow those recipes rather than inventing new wiring. The iPhone
app (`apple`), the WebUI harnesses (`webshell`), the actor clients and Cohesion
Cloud live in their own repos (see "Native clients and other repositories" in
`docs/architecture.md`).
Cohesion is free software under `AGPL-3.0-only` (`docs/licensing.md`).

## Product direction

What the product is for (audiences, what each area and surface does, which
surface gets a feature) is in `docs/product-direction.md`, which is enough to
act on. When a product question isn't answered there, ask the maintainers (in
the issue or pull request) rather than guess.

Keeping the docs true is part of every change, not a follow-up: a change
that makes this file, `docs/`, `web/README.md`, `CONTRIBUTING.md`,
`README.md`, `CHANGELOG.md` or a skill wrong fixes them in the same commit
(the list is in skill `finish-task`), and a change that moves product
direction (a new capability, a changed default, a surface gaining or losing a
feature, a web change the iPhone app should mirror) says so in the report and
updates `docs/product-direction.md`.

## Design principles

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
- **Compatibility.** "Compatibility" in `docs/product-direction.md` is the
  policy; in short, while the major version is `001`:
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

## Decide, then report

Sessions here often run unattended, so keep going rather than stopping to
ask. On an open question take the
conservative choice and record it:

- **Architecture** (where a change sits, a new table, column, migration,
  model, or basic CRUD command or reply, a rule's home): extend the
  existing seam, make the narrowest schema change, and put the rule in one
  place (below).
- **Copy** (UI strings, CLI messages, docs prose): follow skill `copywriting`
  and "Copy" in `web/README.md`, reuse the words the product already uses,
  and pick the plainest option.

List every such choice under "Choices made" in the report, architecture
first. **Stop and ask** only for: a new dependency; a protocol frame or
message that isn't basic CRUD (a `ClientFrame`/`ServerMessage` variant,
something actor or capability clients must speak, `Hello`, pairing); a
change to what a trust boundary admits; something destructive or
irreversible outside the repo (real data, deploying); credentials; or a
task that is truly blocked. Ask with a short recommendation.

## Working in this checkout

- Work on the branch you're given; use a worktree only when the work needs
  isolation. Don't push unless asked. Changes reach `master` by pull request,
  and the `Checks` workflow runs on each.
- The checkout may be dirty with the owner's or another session's work.
  Never revert, stash or reset it, and commit only the paths you changed,
  named on the commit. A file you changed that also holds someone else's
  edits goes in whole, theirs included; no hunk-splitting, no
  `--no-verify` (skill `dirty-checkout`).
- Run the checks before reporting (skill `verify`), then close out with
  skill `finish-task`.
- Don't deploy (`scripts/install-server.sh`), restart the daemon or write
  real data unless told to.

Skills in `.claude/skills/`: `verify`, `dirty-checkout`, `finish-task`,
`protocol-change`, `web-ui-change`, `run-locally`.

## Trust model

Validate at these boundaries and nowhere else:

- **Before pairing**: `/register` with the setup key, the Unix socket's
  handshake and file permissions, and the `cohesion open` loopback proxy.
  A stranger on the LAN or a local process gets nothing else.
- **A connection's address** is a claim, not a credential. It is the peer's own
  unless the peer is named by `--proxy-protocol` (the pass-through Router), whose
  PROXY v2 header is then required and believed; no other peer can claim one.
  The header is unauthenticated, so someone on the path can write false addresses
  into what clients show, and that must stay all they can do: nothing is ever
  admitted or refused by address ("Behind a pass-through router" in
  `docs/architecture.md`).
- **Wire input** from a paired client is well-formed or refused, but a
  paired client is trusted: it may do anything the protocol offers, except
  open pairing when the security settings keep that to clients on this
  computer (a `FromClient` route, the one kind that sees its sender).
  A client's offers and call results are wire input.
- **Extensions** are trusted once installed and a grant is on; a grant
  decides which mounts a scope gets, and nothing inside a mount
  second-guesses the grant.
- **The LLM** is semi-trusted: it reads untrusted text (articles, feeds,
  imported resources). Everything an agent does is a turn of its
  conversation or a fork of it (one turn on a copy of the conversation's
  thread, or, for a cue, on a fresh thread of its own), and its rights follow from one thing, who started its chain
  (`Initiative`, on the trace, inherited by every fork the chain wakes and
  kept on an actor session's follow): `AgentRole`
  (`cohesion-js/src/engine/agent_role.rs`) alone turns that into mounts and
  a database. A capability a role lacks is a mount it doesn't have or a
  database that refuses the write, never a check inside a mount.
  - **Started by the user** (a message, a pick, and the questions and
    actor follow-ups that descend from them): everything the conversation
    may do.
  - **Started by an event** (a cue firing): reads, sends notifications,
    works with actors, and asks with `ask_choice` before any change; no
    `network`, no writes, no cues or memories set. The user's pick starts
    a chain they started. A chain never gains rights.
  - **A cue's function**, deciding whether its fork wakes, reads and sends
    notifications, nothing more.

  Forks are counted in hops and logged, never limited; the spend limits
  hold them. Hooks raising hooks stop at the depth limit (`MAX_DEPTH`, 3).
- **External services** (providers' APIs, models.dev, fetched pages and
  feeds) are untrusted: parse defensively, bound sizes and time.

Review against real threats to these boundaries, not hypothetical ones a
trusted party could mount.

## One rule, one place

A rule is enforced once, at the lowest layer that can hold it, in this
order: a schema constraint (`UNIQUE … COLLATE NOCASE`, `CHECK`, a foreign
key's cascade, a trigger), then a type (an enum whose variants carry the
fields that only make sense together), then one command, then a route.
Clients never recompute a rule the daemon already applied: they show what
the reply says (a client is online because the clients list says so, a name is taken because the command
refused it). When you find the
same rule in two places, replace both with one and say so in the commit.

## Layering

`cohesion` (CLI) depends only on `cohesion-core`; it must never link
rusqlite, rquickjs or the intelligence crate. Dependencies flow one way:
`cohesiond → cohesionkit → cohesion-intelligence → cohesion-js → cohesion-db →
cohesion-core ← cohesion-macros`, with `cohesion-internet` (reading the web)
between cohesion-js and cohesion-core. `cohesion-internet-harness` is a tool
on top of cohesion-internet that nothing depends on. No crate uses cargo
features.

## Conventions

- Happy-path code. Validate only at untrusted boundaries (wire input,
  external APIs); don't add defensive checks for states the types or the
  database already rule out.
- Model structs serialize snake_case (no `#[serde(rename_all)]` on structs);
  enums use `rename_all = "camelCase"`. JS getters are camelCase.
- Never edit or reorder a shipped entry in `MIGRATIONS`; append a new one.
- Reads a client sees use `select_live` so soft-deleted rows stay hidden.
- Read commands are named for what they return, and only the unusual case
  says more. `Get<Entity>Command` returns one live row by id.
  `Get<Entity>IncludingDeletedCommand` returns it even once soft-deleted
  (a hook about a deletion still needs the row). `Lookup<Entity>By<Key>Command`
  returns one live row by another key (`LookupWorkspaceByTitleCommand`).
  `List<Entity>sCommand` takes filters and returns the live rows that
  match; one entity has one List command, and a paged variant for clients
  is `List<Entity>PageCommand`. Metadata filters (`metadata`,
  `has_metadata`) are for scripts only: `FromRequest` always starts them
  empty, and a request carrying them does not compile.
- A new model goes in `every_model!`.
- The vocabulary is agents (whose messages are agent messages, held in model
  threads), actors (whose whole units of work are actor sessions) and workspace
  tasks. A client that is nothing but the WebUI in a web view is a WebUI harness. An actor session's connection-level cousin is a client's session; say
  which you mean.
- Items link to each other as `cohesion://task/<ws>/<id>`,
  `cohesion://document/<ws>/<id>`, `cohesion://agent/<id>` and
  `cohesion://actor-session/<id>` (see "The web UI" in `docs/architecture.md`).
  Every client opens a link to one-row content (an item shown as one row in
  a list with a small detail view: a task) over the current screen, never by
  navigating away: a sheet in the iPhone app. The web UI opens every link
  and forwarded item that way, in one side panel (`linkedItem`, `ItemPanel`)
  the same for every kind but its content: all the item's details, then
  its text, and for full-page content (documents, journal entries, agent
  chats, actor session transcripts) an Open link to its page. A new link kind
  gets a panel of its own on `ItemPanel`, and the web UI's `/open/…` pages
  stay the fallback for a link opened elsewhere.
- `///` comments on script entities become the `.d.ts` the LLM code tool
  sees; write them for a reader of the JS API. The code tool declares only
  each comment's first paragraph up front, so open with one sentence saying
  what the item is.
- Unit tests live out of line: `#[cfg(test)] mod tests;` at the end of the
  source file, with the tests in `foo/tests.rs` (or a sibling `tests.rs` for
  `lib.rs`/`mod.rs`). Never write an inline `mod tests { … }`.
- Write tests for low-level behaviour a regression would quietly break:
  the JS runtime (sandbox isolation, worker survival, deadlines, hooks
  calling into scripts, read-only enforcement), transactions, and
  migrations and triggers. Drive them through real scripts against an
  in-memory database (`cohesion-js/src/tests.rs`). Skip wire-format tests,
  tests the compiler already guarantees, tests that pin generated strings
  (SQL text, error wording, full JSON of a getter), and tests that depend
  on timing, thread scheduling or a worker count.
- Agent avatars, on every client, size the glyph by fixed shares of the
  disc, never by eye: "Avatars" under "Styling" in `web/README.md`.
- The page's session API is stable: `web/src/lib/protocol/session.ts`
  (`#token=`, `/client`, `/connect?token=`), the parts of `WebSocket`
  the connection uses, and `--titlebar-inset`, which the WebUI harnesses set. `cohesion open` and the WebUI harnesses'
  `CohesionWebKit` bridge (in `cohesionhq/webshell`) stand in for them;
  change all three together or not at all (see "The page's side is a stable API" in
  `docs/architecture.md`).
- A change to `Cargo.lock` regenerates `THIRD_PARTY_NOTICES.md`
  (`node scripts/third-party-notices.mjs`).
- Times show on a 12-hour clock (`3:14 PM`) on every client, whatever the
  locale's habit; on the web through `shortTime` (`web/src/lib/format/dates.ts`).
- Web UI text follows skill `copywriting` and "Copy" in `web/README.md`, and every Svelte
  transition takes `motion.duration` (`web/src/lib/ui/motion.ts`), never a
  literal; see "Motion" under "Styling" there.
- Match the surrounding comment density and naming. Each crate root opens
  with a `//!` summary; keep it current when a crate's role changes.
