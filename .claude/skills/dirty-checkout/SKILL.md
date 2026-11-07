---
name: dirty-checkout
description: Work and commit safely in a cohesion checkout that holds someone else's uncommitted or staged changes (the owner's, or another Claude session's). Use at the start of every task (read git status) and before every commit, and whenever git status shows files you didn't change.
---

# Working in a dirty checkout

The person you work for or another session may be mid-change in the
checkout, with files modified, staged or
untracked. Their work is not yours to tidy.

## At the start

Run `git status --short` and note which paths are already dirty, and
whether any are staged (first column). Those are someone else's.

## Never

- `git stash`, `git reset`, `git checkout -- <file>`, `git restore`,
  `git clean`, or reverting someone else's hunks, for any reason.
- `git add -A`, `git add .` or `git commit -a`.
- A plain `git commit` with no paths. The index may already hold someone
  else's staged files, and a bare commit takes all of them.
- Committing someone else's untracked files (`c.db`, scratch files, a new
  module they haven't finished).
- `--no-verify`, or splitting a file's hunks to keep theirs out.

## Committing

Name your paths on the commit itself, so only they go in, whatever else
is staged:

```sh
git add <new files you created>             # new files must be added first
git commit -F - -- <every path you changed> <<'EOF2'
feat(web): …
EOF2
```

`git commit -- <paths>` commits exactly those paths' working-tree
contents and leaves the rest of the index as it was. Deletions you made
with `git rm` are included when you name the path.

**A file you changed that also holds someone else's edits goes in whole,
their hunks included**. Don't split hunks. Name each such
file under Caveats in the report ("`cohesionkit/src/lib.rs` also carried
someone else's uncommitted <what> hunks"). If you can avoid touching a file
someone else is mid-change in, do; if their staged edits are about to be
committed by them, prefer to wait and edit after.

The pre-commit hook (`.githooks/pre-commit`) runs `cargo fmt --all` and
`cargo clippy --fix` across the workspace and prettier on staged web
files, then re-stages the committed files. Let it run. It can reformat
other people's files in the working tree; that's expected, leave them.

After committing, `git show --stat HEAD` must list only the files you
meant. If it doesn't, say so in the report rather than rewriting history
that may already be pushed.

## When their work breaks the build

If someone's uncommitted change fails a check, it is pre-existing (see
`verify`). Don't fix, revert or work around it in their files. Report it,
with the failing check and file.
