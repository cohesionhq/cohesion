# Contributing

[docs/architecture.md](docs/architecture.md) explains how the crates fit
together; read it first. The rules a change follows are in
[docs/design-principles.md](docs/design-principles.md),
[docs/conventions.md](docs/conventions.md) and
[docs/security-model.md](docs/security-model.md). This file covers the checks
and the recipes for common changes.

## Before you start

You'll need what the [README](README.md#getting-started) lists: Rust (rustup
installs the version pinned in `rust-toolchain.toml`, with `rustfmt` and
`clippy`), a C compiler and Node.js 22 or later. For a change bigger than a
small fix, open an issue first so the approach can be agreed before you write it.
Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).

Pull requests go to `master`. Keep each one to one change, describe what it
changes and why, and add an entry under "Unreleased" in
[CHANGELOG.md](CHANGELOG.md) when users would notice it. By opening a pull
request you agree to license your contribution under `AGPL-3.0-only`, or
`MIT` in `cohesion-rs` ([Licensing](docs/licensing.md)).

## Checks

The `Checks` workflow (`.github/workflows/checks.yml`) runs on every pull
request and push to `master`. Run the same checks before you push:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

and the web UI's checks (`npm ci`, `npm run check`, `npm run lint`,
`npm test` and `npm run build`, in `web/`). `npm run generate` rewrites
`web/src/lib/protocol/generated.ts` and `vocabulary.json` beside it from the
Rust protocol types; after it, `git diff` on both must be empty. A change to
the protocol types therefore needs Node and a regenerated `generated.ts`, even
from a Rust-only contributor.

After cloning, enable the repo's git hooks once:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook runs `cargo fmt` and `cargo clippy --fix` and re-stages
the staged Rust files, and runs prettier on staged files under `web/` when
`web/node_modules` exists. A file with both staged and unstaged changes is
re-staged whole. The commit-msg hook drops AI attribution trailers (a
Claude or Anthropic `Co-Authored-By`, "Generated with Claude Code"); a
human co-author stays.

`THIRD_PARTY_NOTICES.md` is generated too: after a change to `Cargo.lock`, run
`node scripts/third-party-notices.mjs` and commit the result.

## Smoke test

Run the daemon and CLI end to end against throwaway state:

```sh
cargo build --workspace
rm -rf /tmp/cohesion-smoke                            # a fresh, empty config each run
./target/debug/cohesiond -c /tmp/cohesion-smoke --socket /tmp/cs.sock --log-level debug &
pid=$!
./target/debug/cohesion -c /tmp/cohesion-smoke/client setup --local /tmp/cs.sock
./target/debug/cohesion -c /tmp/cohesion-smoke/client api '{"type":"listWorkspaces"}'
kill $pid
```

Gotchas:

- The default socket lives inside the config directory, and under a deep
  directory that path exceeds the Unix socket path limit (about 104 bytes).
  Pass `--socket /tmp/<short>.sock`.
- The default port, 3000, may belong to a daemon you rely on. Pass
  `--port` with a free one.
- macOS has no `timeout`. Background the daemon and `kill` its pid.
- To run a migration on real-shaped data, copy a database into the scratch
  directory with `sqlite3 <db> ".backup <file>"`, not `cp`: it is in WAL
  mode.
- A copied database carries its extensions and their accounts' tokens, and
  the daemon runs each extension's `onLaunch` as it starts. One that
  refreshes an OAuth token (Todoist) spends the original daemon's refresh
  token, and the provider may revoke it. Turn extensions off in the copy
  with `sqlite3` first, or migrate it from a test instead.
- `journal edit` and `journal append` create today's journal row, so run them
  against scratch state, never a database you care about.

## Development scripts

`scripts/server.sh` runs the debug daemon with `-c ./dev`, and
`scripts/client.sh` runs the debug CLI with `-c ./dev/client`. Both forward
their arguments. `dev/` is git-ignored local state that the daemon creates the
first time it runs. Connect the CLI to it once, with the daemon running:

```sh
scripts/server.sh &
scripts/client.sh setup --local dev/cohesiond.sock
scripts/client.sh workspace create Home
```

`scripts/web-dev.sh` runs the web UI's live-reload loop against the same
config (see `web/README.md`).

## Publish an image

