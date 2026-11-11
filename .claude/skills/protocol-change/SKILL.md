---
name: protocol-change
description: Add or change something on the cohesion wire from end to end: a model, table, migration, command, reply, invalidation or vocabulary word, through the daemon, generated TypeScript, web stores, mock, CLI and docs, and note the sibling clients it affects. Use for any change that touches cohesion-core's models or protocol, cohesion-db commands or migrations, or what clients receive.
---

# A protocol or schema change

Follow the recipes in `CONTRIBUTING.md` ("Add a model", "Add a command",
"Add a CLI command", "Expose an entity to scripts") rather than inventing
wiring. What you may decide alone and what needs asking is in "Decide,
then report" in `CLAUDE.md`; a new table, migration or CRUD command is
yours to decide, and goes first among the calls in your report.

## Checklist

Go through every step, even the ones that end up as "nothing to do":

1. **Core**: the model or request, `TsWire` on everything the web sees,
   and a vocabulary entry for any new kind a client shows.
2. **DB**: the migration (appended, never editing a shipped one), the
   command with its tests, and `every_model!` for a new model.
3. **Route**: automatic for a `Database` route, otherwise hand-written in
   `cohesionkit/src/routes/`.
4. **Generate**: `cd web && npm run generate`, and commit `generated.ts`
   and `vocabulary.json` with the change.
5. **Web**: the store refetches on its invalidation, and the mock has
   seed data (`protocol/mock/seed.ts`) and a handler in its area's file
   (`protocol/mock/handlers/`). A rule the mock copies from the daemon
   (table filters and sorts, link reading) gets a case in
   `table_cases.json` or `link_cases.json`, which both sides run.
6. **CLI**, if a person would use it from a terminal, with its line in
   `docs/cli.md`.
7. **Scripts**, if extensions or agents should reach it.
8. **Docs**: the "Wire protocol", "Actors" or "Extensions" section of
   `docs/architecture.md`.
9. **Web UI control**: a setting or feature the user controls at run time
   gets one (skill `web-ui-change`); nothing ships reachable only from
   the CLI or `cohesion api`.
10. **Other clients**: go through "Native clients and other repositories"
    in `docs/architecture.md` and name in the report each repo that needs
    a follow-up and what, and whether the iPhone app should gain the
    feature. Don't edit those repos from here.

## Breaking changes

Change the shape outright, with no alias, default or `Option` standing in
for what an older client lacks, and leave `PROTOCOL_VERSION` at 1
("Compatibility" in `docs/design-principles.md`). Name each client it
breaks under "Unreleased" in `CHANGELOG.md` and in the report.
