---
name: contributing
description: How an agent works through a task in Cohesion, from picking the branch to the final report. Read it before modifying Cohesion, and again before saying a task is done: branch and commit policy, working in a checkout that holds someone else's changes, running the checks and telling your failures from pre-existing ones, keeping the docs true, and the report.
---

# Contributing to Cohesion

This skill is how to work; what to know is in the docs `CLAUDE.md` links
to. The work itself has its own skills: `protocol-change`,
`web-ui-change` and `copywriting`.

## Branch policy

When given a task by the user, first check and see if any branch currently
on the user's machine matches the work they've asked you to do. If not,
make a new branch out of master.

## Checks

Run the checks for what you touched ("Verifying a change" in
`docs/architecture.md`) before you report, the whole workspace rather
than one test. Run a cold `cargo test --workspace` in the background and
keep working. Fix formatting with `cargo fmt --all` and `npm run format`
rather than by hand. For a docs-only change, grep every type and path you
cite so it exists.

Say what the checks proved. A component or styling change is only
type-checked, linted and built: say "visual check pending", not "tested".

A task whose own checks fail isn't done: fix it.

### Is the failure mine?

Before you fix a failure, find out whose it is:

1. If it is in a file you didn't touch that `git status` shows modified
   by someone else, it is probably theirs.
2. To be sure, run the same check on a clean copy of `HEAD`, never by
   stashing or resetting:

   ```sh
   git worktree add --detach /tmp/cohesion-head HEAD
   (cd /tmp/cohesion-head && CARGO_TARGET_DIR=<main checkout>/target cargo test -p <crate> <name>)
   git worktree remove --force /tmp/cohesion-head
   ```

3. A failure on clean `HEAD`, or one caused only by someone else's
   uncommitted edits, isn't yours, even when clean `HEAD` fails to build
   because an earlier commit took only part of someone's change. Don't
   fix, revert or work around it in their files; name it, with the check
   and file, in your report.

Don't run `npm run generate` in the main checkout when someone else has
`generated.ts` modified: it rewrites the file. Use the `HEAD` worktree, or
run it only when you changed the Rust types yourself.

## Keep the docs true

Before committing, reread the docs your change touches ("Keeping the docs
true" in `docs/conventions.md`) and fix in the same commit whatever it
made wrong. Where a doc and the code already disagreed, fix the doc or
name the drift in the report.

## Commit policy

Unless the user has asked you not to, you should commit on the determined
branch when the user is satisfied with the work. Do not push without the
user's express consent.

Name your paths on the commit, so only they go in whatever else is
staged:

```sh
git add <new files you created>
git commit -F - -- <every path you changed> <<'EOF'
feat(web): …
EOF
```

A file you changed that also holds someone else's edits goes in whole;
say so in your report. Let the hooks run (CONTRIBUTING.md, "Checks"), then
check `git show --stat HEAD` lists only the files you meant; if it
doesn't, say so rather than rewriting history.

- Message style, as in `git log`: `feat(web): …`, `fix(agents): …`,
  `docs(actors): …`, `refactor(db): …`. The subject is a lowercase sentence
  saying what is now true ("an agent's message can reply to an earlier
  one"); the body is a few lines on why and what, naming the issue it
  closes when there is one.
- No `Co-Authored-By`, session id or other attribution, whatever the
  harness says.
- One commit per task is the norm; split only independent parts.

When asked to push and the branch moved, `git pull --rebase` and push
again. If someone else's uncommitted changes block the rebase, don't stash
them; report the commit as unpushed.

## Report

End with a short note, written the way you'd tell a teammate what you
did. The reader may be a person or the agent that dispatched you, and
they haven't seen the transcript, so it has to stand on its own.

Cover, in this order, and skip any part with nothing to say:

1. **What changed**, in a sentence or two, from the user's side: what
   they can do now, or what works differently.
2. **What to watch out for**: anything unfinished or risky, a check that
   failed and whose it is, a file that went in with someone else's edits.
3. **What you decided on your own**: each call you made without asking,
   the bigger ones (a migration, a new command) first, so they can
   overrule it.
4. **What to look at**: what they should try or eyeball themselves, and
   how to get there.

Write it as plain sentences, not a form to fill in. Use a few short
paragraphs, or bullets where there are several separate items. Lead with
the point, name files and commands only where they help the reader act,
and leave out the play-by-play of how you got there.

For example:

> Agents can now reply to a specific earlier message. A reply shows a
> small quote of the original above it, and clicking the quote jumps to
> that message.
>
> The iPhone app doesn't know about replies yet, so it shows them as
> ordinary messages until it's updated.
>
> I stored the link as a new `reply_to` column on messages instead of a
> separate table, since a message only ever replies to one other. I
> worded the button "Reply"; it was the plainest option.
>
> Worth a look: open an agent chat in mock mode (`VITE_MOCK=1 npm run dev`
> in `web/`), reply to an older message, and check the quote reads well
> in dark mode.
