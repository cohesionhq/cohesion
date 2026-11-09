---
name: protocol-change
description: Add or change something on the cohesion wire from end to end: a model, table, migration, command, reply, invalidation or vocabulary word, through the daemon, generated TypeScript, web stores, mock, CLI and docs, and note the sibling clients it affects. Use for any change that touches cohesion-core's models or protocol, cohesion-db commands or migrations, or what clients receive.
---

# A protocol or schema change

The recipes are in `CONTRIBUTING.md` ("Add a model", "Add a command",
"Expose an entity to scripts"); follow them in order rather than inventing
wiring. This skill is the checklist around them.

## Decide or ask

Decide these yourself, take the narrowest option, and list them first
under "Choices made":

- a new table, column or migration (append to `MIGRATIONS`; never edit a
  shipped entry);
- basic CRUD commands and replies (`Get…`, `List…`, `Create…`, `Update…`,
  `Delete…`) and their invalidations;
- where a rule lives, by "One rule, one place" in `docs/conventions.md`.

**Stop and ask** before:

- a new dependency (any crate or npm package);
- a protocol frame or message that isn't basic CRUD: a new `ClientFrame`
  or `ServerMessage` variant, something actor or capability clients must
  speak, a change to `Hello`, pairing or the session handshake;
- a change to what a trust boundary admits (see `docs/security-model.md`).

## Checklist

1. **Core**: the model or request in `cohesion-core`, `TsWire` on
   everything the web sees, a vocabulary entry for any new kind a client
   shows (`cohesion-core/src/protocol/vocabulary.rs`; it won't compile
   without one).
2. **DB**: the migration, the command in
   `cohesion-db/src/commands/<domain>.rs` with tests in
   `<domain>/tests.rs`, `every_model!` for a new model. Reads clients see
   use `select_live`. Naming follows "Code conventions" in `docs/conventions.md`.
3. **Route**: a `Database` route is automatic; anything else is a
   hand-written `Route` in `cohesionkit/src/routes/<domain>_routes.rs`.
4. **Generate**: `cd web && npm run generate`. Commit `generated.ts` and
   `vocabulary.json` with the change.
5. **Web**: the store in `web/src/lib/stores/` refetches on its
   invalidation (`matches`, see `stores/README.md`); add seed data and a
   handler to `web/src/lib/protocol/mock.ts` so mock mode and the store
   tests keep working.
6. **CLI**, if a person would use it from a terminal: "Add a CLI command"
   in `CONTRIBUTING.md`, with its line in `docs/cli.md`. Otherwise
   `cohesion api` reaches it, with no change.
7. **Scripts**, if extensions or agents should reach it: `#[js_entity]`,
   behind the right mount, with `///` comments written for the JS reader.
8. **Docs**: `docs/architecture.md`: its "Wire protocol" section for frames
   and requests, "Actors" for actor-facing frames, "Extensions" for the
   script API and permissions.
9. **Web UI control**: a setting or feature the user controls at run time
   gets a control in the web UI (`web-ui-change`); the daemon never ships
   one only the CLI or `cohesion api` can reach.
10. **Sibling clients**: go through "Other clients" below and name, under
    Caveats, each repo that needs a follow-up and what: the iPhone app's
    hand-written `Commands.swift` and models for any model or command it
    uses, `cohesion-ts` for a frame, CohesionWebKit and `cohesion open` for
    the page's session API. Under Product direction, say whether the iPhone
    app should gain the feature.
11. **Verify**: the cargo trio and the web checks (`verify`).

## Other clients

The other repos read this protocol (see "Native clients and other
repositories" in `docs/architecture.md`), all under `github.com/cohesionhq`:

- `apple` (the iPhone app) has hand-written Swift models
  (`Cohesion/Models/`) and commands (`Cohesion/Connection/Commands.swift`);
  a renamed or removed field breaks it.
- `ts`, the TypeScript library used by the actor clients (`actor-agentsdk`,
  `actor-opencode`, `actor-codex`), copies this repo's
  `generated.ts` (`bun run generate`) and hand-writes the session frames
  in `src/frames.ts` from `envelope.rs` and `capabilities.rs`.
- `webshell`, the WebUI harnesses and their CohesionWebKit bridge,
  show the web UI and depend on the page's session API
  ("The page's side is a stable API" in `docs/architecture.md`).

Don't edit those repos from here. Nothing is kept for an older client
(see "Compatibility" in `docs/design-principles.md`): change the shape outright, with no
alias, default or `Option` standing in for what an older frame lacks. A
breaking change does not bump `PROTOCOL_VERSION` (`envelope.rs`; it stays 1
during active development), and the report and `CHANGELOG.md` name each of
those repos it breaks and what they need. A new command or model they should use needs
a follow-up there too. The web's own generated types and mock (steps 4 and
5) are this repo's, so they change in the same commit.
