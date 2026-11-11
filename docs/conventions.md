# Conventions

The rules for changing the code: keeping the docs true, where a rule is
enforced, which crate may depend on which, and the coding conventions. The
reasoning behind them is in [design-principles.md](design-principles.md), the
crates themselves in [architecture.md](architecture.md), and step-by-step
recipes in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Keeping the docs true

Keeping the docs true is part of every change, not a follow-up: a change
that makes `CLAUDE.md`, `docs/`, `web/README.md`, `CONTRIBUTING.md`,
`README.md`, `CHANGELOG.md` or a skill wrong fixes them in the same commit
(what each doc covers is in "Documentation" in `CLAUDE.md`). The same
holds for a crate's `//!` summary when its role changes, the `///` comments
on script entities (the LLM reads them as `.d.ts`), a `CHANGELOG.md` entry
under "Unreleased" for anything a user would notice or any breaking wire
change with the clients it breaks, and `THIRD_PARTY_NOTICES.md` after a
change to `Cargo.lock`. A change that moves product
direction (a new capability, a changed default, a surface gaining or losing a
feature, a web change the iPhone app should mirror) says so in the report and
updates [product-direction.md](product-direction.md).

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

`cohesion-rs` sits under cohesion-core and depends on no workspace crate. It
is `MIT` so clients outside this repository can link it, and it holds only
what a client needs to talk to a daemon. Something whose reason to exist is
serving or storing the daemon's state stays in cohesion-core; where a client
type would need it, the client keeps that part as raw JSON instead
(`docs/licensing.md`).

## Code conventions

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
  `cohesion://document/<ws>/<id>`, `cohesion://resource/<ws>/<id>`,
  `cohesion://document-folder/<ws>/<id>`, `cohesion://contact/<ws>/<id>`,
  `cohesion://agent/<id>`,
  `cohesion://actor-session/<id>` and `cohesion://article/<id>` (see "The web UI" in [architecture.md](architecture.md#16-the-web-ui)).
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
  disc, never by eye: "Avatars" under "Styling" in [web/README.md](../web/README.md).
- The page's session API is stable: `web/src/lib/protocol/session.ts`
  (`#token=`, `/client`, `/connect?token=`), the parts of `WebSocket`
  the connection uses, and `--titlebar-inset`, which the WebUI harnesses set. `cohesion open` and the WebUI harnesses'
  bridges (`CohesionWebKit`, and `gtk/` in `cohesionhq/webshell`) stand in for them;
  change them all together or not at all (see "The page's side is a stable API" in
  [architecture.md](architecture.md#the-pages-side-is-a-stable-api)).
- A change to `Cargo.lock` regenerates `THIRD_PARTY_NOTICES.md`
  (`node scripts/third-party-notices.mjs`).
- Times show on a 12-hour clock (`3:14 PM`) on every client, whatever the
  locale's habit; on the web through `shortTime` (`web/src/lib/format/dates.ts`).
- Web UI text follows skill `copywriting`, and every Svelte
  transition takes `motion.duration` (`web/src/lib/ui/motion.ts`), never a
  literal; see "Motion" under "Styling" there.
- Match the surrounding comment density and naming. Each crate root opens
  with a `//!` summary; keep it current when a crate's role changes.
