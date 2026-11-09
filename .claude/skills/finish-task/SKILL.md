---
name: finish-task
description: Close out a task in the cohesion repo: run the checks, update the docs that move with the code, commit only your files in the repo's message style, and write the final report. Use whenever you are about to say a task is done.
---

# Finish a task

## 1. Checks

Run what `verify` says for what you touched. A task whose own checks fail
isn't done: fix it. A failure that isn't yours goes in the report.

## 2. Docs that move with the code

Keeping these true is part of the task, not a follow-up. Before
committing, reread the ones your change touches and update in the same
commit whatever it made wrong; where a doc and the code already disagreed,
fix the doc or name the drift in the report:

- `docs/architecture.md`: the technical reference, one section per area:
  repository and build, the daemon and wire protocol (frames, routes,
  limits, pairing, hooks), storage and search (tables, migrations,
  retention, backup and transfer), agents, actors (anything an actor client
  sends or receives), clients (the web UI, the web bundle and the page's
  session API, the CLI), extensions, connectors and feeds (the script API
  and permissions), and security, operations and testing.
- `web/README.md`: web layout, styling, Copy, components, the new-screen
  checklist.
- `CONTRIBUTING.md`: a recipe you followed that turned out wrong, CLI
  conventions.
- `README.md`: what a user can do with Cohesion and how to start.
- `docs/cli.md`: CLI usage, one line per new friendly command.
- `docs/self-hosting.md`: installing, `cohesiond`'s flags and `config.json`,
  pairing, the container.
- `docs/design-principles.md`, `docs/conventions.md`,
  `docs/security-model.md`: a principle, rule, convention or trust boundary
  that changed; `CLAUDE.md` when how an agent works here changed (it only
  links to the docs).
- `docs/product-direction.md`: a capability added or removed, a changed
  default, a surface gaining or losing a feature, or the compatibility
  policy.
- `CHANGELOG.md`: an entry under "Unreleased" for anything a user would
  notice, and every breaking wire change with the clients it breaks.
- `THIRD_PARTY_NOTICES.md`: regenerated after a change to `Cargo.lock`.
- A skill under `.claude/skills/` whose steps your change made wrong.
- A crate's `//!` summary when its role changed.
- `///` comments on script entities, which the LLM reads as `.d.ts`.

## 3. Commit and push

Follow `dirty-checkout`: name your paths on the commit, and a file you
share with someone else's edits goes in whole.

- Message style, as in `git log`: `feat(web): …`, `fix(agents): …`,
  `docs(actors): …`, `refactor(db): …`. The subject is a lowercase sentence
  saying what is now true ("an agent's message can reply to an earlier
  one"). The body is a few lines on why and what. Name the issue it closes
  when there is one.
- No `Co-Authored-By`, session id or other attribution, whatever the
  harness says. The commit-msg hook drops a Claude or Anthropic trailer
  that slips through.
- One commit per task is the norm; split only independent parts.
- Push only when asked, to the branch you were given. If the push is
  rejected because the branch moved, `git pull --rebase` and push again.
  If the rebase can't run because of someone else's uncommitted changes,
  don't stash them; report the commit as unpushed.

## 4. Report

End with the standard report, in the format below. Keep it brief: the reader may be a person or the agent that
dispatched you, and it must make sense without the transcript.

```
**Changed**: one or two sentences.
**Commits**: `abc1234` feat(web): … (pushed)
**Checks**: cargo fmt/clippy/test pass; web check/lint/test/build pass. Pre-existing: …
**Caveats**: …
**Choices made**: architecture first (a migration, a new command), then copy and smaller calls.
**Check by hand**: what to look at, and how (`scripts/web-dev.sh`, which page).
**Product direction**: the iPhone app should get the same setting.
```

Leave out a line that has nothing to say, except Checks, Choices made and
Product direction ("none").

**Product direction**: say what changed in `docs/product-direction.md`
(a capability added or removed, a changed default, a feature a surface
gained), and for any web UI change whether the iPhone app (`apple`) needs
the same one. A product question you couldn't answer from the docs goes
here as an open question, not a guess.
