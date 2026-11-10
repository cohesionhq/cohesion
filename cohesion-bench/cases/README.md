# Bench cases

Each case is a directory, `<category>/<case-name>/`, that asks an agent for
one thing and checks whether it got done. The bench runs it on a scratch
daemon of its own (see `docs/architecture.md`, "Benchmarking models").

```
tasks-edit/reschedule-overdue/
  prompt.md      what the user says, with a little front matter
  validate.ts    whether the agent did it
  solution.ts    what a good agent would do
  setup.ts       optional: data only this case needs
```

## prompt.md

```markdown
---
title: Move overdue tasks to a date
difficulty: hard
---

Move all my overdue Personal tasks to {{day:3}}.
```

- `title` and `difficulty` (`easy`, `medium` or `hard`) are required. The
  difficulty weighs the case in the accuracy score (1, 2 or 3) and sets how
  much it may spend before its efficiency score drops.
- `state` is the starting state in `states/`: `default` when left out, or
  `none` for an empty `Personal` workspace.
- `workspaces` names the workspaces to make, comma-separated, when they
  aren't the state's; `timeout` is in seconds, 300 when left out.
- `{{day:N}}` is the date N days from today, as `YYYY-MM-DD`.

## validate.ts

```ts
export default function validate() {
  return B.check([
    [B.task('Return library books').dueDate === B.day(3), 'library books moved'],
    B.onlyChanged('Return library books', 'Renew the gym membership', 'Cancel the old phone contract'),
  ]);
}
```

Return nothing (or `false`) to pass and a string saying what's wrong to
fail. Throwing fails too. `B.check` turns `[ok, what]` pairs into exactly
that. For a question, `B.answered([...])` also fails the case if anything
changed. `reply` is what the agent told the user, and `bench` adds the
questions it asked, the cues it set and the memories it holds.

## solution.ts

```ts
export default function solution() {
  for (const t of B.ws().tasks({ dueDate: { before: B.day(0) } })) t.dueDate = B.day(3);
}
```

For a question, return the answer as a string. For a case about cues or
memories, return the `bench` a good agent would leave, such as
`{ memories: ['The user is vegetarian.'] }`.

## Checking cases

```sh
cargo run -p cohesion-bench -- validate                 # every case fails as it starts and passes after its solution
node web/node_modules/typescript/bin/tsc -p cohesion-bench/cases   # types, after any bench command
```

Both run without a model. `validate` also runs in `cargo test`, so a change
to the script API that breaks a case fails the tests.

## What's here

- `prelude.ts`: `B`, the helpers every script shares (`B.task`, `B.doc`,
  `B.day`, `B.check`, `B.unchanged`, `B.newTasks`, …).
- `states/default.ts`: the default starting state, a lived-in `Personal`
  workspace and an `Acme` work one. Cases rely on its titles and facts, so
  run `validate` after changing it.
- `types/bench.d.ts`: the bench's globals. Each bench command writes
  `types/cohesion.d.ts`, the script API's declarations, and git ignores it.
