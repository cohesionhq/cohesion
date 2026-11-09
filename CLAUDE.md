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

This file holds only how to work here; the documentation is in `docs/`. Read
the doc a change touches before changing it, and keep it true in the same
commit ("Keeping the docs true" in `docs/conventions.md`).

## Documentation

- [docs/architecture.md](docs/architecture.md): the technical reference,
  from the crates and the wire protocol to agents, actors, clients,
  extensions, security and testing.
- [docs/product-direction.md](docs/product-direction.md): what the product is
  for (audiences, what each area and surface does, which surface gets a
  feature) and the compatibility policy, which is enough to act on. When a
  product question isn't answered there, ask the maintainers (in the issue or
  pull request) rather than guess.
- [docs/design-principles.md](docs/design-principles.md): how a change should
  be shaped (extend what exists, SOLID, the type system, no speculative
  generality, compatibility). Read it before any code change.
- [docs/conventions.md](docs/conventions.md): keeping the docs true, one rule
  in one place, layering between crates, and the code conventions (naming,
  tests, links, times, copy).
- [docs/security-model.md](docs/security-model.md): the trust model, the
  boundaries where input is validated and what an agent may do. Review
  against it.
- [docs/licensing.md](docs/licensing.md): the licence and trademarks.
- [docs/cli.md](docs/cli.md): using the CLI.
- [docs/self-hosting.md](docs/self-hosting.md): installing and running a
  server.
- [CONTRIBUTING.md](CONTRIBUTING.md): checks and the recipes for models,
  commands, CLI commands and script entities.
- [web/README.md](web/README.md): the web UI's layout, styling, Copy and
  components.
- Skills in `.claude/skills/`: `contributing` (branch, checks, docs,
  commit and report, around every task), `protocol-change`
  (anything on the wire), `web-ui-change` (anything under `web/src`),
  `copywriting` (text a user reads).

## Running it locally

To see a change working, use state that's yours to break: a scratch
daemon under `/tmp` ("Smoke test" in `CONTRIBUTING.md`) or the web UI in
mock mode (`VITE_MOCK=1 npm run dev` in `web/`).

- A daemon someone relies on, and the CLI paired with it: read from it if
  you need to, but don't write to it, restart it or deploy to it unless
  told.
- `dev/` is the developer's own state for `scripts/server.sh`,
  `scripts/client.sh` and `scripts/web-dev.sh`. Don't write to it on their
  behalf; copy it. `scripts/web-dev.sh` blocks until killed, so hand it to
  the person rather than running it.
- Stop every daemon and dev server you start.
- Behaviour a regression would quietly break belongs in a test, not only
  in a manual run.

## Decide, then report

Sessions here often run unattended, so keep going rather than stopping to
ask. On an open question take the
conservative choice and record it:

- **Architecture** (where a change sits, a new table, column, migration,
  model, or basic CRUD command or reply, a rule's home): extend the
  existing seam, make the narrowest schema change, and put the rule in one
  place ("One rule, one place" in `docs/conventions.md`).
- **Copy** (UI strings, CLI messages, docs prose): follow skill `copywriting`, reuse the words the product already uses,
  and pick the plainest option.

Mention every such choice in the report, architecture first. **Stop and ask** only for: a new dependency; a protocol frame or
message that isn't basic CRUD (a `ClientFrame`/`ServerMessage` variant,
something actor or capability clients must speak, `Hello`, pairing); a
change to what a trust boundary admits (`docs/security-model.md`);
something destructive or irreversible outside the repo (real data,
deploying); credentials; or a task that is truly blocked. Ask with a short recommendation.
