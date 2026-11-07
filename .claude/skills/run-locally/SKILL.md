---
name: run-locally
description: Run the cohesion daemon, CLI or web UI to see a change working without touching real state: scratch config directories, short sockets and mock mode. Use when a change needs exercising beyond unit tests (a migration, a route, daemon behaviour, a CLI command), and before running anything that could write to a database.
---

# Running Cohesion without touching real state

Keep real data and test data apart:

1. **A daemon you rely on**: the one holding your real data, and the
   `cohesion` CLI paired with it. Read from it if you need to; don't write
   to it, restart it or deploy to it unless told.
2. **`dev/`**: the developer's own local state for `scripts/server.sh`
   and `scripts/client.sh`, git-ignored. Don't write to it on someone's
   behalf. Copy it instead.
3. **A scratch directory**: yours to break. A daemon's database is in WAL
   mode: copy a running one with `sqlite3 ... ".backup <file>"`, not `cp`
   alone.

## Smoke test the daemon and CLI

```sh
cargo build --workspace
rm -rf /tmp/cohesion-smoke
./target/debug/cohesiond -c /tmp/cohesion-smoke --socket /tmp/cs.sock --port 3100 > /tmp/cs.log 2>&1 &
pid=$!
until [ -S /tmp/cs.sock ]; do sleep 0.5; done
./target/debug/cohesion -c /tmp/cohesion-smoke/client setup --local /tmp/cs.sock
./target/debug/cohesion -c /tmp/cohesion-smoke/client api '{"type":"listWorkspaces"}'
kill $pid
```

- Pass `--port` with a free port (3000 is the default, which a daemon you
  rely on may hold) and `--log-level debug` to see SQL.
- Always pass a short `--socket /tmp/<name>.sock`. A socket under a deep
  config directory exceeds the Unix socket path limit.
- `setup --local` points the scratch client config at your socket.
- To see a new migration run on real-shaped data, copy an existing
  database into the scratch directory first (`sqlite3 ... ".backup"`): the
  debug daemon migrates the copy.
- A copy of a real database carries its installed extensions and their
  accounts' tokens, and the daemon runs each extension's `onLaunch` as it
  starts: an extension that refreshes an OAuth token (Todoist) spends the
  original daemon's refresh token, and the provider may revoke it. Before
  starting on such a copy, turn extensions off in it with `sqlite3`, or
  migrate the copy from a test instead of starting the daemon.
- Run it in the background and `kill` the pid. Don't leave daemons
  running.
- A fresh scratch directory starts empty: seed what you need through
  `api` calls or a script.

## The web UI

- `VITE_MOCK=1 npm run dev` in `web/` needs no daemon (`web-ui-change`).
- `scripts/web-dev.sh` runs Vite and the CLI proxy against `dev/client`.
  It blocks until killed, so hand it to the person you work for in the report rather than
  running it yourself.
- Against your scratch daemon, see "Web UI" in `CONTRIBUTING.md`.

## Scripts

`./target/debug/cohesion -c /tmp/cohesion-smoke/client run script.js`
runs a script in the scratch daemon. For behaviour a regression would
quietly break, write a test in `cohesion-js/src/tests.rs` instead
(in-memory database, real scripts).
