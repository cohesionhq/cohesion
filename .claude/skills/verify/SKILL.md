---
name: verify
description: Run the checks a change in the cohesion repo needs (cargo fmt, clippy, tests; the web UI's check, lint, test and build; regenerated protocol types) and tell your own failures from pre-existing ones. Use before every commit and before reporting any task as done, and whenever a check fails and you need to know whether you caused it.
---

# Verify a change

CI (`.github/workflows/checks.yml`) runs the same checks on every pull
request and push to `master`, but run them locally first. Run the
checks that cover what you touched, from the repo root:

| You changed | Run |
|---|---|
| Anything under `web/` | `cd web && npm run check && npm run lint && npm test && npm run build` |
| Any Rust | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` |
| A model, request, reply, invalidation or vocabulary word (anything `TsWire`) | also `cd web && npm run generate`, then commit `web/src/lib/protocol/generated.ts` and `vocabulary.json`, then the web checks |
| A model | it is in `every_model!` (`cohesion-db/src/db_macros.rs`); `cargo test -p cohesion-db every_model_matches_the_schema` |
| Docs only | nothing to run; grep every type and path you cite so it exists |

`cargo fmt --all` fixes formatting and `npm run format` (in `web/`) fixes
prettier; run them rather than hand-fixing. `cargo test -p <crate> <name>`
runs one test while iterating, but run the whole workspace before you
report. A cold `cargo test --workspace` takes a few minutes; run it in the
background and keep working.

A release build (`cargo build --release --workspace --locked`) is only
needed if you changed `build.rs`, the web bundle embedding or a dependency.

## What the checks prove

- Rust tests cover the database, migrations, the JS runtime and routes. A
  passing suite is real evidence.
- `npm test` (vitest) covers stores and helpers against the mock daemon,
  not Svelte components. A component or styling change is type-checked,
  linted and built, nothing more. Say "visual check pending", not "tested".
- `npm run generate` with no diff proves the TS types match the Rust ones.

## Is the failure mine?

The checkout is often dirty with someone else's in-progress work (see
`dirty-checkout`), and that work can break a check. Before you fix a
failure, find out whose it is:

1. Read the failure. If it is in a file you didn't touch and `git status`
   shows that file modified by someone else, it is probably theirs.
2. To be sure, run the same check on a clean copy of `HEAD`. Never stash or
   reset to get one:

   ```sh
   git worktree add --detach /tmp/cohesion-head HEAD
   (cd /tmp/cohesion-head && cargo test -p <crate> <name>)   # or the web check, after npm ci
   git worktree remove --force /tmp/cohesion-head
   ```

   Pass `CARGO_TARGET_DIR=<main checkout>/target` to reuse the main
   build cache instead of building from scratch.
3. Clean `HEAD` can itself fail to build when an earlier commit took only
   part of someone's change (a type used in a committed file but defined
   in an uncommitted one). That is pre-existing too: report it.
4. A failure on clean `HEAD` that isn't yours is pre-existing. Leave it
   alone and name it in the report's checks line. A failure caused only by
   someone else's uncommitted edits is also theirs: report it, don't fix
   it.

Don't run `npm run generate` in the main checkout to "check" it when
someone else has `generated.ts` modified. It rewrites the file. Use the
`HEAD` worktree, or only run it when you changed the Rust types yourself.