Published images live in `ghcr.io/cohesionhq/cohesion`, a package linked to this
repo. There are two ways to publish one. Building and running an image
is in [docs/self-hosting.md](docs/self-hosting.md#run-in-a-container).

Releasing is the usual way. Run the `Release` workflow
(`.github/workflows/release.yml`) from GitHub's Actions tab: it takes no input.
It runs `scripts/release.sh`, which tags the head of `master` with its version, pushes the tag and creates a GitHub release listing the
commits since the last release. Then it runs the `Image` workflow
(`.github/workflows/image.yml`), which builds the amd64 image from the tag and
pushes it tagged with the version, signed in with the workflow's own
`GITHUB_TOKEN`. Pushes to `master` publish nothing. Running `scripts/release.sh`
on your own computer does the same, with `gh` signed in, and the tag it pushes
starts the `Image` workflow. The script reads only the local repository: it
releases the checked-out commit, works out the version from the local tags, and
stops if that commit is already released.

A version is `vXXXYYYYY`, such as `v00100042`. `XXX` is the major version, set by
hand in `MAJOR_VERSION`. `YYYYY` is the minor version: the script finds the last
tag of the major version and adds 1. It starts again when the major version
changes, so the first release after `MAJOR_VERSION` changes to `002` is
`v00200000`. Both binaries are built knowing the version, and `cohesion status`
shows it. A build without it, such as `cargo build` or `publish-image.sh`, shows
the crate version.

`scripts/publish-image.sh [commit]` is the local fallback. The script builds a clean copy
of the commit (default `HEAD`) with Podman, so nothing uncommitted gets in, and
pushes it to the same package. The tag is the commit's short hash, the
`org.opencontainers.image.revision` label holds the full one, and the script
prints the reference to pull by digest. The script signs in with `gh auth token`, so `gh`
needs the scope first: `gh auth refresh -s write:packages`.

## Web UI

`web/` is a SvelteKit app built to static files that cohesiond embeds and
`cohesion open` serves (see `web/README.md`, including how styling works with
Tailwind). Its checks:

```sh
cd web
npm ci
npm run check && npm run lint && npm test && npm run build
```

`web/src/lib/protocol/generated.ts` is written from cohesion-core's
`TsWire` types by `npm run generate`; after changing a model, request or
reply, regenerate it and commit the result.

cohesiond embeds `web/build/` and hands it to any paired client that sends
`WebBundle`; `cohesion open` fetches it at launch and serves it from memory,
so the UI always matches the daemon ("The web UI" in `docs/architecture.md`
has the details, for writing another client). It is embedded at compile time, so run `npm run build` before
building `cohesiond` (`scripts/install.sh` and `scripts/install-server.sh` do). With no build
present the workspace still compiles (`web/build/.gitkeep` keeps the
directory), with a warning, and `WebBundle` fails until one exists.

For UI work, `scripts/web-dev.sh` runs the proxy in dev mode and Vite with hot
reload (http://localhost:5173) against the dev client config, sharing a
random token it makes for the run. Against a scratch daemon (above), do the
same by hand: the proxy and Vite both read the token from the environment.

```sh
export COHESION_DEV_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
./target/debug/cohesion -c /tmp/cohesion-smoke/client open --dev --port 4000 --no-browser
cd web && npm run dev                      # http://localhost:5173
```

`VITE_MOCK=1 npm run dev` runs the UI against an in-memory mock daemon, with
no CLI or daemon at all.

## Tests

Test the low-level machinery: the JS runtime and how scripts interact with
the database, transactions, and migrations and triggers. Don't test the
wire format.
Prefer one behavioural assertion over pinning generated text, and never
depend on timing or on how many workers the pool has. Leave out anything
the type system already enforces.

- `every_model_matches_the_schema` in `cohesion-db/src/migrations/tests.rs`
  prepares and inserts every model against the migrated schema; add a new
  model to the `every_model!` list in `cohesion-db/src/db_macros.rs`.
- `Container::in_memory()` plus `migrate()` gives a full database for unit
  tests (borrow it with `WriteContainer::new(&container)` to query it directly); `context()` in `cohesion-db/src/test_support.rs` wraps one in a
  `DatabaseContext`. A command's tests live next to it in
  `cohesion-db/src/commands/<domain>/tests.rs`; those and
  `cohesion-js/src/tests.rs` show the pattern.

## Recipes

### Add a model

1. Define the struct in `cohesion-core/src/model/<name>.rs`:
   ```rust
   /// A named thing in a workspace.
   #[derive(Clone, Serialize, Deserialize, Model)]
   #[model(table = "things", backup = Included)]
   pub struct Thing { pub id: i64, pub workspace_id: i64, pub name: String, pub deleted: bool }
   ```
   `backup` is required: `Included`, or `LeftOut` for bytes a backup should
   not carry (`BackupClass`). Every field is a column; `id: i64` is required. `#[model(skip)]` marks a
   non-column field; `#[model(json)]` and `#[model(group)]` store a field as
   JSON or as several columns (see "Store a field as JSON" below), and
   `join`/`expr`/`or_from` read a field through another table (see "A model
   that joins"). The derive also names the model for client-facing messages
   (`Thing::NAME`, from the struct's name in lower words: `TaskFolder` is
   "task folder"), so name the struct as a client would say it.
   Field types need `IntoValue`/`FromValue`
   (`cohesion-core/src/value/value_conversion.rs`); a
   text enum column derives them with `#[derive(TextEnum)]`, as `DocumentType`
   does. Add `#[text_enum(store = "integer")]` to store the discriminant
   instead (an `INTEGER` column) when SQL should order by it, as
   `TaskPriority` does; clients still see the text.
   Structs serialize snake_case: no `#[serde(rename_all)]` on structs (enums
   keep `rename_all = "camelCase"`). JS getters and `toJSON` are camelCase.
   A `deleted: bool` column makes the derive also implement the `SoftDelete`
   marker, which unlocks `select_live`.
2. Append a migration to `MIGRATIONS` in `cohesion-db/src/migrations.rs`.
   Never edit or reorder a shipped entry. `Container::migrate` tracks progress
   in `PRAGMA user_version`.
3. Re-export it from `cohesion-core/src/model/mod.rs`.
4. Add it to the `every_model!` list in `cohesion-db/src/db_macros.rs`, whose
   test (`cohesion-db/src/migrations/tests.rs`) fails fast when the struct and
   the migrated table disagree.

### Query it

Builders live in `cohesion-db/src/query/` (one file per statement); every
command reaches them through `ctx.db`, the view of the connection its
`Access` picks: a `ReadContainer` for `Read`, a `WriteContainer` for `Write`.
Reads (`select`, `select_id`, `select_column`, `select_live`, and the
partial-row projections `select_as` and `select_live_as`) come from the
`ReadableDatabase` trait both views implement, so import it
(`use crate::ReadableDatabase;`) wherever you call them; the conditions
shared by every builder with a `WHERE` come from `Filtered`, imported the
same way. Only a `WriteContainer` adds `insert`, `update` and `delete`.
A command writes no `SELECT`, `INSERT`, `UPDATE` or `DELETE` of its own (the
full-text searches, over virtual tables that are no model, are the exception):

```rust
ctx.db.select::<Thing>().filter_eq(Thing::WORKSPACE_ID, ws).order("name").all()?;
ctx.db.select::<Thing>().filter_eq(Thing::ID, id).one()?;              // Option<Thing>
ctx.db.select_live::<Thing>().filter_eq(Thing::WORKSPACE_ID, ws).all()?; // deleted = FALSE
ctx.db.insert::<Thing>().set(Thing::NAME, name).set(Thing::WORKSPACE_ID, ws).returning_one().found()?;
ctx.db.update::<Thing>().set(Thing::NAME, name).set_now(Thing::MODIFIED)
      .filter_eq(Thing::ID, id).returning_one().found()?;
ctx.db.delete::<Thing>().filter_eq(Thing::ID, id).execute()?;
```

`#[derive(Model)]` gives each stored column a typed constant, `Thing::NAME`
a `Column<Thing, String>`, so `set(column, value)` and
`filter_eq(column, value)` take only a column of the builder's model and a
value of its type (a misspelled column, another model's column or a
wrong-typed value is a compile error). A column read through `coalesce` or
`expr` is typed `Option`, since what is stored may be `NULL`.
`filter_eq` with `None` matches `IS NULL`. `filter` takes any other SQL
condition (a joined table's column, `<`, `OR`, an expression) and `bind`
fills its `?` placeholders, as it does those of the most recent
`raw_set`. `raw_set` assigns a SQL expression instead of
a value, but prefer the typed ones: `set_null(column)` (nullable columns
only), `set_true(column)` and `set_false(column)` (no bind), `set_now(column)`, `set_max_incremented(column)`
and `set_max_incremented_within(column, scope_column, scope)` (the end of a sorted
list), `append(column, text)` and `append_line` on an `Update`. There is no
`raw_filter`: membership is typed, from `Filtered`.

- `filter_in(column, subquery)` and `filter_not_in`: the subquery is another
  select of one column, `ctx.db.select_column::<Other, i64>("thing_id")
  .filter("workspace_id = ?").bind(ws)`, with its own filters and binds.
- `filter_in_values(column, values)` and `filter_not_in_values`: a list;
  an empty `IN` list matches nothing, an empty `NOT IN` list everything.
- `filter_day(column, DayFilter)`.
- `order_bound(order, value)` on a `Select`: an `ORDER BY` with one `?`,
  bound after the filters' whatever order they were added in, for a sort key
  read through a subquery on an id from the wire (the table rows query).

These take a column by name and qualify it with the model's table.
`raw_set` and `set_group` debug-assert that the column is one of the
model's `columns()`.
`.one().found()` turns `Ok(None)` into `CommandError::NotFound` of the
model's (or the projection's model's) `Named::NOUN`. Type inference usually removes
the turbofish when the result is assigned to a typed binding.

`select_live::<M>()` (only for `M: SoftDelete`) starts with
`deleted = FALSE`; use it for every read a client sees, so soft-deleted rows
stay hidden. Plain `select` sees them. `update_live::<M>()` does the same for
an update, and `.live()` (from `Filtered`) adds the condition to any builder,
such as a projection's select, so a command never types `deleted = FALSE`.

### Store a field as JSON, or several columns as one enum

`#[model(json)]` on a field stores a serde type as JSON text. Write it with
`set("column", Json(&value))` (`cohesion_core::value::Json`); a stored value
that doesn't parse is a `RowError`, not a panic.

A group of nullable columns that together say one thing is one enum field:

```rust
#[derive(ColumnGroup)]
pub enum AccountHost {
    Extension { extension_id: i64, identifier: String },
    Orphaned,
}

#[model(group)]
pub host: AccountHost,   // in the #[derive(Model)] struct
```

A variant's fields are columns named after them. A row is the variant whose
non-`Option` columns are all set and whose other columns are all `NULL`; a
row fitting no variant, or two, is a `RowError`. Write with
`.set_group(AccountHost::Orphaned)` on an `insert` or
`update`: every column of the group is set, `NULL` for the other variants'.
The schema should still hold the same rule in a `CHECK`. The group's columns
are columns of the model for `filter`s, so none may share a name with the
model's other columns or another group's; the derive refuses to compile one
that does.

A group whose variants carry rows of another table, as a choice property
carries its options (`PropertyType`), implements `ColumnGroup` by hand and
names that SQL in `READS` (`(name, sql)`, selected `AS` the name its
`from_row` reads); the field is marked `#[model(group, reads)]`, which keeps
the model from being `SingleTable`, since `RETURNING *` can't read them, and
the derive refuses an unmarked field whose group reads any. What it reads is
never written by `set_group`: those rows have commands of their own.

Prefer a group to nullable columns read one by one. When a row is one of
several kinds, each with columns only it uses, the group makes the
combinations the `CHECK` refuses unrepresentable in Rust too: code matches
on the variant instead of testing which columns are set, and a new kind is
a new variant the compiler makes every `match` handle.

### Select part of a model, or join

When a read needs only some columns (a list that leaves out a long body), or
columns of another model, declare a projection in `cohesion-core` next to its
model and read it with `select_as`/`select_live_as`, which take the same
`filter`/`order`/`limit`:

```rust
#[derive(Clone, Serialize, Projection, TsWire)]
#[projection(of = Thing)]
#[projection(left_join = Owner, on = "owners.id = things.owner_id")]
pub struct ThingSummary {
    pub id: i64,
    pub name: String,
    #[projection(expr = "substr(body, 1, 200)")]
    pub excerpt: String,
    /// `owners.owner_name`, checked against `Owner`'s field of that name.
    #[projection(from = Owner)]
    pub owner_name: Option<String>,
    /// `COALESCE(things.size, owners.size)`.
    #[projection(or_from = Owner)]
    pub size: Option<i64>,
    /// `owners.id`, under another name.
    #[projection(from = Owner::id)]
    pub owner_id: Option<i64>,
    /// `COALESCE(things.label, owners.owner_name, 'Untitled')`.
    #[projection(coalesce(Thing::label, Owner::owner_name, "'Untitled'"))]
    pub label: String,
}

ctx.db.select_live_as::<ThingSummary>().filter("things.workspace_id = ?").bind(ws).all()?;
```

A plain field must be one of the model's fields with the same type, or the
derive doesn't compile; an `expr` field is read from `<expr> AS <field>`
unchecked (an aggregate such as `SUM(col)` is an `expr`, read without a
`GROUP BY` as one row).

Name a column as `Model::field` rather than in SQL text wherever an
attribute takes one: `from = Model::field`, an `expr`, and each term of a
`coalesce(...)` list (which writes the `COALESCE` for you; its other terms
are SQL strings, `coalesce("SUM(col)", "0")`). `#[derive(Model)]` gives
every stored column a hidden constant, so a misspelled field, a field that
isn't a column, or a model the projection doesn't join fails to compile.
`Model::field` is the column as stored, even when the model reads that
field through an expression. `join = Model` is an inner join,
`left_join = Model` a left one (several `#[projection(...)]` attributes
join several); its `on` may hold `?`, filled by `bind_join` before any
`filter`'s `bind`. The model's columns are qualified with its table, so a
joined table's `id` is never ambiguous, but a `filter` or `order` of yours
names a column both tables have with its table (`things.id`). A `#[model(group)]` field is a
projection's `#[projection(group)]` field too, and an `expr` may be a path
to a `&str` constant instead of a literal, so a filter can reuse the same
SQL.

### A model that joins

When every read of a model must include another table's data, declare the
join on the model itself, never as a second `Projection` of the whole row
(a model that must be read through `select_as` compiles when read with
`select` and fails at run time):

```rust
#[derive(Clone, Model)]
#[model(table = "things", backup = Included)]
#[model(left_join = Owner, on = "owners.id = things.owner_id")]
pub struct Thing {
    pub id: i64,
    /// Stored as `things.name`, which is `NULL` for an owned thing; read as
    /// the owner's name then. Writes set the column; reads take the expression.
    #[model(coalesce(Self::name, Owner::owner_name))]
    pub name: String,
    /// `COALESCE(things.size, owners.size)`, checked against `Owner`'s field.
    #[model(or_from = Owner)]
    pub size: Option<i64>,
}
```

`join` and `left_join` take the same `on` as a projection's (several join
several). `expr` (a literal, a path to a `&str` constant, or a column
`Model::field`), `coalesce(...)` (as on a projection, with `Self::field` for
the model's own column) and `or_from` change how a stored column is *read*; the column is still the model's, so
`set("name", ..)` writes it. Every read applies the joins and expressions:
`select`, `select_live`, `select_id`, `select_column`, and the row an
`insert`/`update` returns. `returning_one`/`returning_all` on those
re-read the written ids through the model's select, because `RETURNING`
cannot join. `Delete::returning_*` exists only for a `SingleTable` model
(the derive implements the marker when there are no joins or derived fields),
since a deleted row cannot be read again: a joined model's delete doesn't
compile with `returning_*`. A `filter` on a joined model names a column both
tables have with its table (`things.id`). Plain `Projection`s remain for
reading *part* of a row.

### Add a command

1. Add a plain request struct to the domain's
   `cohesion-core/src/protocol/<domain>_requests.rs` (re-exported from
   `protocol/mod.rs`), and one `Command` variant in
   `cohesion-core/src/protocol/command.rs`:
   ```rust
   /// Renames a thing.
   #[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
   pub struct RenameThingRequest { pub id: i64, pub name: String }

   // in `pub enum Command`:
   /// Renames a thing.
   #[reply(Thing)]
   RenameThing(RenameThingRequest),
   ```
   `#[derive(Commands)]` generates the `Call` impl the CLI uses, the
   `From<RenameThingRequest> for Command` conversion, and the `dispatch` arm.
   A request with no arguments is a unit struct on a unit variant that names
   it: `#[reply(Vec<Workspace>, request = ListWorkspacesRequest)] ListWorkspaces,`.
   If `Thing` is a new reply payload, add `Thing(Thing)` to `ReplyData`
   (`cohesion-core/src/protocol/reply_data.rs`);
   `#[derive(Replies)]` generates its `From`/`TryFrom` and `ReplyPayload`
   impls, and a `#[reply]` type with no variant is a compile error. Each payload
   type has exactly one variant, so a reply that is a bare primitive gets a
   newtype, as `ScriptResult(String)` does for `RunScript`.
   Request and reply types (and the models they carry) derive `TsWire`,
   which declares them as TypeScript for the web UI; mark a request
   `#[ts_wire(input)]` so clients may leave its `Option` fields out. Then run
   `npm run generate` in `web/` to rewrite `src/lib/protocol/generated.ts`;
   commit it with the change.
2. Write the command in `cohesion-db/src/commands/<domain>.rs`:
   ```rust
   #[derive(FromRequest)]
   #[request(RenameThingRequest)]
   pub struct RenameThingCommand { pub id: i64, pub name: String }
   impl DatabaseCommand for RenameThingCommand {
       type Response = Thing;
       type Access = Write;
       fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Thing>> {
           let thing = ctx.db.update().set(Thing::NAME, self.name)
               .filter_eq(Thing::ID, self.id).returning_one().found()?;
           Ok(thing.invalidating([Invalidation::Things]))
       }
   }
   ```
   Every command declares `type Access = Read;` or `type Access = Write;`, and
   `ctx.db` is the view that access picks. A `Read` command's `handle` takes
   `ctx: &mut CommandContext<'_, Read>`, whose `db` is a `ReadContainer` with
   only the `ReadableDatabase` reads, so the compiler rejects a read that
   writes. A `Write` command takes `ctx: &mut CommandContext<'_>` (`Write` is
   the default), whose `db` is a `WriteContainer`. A write command that
   calls a read command inline passes `ctx.as_read()`:
   `GetThingCommand { .. }.handle(&mut ctx.as_read())`.
   A `Write` command returns its response as `Changed<T>`, naming the
   invalidation keys it changed, so no write can commit without saying what
   clients must refetch: `thing.invalidating([key])` (an `Option` or a `Vec`
   works too), or `thing.invalidating_nothing()` for a write no client
   shows. `Changed`'s `and`, `merge`, `map` and `and_then` combine a
   helper's keys with the command's. A write that runs another write inline
   calls `ctx.run(OtherCommand { .. })?`, which returns the plain response
   and keeps its keys.
   `FromRequest` destructures the request exhaustively, so a request field
   the command doesn't name is a compile error. Drop one on purpose with
   `#[request(RenameThingRequest, ignore(field_a, field_b))]`. It also
   implements `DbRequest` for the request, which is what ties the request to
   this command.
   `CommandContext` gives you `db`, `secrets`, `now`,
   `events` (raised once the command commits) and `trace` (the chain of
   events that led to this command), whichever its access.
   Commands that are only used internally (from JS, intelligence, or other
   commands) skip the derive and the `Command` variant entirely.

   A command scoped to a workspace calls `ensure_workspace(ctx.db, id)?`
   (`commands/workspace_scope.rs`, which takes any `impl ReadableDatabase` by value so both
   kinds of command can call it) first, so a missing or deleted workspace is
   `NotFound`. Constraint failures map themselves when converted to
   `CommandError`: foreign key -> `InvalidReference`; NOT NULL, UNIQUE/PRIMARY KEY and
   CHECK -> `Validation` with a generic message. Anything else stays
   `CommandError::Database`, which the kit logs and reports as internal.
3. There is no handler table. Kit's single blanket
   `impl<R: DbRequest> Route<Database, R> for CohesionContext` runs every
   database request, so steps 1 and 2 are the whole wiring. A variant whose
   request has no `FromRequest` command fails to compile in `dispatch` with
   "has no database command".

   A command that isn't a database call names its kind with `via`, as
   `RunScript` does with `#[reply(ScriptResult, via = Script)]`, and gets a
   hand-written impl:
   ```rust
   impl Route<Script, RunScriptRequest> for CohesionContext {
       async fn route(&self, request: RunScriptRequest) -> Result<ScriptResult, ServerError> { ... }
   }
   ```
   The impl goes in its domain's file under `cohesionkit/src/routes/`
   (`extension_routes.rs`, `agent_routes.rs`, ...; `routes/mod.rs` lists
   them), whatever its kind. The kind only keeps the blanket database impl
   from overlapping the hand-written ones. The kinds are unit structs in
   `cohesion-core/src/protocol/route_kinds.rs`; a new kind is another one
   there.
4. From the CLI: every command is reachable raw, with no CLI change:
   `cohesion api '{"type":"renameThing","args":{"id":1,"name":"x"}}'`. A
   friendly command is optional; see [Add a CLI command](#add-a-cli-command).

### Add a CLI command

The CLI is a terminal client for small jobs, minimal by design: the web UI
is the full surface, and `cohesion api` reaches every daemon command as
JSON. A command gets a friendly form only if a person would run it from a
terminal (add a task, read the journal, chat with an agent), or a new
install needs it before the web UI can be reached (pairing, endpoints).
Settings a person changes once, and anything a script would drive, stay
on `cohesion api`.

Conventions every command follows:

- **Shape**: `cohesion <noun> [verb] [args]`. A noun is a `Noun<…>`
  (`cli/noun.rs`) whose bare form takes its default action, usually
  listing. `-w <id|title>` overrides the selected workspace for one
  command; a workspace-scoped handler gets the id from `ctx.workspace()`.
- **Names**: a thing the user names is accepted by id or by name, matched
  case-insensitively, through `find` in `name_lookup.rs` (implement `Named`
  for a new kind). An unknown name is `UnknownName`; one several things
  share is `AmbiguousName`, which lists their ids to pass instead.
- **Output**: friendly commands print text for people on stdout: lists
  through `table::list` with `row!` (`cli_macros.rs`), which aligns columns
  and prints the empty-state hint, and a progress spinner through
  `terminal_io::spin`, on stderr and only on a terminal. JSON is
  `cohesion api`'s job, not theirs.
- **Confirmation**: a delete asks first through `terminal_io::confirmed`
  ("Delete Ada? [y/N]" on stderr) and takes `-y`/`--yes` to skip it.
- **Errors and exit codes**: a handler returns `CliError`; `main` prints
  it to stderr and exits 1. Everything else exits 0, including a declined
  confirmation ("Cancelled."). `cohesion api` prints the daemon's reply,
  an error reply included, as JSON on stdout and exits 0; it exits 1 only
  when the request can't be parsed or sent. Add a `CliError` variant for a
  new local failure; a daemon error passes through as `Server`.
- **Help**: each subcommand's and argument's `///` comment is its `--help`
  text, a sentence for a person.

Wiring a new noun:

1. A module under `cohesion/src/cli/`, declared in `cli/mod.rs`, with a
   `#[derive(Subcommand)]` enum of its verbs next to the handlers they
   dispatch to. A handler calls `ctx.call(RenameThingRequest { .. }).await?`
   and gets the typed reply.
2. One variant in `CliCommand` and one dispatch arm in
   `cli/cli_command.rs`.
3. A line under "Common commands" in `docs/cli.md` (or the noun's own section
   there) showing the common use, in the same commit.
4. `cargo test -p cohesion` runs `command_tree_is_consistent`
   (`args/tests.rs`), which catches clashing flags and names. Handlers
   have no tests of their own; try the command against a scratch daemon
   ("Smoke test" above).

A verb on an existing noun is step 1 and 3 only. The CLI depends only on
`cohesion-core`, never on the database, script or intelligence crates.

### Expose an entity to scripts

1. In `cohesion-js/src/entities/<name>.rs` (declared in `entities/mod.rs`):
   ```rust
   use cohesion_macros::js_entity;

   #[js_entity(Thing)]
   impl JsThing {
       /// Methods are ordinary Rust; `self.inner` is the model, `self.db` the guarded database.
       pub fn rename<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
           self.inner = self.write(&ctx, RenameThingCommand { id: self.inner.id, name })?;
           Ok(())
       }
   }
   ```
   `JsThing` isn't declared anywhere else: the attribute generates the struct
   (`inner: Thing`, `db`), `new`/`inner`/`read`/`write`, and one `#[rquickjs::methods]`
   block holding your methods plus an enumerable camelCase getter per column
   and `toJSON`. An entity with no methods is `#[js_entity(Thing)] impl JsThing {}`.
   The JS class is named after the model; override with `#[js_entity(Thing, js = "Name")]`.
   `#[js_entity(Thing, metadata)]` adds `metadata(key)` and
   `setMetadata(key, value)`, string pairs stored in the `metadata` table.
   Only models with a `HasMetadata` impl qualify: a new one is a column in
   `metadata` (a foreign key that cascades, in a new migration), a
   `MetadataOwner` variant and a line in `has_metadata!`. To find rows by metadata, call
   `.with_metadata(pairs)` (or `.with_metadata_keys(keys)` for any value) on
   the model's `select`/`select_live` query.
   `db` is a `GuardedDatabaseContext`. `self.read(&ctx, cmd)` accepts only
   `Access = Read` commands and always works. `self.write(&ctx, cmd)` accepts
   only `Access = Write` commands; in a read-only sandbox it throws a JS
   `Error` ("This script has read-only access to Cohesion; the
   `cohesion:write` permission is required to change data.") instead of
   running the command. Pick the gate by the command's `Access`; the compiler
   rejects a wrong pick.
   `Thing` must `#[derive(Fields)]` in cohesion-core, which hands its fields to
   the attribute; `Model` doesn't imply it, so add it to a model's derives
   when exposing it. A plain struct with no table works the same way
   (`ModelUsage` is one). Hide a field from scripts with `#[fields(skip)]`,
   and name its getter with `#[fields(rename = "name")]` where the column's
   camelCase reads badly (`IntelligenceAgent`'s `agent_name` is `name`).
2. Add `thing::JsThing` to `entities` in one module of the `script_api!`
   list in `cohesion-js/src/script_api.rs`, which exposes a global `Thing`
   for `instanceof` and adds its TypeScript declaration to that module and
   `TS_DECLARATIONS`. Pick `CORE` for `cohesion`'s everyday entities and
   anything two modules share, otherwise the module of the global that
   returns it (`ARTICLES`, `CONNECTORS`, ...). Each type is listed once, so
   it can't be mounted but undocumented, or documented twice.
3. Return it from somewhere scripts can reach: a method on `CohesionGlobal`
   (`globals/cohesion_global.rs`) or on another entity, via `JsThing::new(thing, self.db.clone())`.
   To hand plain data to a script function (a connector's credential, say),
   pass `ScriptArgument::Json(value)`; the function receives it as an
   ordinary object. Structs a script returns or receives derive `TsInterface`
   (its fields become the `.d.ts`), with `#[serde(rename = "camelName")]` on
   fields that need camelCase keys; the macro honours `rename` and `alias`,
   and declares an `Option` field optional. An enum of such structs, tagged
   with `#[serde(tag = "kind", rename_all = "camelCase")]`, derives it too
   and is declared as a union (`Block` in `model/cohesion_ui.rs`).
4. Every column type needs `ToJs` (`conversion/to_js.rs`). `i64`, `f64`, `bool`,
   `String` and `Option<T>` have it; add a `TextEnum` column's type to
   `enums` in `script_api!`, and also to `mounted` if scripts should name its
   cases as a global (`TaskStatus.completed`). A missing impl is a compile
   error naming the type.
5. Check the TypeScript. `#[js_entity]` declares the class from the written
   Rust types (`Vec<JsTask>` is `Task[]`, `Option<T>` is `T | null`) and the
   `///` comments. Where the Rust type says too little, override it:
   `#[ts(type = "number | string")]` on an argument, `#[ts(returns = "Task | Document")]`
   on a method, or `#[ts(skip)]` to leave a method out. Methods on
   `CohesionGlobal` get the same from `#[js_global("cohesion")]`.
   A method taking an options object takes a `#[derive(JsOptions)]` struct
   instead of an `Object` (see `TaskListOptions` in `entities/workspace_options.rs`):
   it parses each camelCased property and declares an `interface` with the
   fields' `///` comments. `Option` fields may be left out; wrap the argument
   in `Opt<...>` to make the whole object optional; `#[js_options(rename = "baseURL")]`
   overrides a key. An argument with several shapes is a `#[js_options(tag = "kind")]`
   enum of such structs, declared as a union TypeScript narrows on `kind`
   (`EndpointOptions` in `globals/intelligence_endpoint_options.rs`). List every options type,
   an enum's structs too, under `options` in `script_api!`. Field types need
   `FromJsField`; `enums` get it.
   `cohesion_js::script_api::TS_DECLARATIONS` is the whole `.d.ts` file.
6. Which modules an agent sees follows from what its role mounts
   (`AgentRole::mounts` in `cohesion-js/src/engine/agent_role.rs`, each
   mount's modules in `Mount::modules`), so an agent is never told about a
   global it lacks. A new agent capability is a mount added to the roles
   that should have it, there and nowhere else. A
   module's `load` says whether the code tool declares it (`upfront`: core,
   the role's agent global, `services`) or indexes it for the
   `documentation` tool (`on_demand`), so the `///` comments are prompt text
   too. An `upfront` module is declared in brief, with only the first
   paragraph of each comment (`TsDocumentation::TS_BRIEF`), and the
   `documentation` tool returns it in full; an `on_demand` module comes
   brief too unless the call asks for `full`. So open each comment with one
   sentence saying what the item is, keep anything a script gets wrong
   without it in that paragraph, and put other caveats and examples in later
   paragraphs. Every `upfront` type is in every request's prompt: a type few
   scripts use belongs in an `on_demand` module. A new global's module goes in `Mount::modules` for its mount.

### Clean up an article's text

Coherence, Cohesion's own parser, reads a page or a feed's content in
three steps: its rules are decided over the
document, its finder picks out the article, skipping what the rules left
out, and its writer makes that Markdown, carrying out the rules as it
walks. What of a page or a feed item isn't the article (share widgets,
pitches, bylines, permalinks) is one entry in a JSON file in
`cohesion-internet/coherence/rules/`. The files are read in name order,
which their number prefix sets, and when two rules pick the same element
the first decides it. An entry is a `trigger` that picks elements (by
selector, `id`, class, `class`/`id` name, heading, link text, image
address…) and the `action` taken on them (`delete`, `unwrap`,
`delete_section`, `link_to`, …), with an `about` saying what it is for:

```json
{
  "about": "A share widget, `share` at any boundary of its name.",
  "trigger": { "named": { "part": ["share", "sharedaddy"] } },
  "action": "delete"
}
```

`build.rs` turns the files into `static` Rust at build time, so a typo is
a build error naming the file and the binary carries no JSON. There are
no size limits: a rule matching by name must be precise enough never to
match the article's own container (names that state a page's state, such
as `has-sidebar`, and utility classes are already ignored).
`coherence/hints.json` weighs names for and against holding the article.
A case no trigger can say is a new `Trigger` variant in
`src/coherence/rule.rs` and in `build.rs`, never a pass of its own. See
what a change does on the real page or feed first:

```sh
cargo run -q -p cohesion-internet-harness -- feed <url> > before.md
# change a rule
cargo run -q -p cohesion-internet-harness -- feed <url> > after.md
diff before.md after.md
```

It prints the Markdown the daemon would store, each feed item's from the
feed or its page; `page <url>` reads a single page as an article.
Add a test in `cohesion-internet` with the markup the rule is for, cut
down to what it needs.

### Log something

```rust
use cohesion_core::log::Module;
cohesion_core::log_info!(Module::Database, "migrated to version {}", v);
```

A new subsystem gets a `Module` variant in `cohesion-core/src/log/module.rs`.
