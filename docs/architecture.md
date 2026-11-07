# Architecture

Cohesion is a daemon (`cohesiond`) that owns a SQLite database, a JavaScript
runtime and connections to LLM endpoints, plus a CLI (`cohesion`) that talks to
it over mutual-TLS HTTP, WebSocket, or a local Unix socket. The code is split
into small crates with a one-way dependency graph so that every layer can be
built and tested alone.

```
cohesion (CLI) ────────────────────────────────────────────────────────────────────────────┐
                                                                                           ▼
cohesiond ──► cohesionkit ──► cohesion-intelligence ──► cohesion-js ──► cohesion-db ──► cohesion-core ◄── cohesion-macros
                                                            │                             ▲
                                                            └──► cohesion-internet ───────┘
```

Arrows point at dependencies; a crate may also depend on anything further
right (cohesionkit uses db and js directly, and every crate uses core).

| Crate | What lives there | Depends on |
|---|---|---|
| `cohesion-core` | Models, protocol (`Command`, `ReplyData`, `Route`), the `TsWire` declarations the web UI's `generated.ts` is written from, `Value`/`Row`/`Model`/`Projection`/`ColumnGroup` traits, `Caller`, invalidations, logging, auth, paths | cohesion-macros, serde, serde_json, rustls, aws-lc-rs |
| `cohesion-macros` | `#[derive(Model)]`, `#[derive(Projection)]`, `#[derive(ColumnGroup)]`, `#[derive(FromRequest)]`, `#[derive(TextEnum)]`, `#[derive(Commands)]`, `#[derive(Replies)]`, `#[derive(JsOptions)]`, `#[js_entity]`, `#[js_global]` | syn, quote |
| `cohesion-db` | `Container` and its `ReadContainer`/`WriteContainer` views, query builders, migrations, `DatabaseContext`, `GuardedDatabaseContext`, `CredentialManager`, commands | core, rusqlite, tokio |
| `cohesion-internet` | Reading the web: fetching an article's page as Markdown (`article_snapshot`) and reading RSS, Atom and JSON feeds (`feed`), both through one `pipeline` that reads HTML with Coherence, Cohesion's own parser (`coherence`, with its rules) | core, reqwest, dom_query, quick-xml |
| `cohesion-internet-harness` | A tool, not linked by anything: prints what cohesion-internet makes of a page's or feed's URL, for building Coherence's rules | core, internet |
| `cohesion-js` | `ScriptEngine`, the `cohesion` host namespace, JS entities, TypeScript declarations | core, db, internet, rquickjs, reqwest |
| `cohesion-intelligence` | `IntelligenceBackend` and its three wire formats, `ModelCatalog`, `Session`, `AgentCoordinator`, the `AccountCredentials` trait account-backed endpoints sign in through | core, db, js, reqwest |
| `cohesionkit` | Facade: `CohesionContext`, the `CapabilityClients` registry, connectors, and the `Route` impls that run protocol commands | all of the above |
| `cohesiond` | HTTP/WS/socket transports, sessions (those of clients offering actors among them), TLS | cohesionkit, axum |
| `cohesion` | CLI, and the `cohesion open` loopback proxy that serves the web UI (`web/`), fetched from cohesiond, over the CLI's connection | core, reqwest, tokio-tungstenite, axum, flate2 |

No crate uses cargo features. The CLI never links rusqlite or rquickjs.

Each crate's `lib.rs` opens with a `//!` summary; `cargo doc --workspace
--no-deps --open` renders the per-item documentation.

## Where to look

| For | Look in |
|---|---|
| A model, request, reply, invalidation, hook kind | `cohesion-core/src/{model,protocol}/`, `invalidation.rs`, `model/hook_kind.rs` |
| The words clients show for kinds | `cohesion-core/src/protocol/vocabulary.rs` |
| A database command, its tests | `cohesion-db/src/commands/<domain>.rs`, `<domain>/tests.rs` |
| The schema | `cohesion-db/src/migrations.rs` (append only), `every_model!` in `db_macros.rs` |
| Transactions, events, invalidation publishing | `cohesion-db/src/database_context.rs`, `database_event.rs` |
| The script API | `cohesion-js/src/{globals,entities,script_api}/`, sandboxes in `sandbox/`, the engine in `engine/` |
| Models and providers | `cohesion-intelligence/src/{providers,catalog,session}/`, `intelligence_backend.rs` |
| Agents, cues, runs | `cohesion-intelligence/src/agents/` (`agent_coordinator.rs`, `agent_host.rs`, `agent_fork.rs`, `agent_messenger.rs`, `conversation_session.rs`, `conversation_mark.rs`, `relevant_set.rs`, `applied_effects.rs`); what each role may reach: `cohesion-js/src/engine/agent_role.rs` |
| Non-database routes | `cohesionkit/src/routes/<domain>_routes.rs` |
| Hooks and timed events | `cohesionkit/src/hooks/` (`hook_manager.rs`, `hook_receiver.rs`, `day_clock.rs`) |
| Extensions, connectors, capability clients, actors | `cohesionkit/src/{extension_manager,connectors,capability_clients}/`, `actor_sessions.rs`, `script_actors.rs` |
| Transports, TLS, sessions, pairing | `cohesiond/src/server/` |
| CLI nouns, `cohesion open` | `cohesion/src/cli/`, `cohesion/src/web_proxy/` |
| The web UI | `web/src/lib/{protocol,stores,features,ui,shell}/`, `web/src/routes/` (`web/README.md`) |

`docs/actors.md` is the actor client protocol, `docs/extensions.md` the
extension and script API, `docs/serving-the-webui.md` how a client fetches
and serves the web UI, `docs/cli.md` the command line and
`docs/self-hosting.md` installing and running the daemon, its flags and
`config.json` among them. `docs/tour/` is a longer, illustrated walk through
the product (A), the daemon (B), clients (C), events (D), agents (E),
inference (F), capabilities (G), extensions (H), content (I), trust and
operations (J) and worked scenarios (K).

## Request flow

1. A transport (`cohesiond/src/server/{http_routes,ws_routes,serve}.rs`; the
   socket itself is `unix_socket.rs`) first establishes which client is
   connecting, then builds a `Caller` (transport, client, remote address),
   which it names in its own log lines. Over TLS `PeerAcceptor` looks the
   certificate up; on the Unix socket `local_acceptor` runs the socket's
   handshake (see Clients and identity). A session is handed that client
   and never sees either step.
   The caller does not travel further than the transport; the client does,
   as below.
2. `Session` (`session.rs`) reads each frame as a `ClientFrame`
   (`ReceivableMessage::frame` in `transport.rs`) and queues a `Request` for
   its handler; a one-off HTTP command is read as a `Request` alone
   (`handle_http`). Either way `handle_request` (`request_handler.rs`)
   calls `Command::dispatch` on a `ClientRequest { context, client }`
   (`cohesionkit/src/client_request.rs`), the context and the client that
   sent the request. The frames of a client offering capabilities are the
   session's own (see Capabilities).
3. `dispatch` is generated by `#[derive(Commands)]` on `Command` and routes
   each variant to `Route<Kind, Request>`, where `Kind` is the variant's
   `via` (default `Database`). `ClientRequest` hands every kind but
   `FromClient` to the context, as if no client had asked. The kinds live in
   `cohesion-core/src/protocol/route_kinds.rs`:
   - `FromClient`: hand-written impls on `ClientRequest`, for the few
     requests that treat their sender differently from any other client:
     `BeginPairing` (refused to a client that isn't local when the
     security settings keep pairing to clients on this computer, checked on
     every call), `GetPairingAccess` (whether
     `BeginPairing` would refuse, and why, from the same function, so the
     web UI disables Add client… without deciding itself), `ListClients`
     (which also marks the client asking as `current`) and
     `RevokeOtherClients`, all in `client_routes.rs`.
   - `Database`: one blanket `Route<Database, R>` for every `DbRequest`, in
     `cohesionkit/src/routes/database_routes.rs`.
   - `Script`, `Intelligence` and `Local`: hand-written impls, filed by
     domain under `cohesionkit/src/routes/` rather than by kind (the kind
     only keeps them from overlapping the blanket impl):
     `script_routes.rs` (`RunScript`, `Documentation`), `catalog_routes.rs`
     (`UpdateModelCatalog`), `agent_routes.rs` (agent requests that touch a
     running agent), `actor_routes.rs` (`StartActorSession`, `SendActorSessionMessage`,
     `StopActorSession`, `ResolveActorSession`, `SetActorSessionMode`, which call the client offering
     the actor, and `ListActorUsage`), `client_routes.rs` (`About`, `WebBundle` (the web UI; see
     `docs/serving-the-webui.md`), `DeleteClient`), `extension_routes.rs` (`RegisterExtension`,
     `SetExtensionPermission`, `SetExtensionHook`, `UninstallExtension`,
     `SearchExtensionRegistries`, `ListExtensionUpdates`,
     `CheckExtensionUpdates`, `PreviewExtension`, `PreviewExtensionScript`,
     `CallExtension`), `connector_routes.rs` (the connector requests:
     `ListConnectorServices`, `CompleteConnectorAuthentication`,
     `ImportResource`, `RefreshResources` and the rest), and
     `article_routes.rs` (`ResnapshotArticle`),
     which fetches an article's page on a blocking thread with
     `cohesion_internet::article_snapshot::snapshot` before one database command
     stores it, and `feed_routes.rs` (`AddFeed`,
     `RefreshFeed`, `RefreshFeeds`), which read a feed with
     `cohesion_internet::feed::read_feed` the same way. `cohesionkit/src/feeds/`
     also reads every followed feed hourly from daemon start. Feeds are shared
     by every workspace, and an article, which always comes from a feed, belongs
     to none. A feed's new items raise the global `FeedArticles` and `Feeds`
     invalidations.
4. A `Database` request is converted into its `DbRequest::Command` and sent to
   `DatabaseContext`, which runs it on the single database thread inside a
   savepoint, publishes its invalidations on success, and rolls back on error.
   A `Write` command's `handle` returns `Changed<T>`, its response with the
   keys it changed (`Access::Output`), so the keys a write publishes are in
   its return value or in the commands it ran with `ctx.run`; nothing else
   can add one.
   Each command declares `type Access = Read` or `Write`; `Access` picks the
   view of the connection the command's context holds, so a `Read` command's
   `ctx.db` is a `ReadContainer` with only the `ReadableDatabase` reads and
   cannot write. A write also names the area it changes, `Write<A>`:
   `Write` alone is `Write<UserData>`, the intelligence settings
   commands (endpoints, their models, tier defaults, spend limits) are
   `Write<IntelligenceSettings>`, an agent's memories and cues
   (`CreateMemoryCommand`, `DeleteMemoryCommand`, `SetAgentCueCommand`,
   `DeleteAgentCueCommand`) are `Write<AgentSettings>`, which no extension
   permission grants, and `SendNotificationCommand` is
   `Write<Notifications>`: it runs no SQL, so it takes no lock, and only
   raises `notification:send` and an `Invalidation::Notification` carrying
   the title and message for clients to show. A command may `ctx.run` a
   write of any area.
5. The reply is converted `Into<ReplyData>` and framed back to the client.

A session sends a client nothing unasked but its `Hello`: only once the
client sends `ClientFrame::Subscribe { id }` (answered `accepted`) does it
also push every `Invalidate` batch published from then on, and a `Reset`
when it fell behind. The web UI and the iOS app subscribe first thing on
each connection, before they load anything, so no change after a load
goes unheard; the CLI subscribes only while following an agent's turn, and a
client that only offers never does.

`Hello` also carries the `Vocabulary` (`cohesion-core/src/protocol/vocabulary.rs`):
what each permission, hook, task priority, task size, agent setting, model tier,
provider, network access, pairing opener and security event is called, each list in the order clients show it. Clients show
these words and keep none of their own (only icons and colours stay on the
client); a new variant fails to compile until it is worded there.

## Transports and the wire

cohesiond listens two ways (`cohesiond/src/server/serve.rs`):

- **HTTPS with client certificates** on `--port` (3000 by default):
  `POST /register` pairs a new client with a setup key; `POST /command`
  runs one request; `GET /connect` upgrades to a WebSocket session. The
  last two need a registered certificate (`require_registered`,
  `PeerAcceptor`, `client_cert_verifier.rs`). `first_byte.rs` bounds how
  long a connection may sit before sending anything.
- **A Unix socket** for clients on the same machine, which open with a
  `LocalIdentity` line instead of a certificate
  (`unix_socket.rs`, `local_acceptor/`).

A session is the same on either (`transport.rs`'s `Exchange`). Every frame
is one line of JSON. The client sends `ClientFrame`s: a request
`{ "id", "type", "args" }` (`type` camelCase, `args` snake_case),
`subscribe`, and, for a client offering capabilities, its offers and
call results. The daemon sends `Frame`s (`protocol/envelope.rs`), each
numbered with a `seq` and tagged with a `type`: `hello` first (who the
client is, `PROTOCOL_VERSION`, the `Vocabulary`, the web bundle's hash),
then `reply`, `invalidate` and `reset` once subscribed, the calls of a
capability client, and `evicted`. `docs/actors.md` walks through a
session frame by frame.

`cohesion open` (`cohesion/src/web_proxy/`) is the third way in: a
loopback HTTP server, guarded by a launch token, that serves the web UI
and relays the page's `/connect` and `/client` over the CLI's own
connection. A local process gets nothing from it without the token.

### Behind a pass-through router

A router that forwards TLS bytes without holding keys can't add
`X-Forwarded-For`, so it can only say who the client is ahead of those bytes,
in a PROXY protocol v2 header. `--proxy-protocol <ADDRESS>` names such a router
(`proxy_protocol.rs`). `PeerAcceptor` asks `TrustedProxies` for the client's
address before the TLS handshake, and everything after it, the client list,
refused pairings and the logs, uses that address (`Peer`, `Caller`). The
header is read to its exact length from the `TcpStream`, so the stream rustls
gets starts at the ClientHello and needs no wrapper. With the flag off, or from
a peer it doesn't name, nothing is read and the peer's own address is used.

- **From a named peer** the header is required: TCP over IPv4 or IPv6, at most
  216 bytes after its 16-byte prefix, whole within 5 seconds. Anything else
  closes the connection with a warning and records nothing, including a TLS
  handshake with no header, so a router that lost its flag fails rather than
  showing its own address as every client's.
- **From any other peer** a header is not looked for. It reaches rustls as bytes
  that aren't a ClientHello, and the connection closes with a debug line. That
  peer can still speak TLS and is then served under its own address, as it
  would be with the flag off. The flag decides whose claim is believed, not who
  may connect.

**How a deployment says which peer is the Router:** put
`--proxy-protocol <address>` in each daemon's container arguments and start the
Router with `-proxy-protocol`, moving both at once, since each side refuses a
connection from the other without them. The address is the Router as the daemon
sees it from inside the container, which is not always the address the Router
has: rootless Podman's pasta shows a peer on the same machine as `169.254.1.2`
(measured on Podman 6.1.3), and a Router on another machine should arrive as its
own private address (not measured). To check, run the daemon with `--log-level
debug` and a wrong or missing `--proxy-protocol`: each failed handshake names the
peer it came from. Repeat the flag for several Routers. A daemon's container has
to be recreated to change it.

Believing an address is worth exactly as much as the network underneath it,
because the header proves nothing. Anyone who can complete a TCP connection from
a named address can claim any address, and forging a source without seeing the
replies can't complete one, so that takes a position on the path or on the same
machine. With the Router on the daemon's machine, every process there shares
`169.254.1.2`, other daemons' containers included. What such a peer can do: write false
addresses into Settings › Security, set the last address of a client whose key it
holds, hide its own address, and, by using many addresses, reach the 4096 that
`Security` remembers, after which refused pairings from new addresses go
unrecorded for up to an hour. What it can't do: pair without a setup key, act as
a client without its key, or read anything, since TLS still runs from the client
to the daemon. That holds only while an address admits nothing, so nothing may
ever be allowed or refused by `Peer::ip`; "on this computer" comes from the kind
of key a client holds, not from where it connects.

### Generated types

Every type a client sees derives `TsWire` (`cohesion-core/src/ts_wire.rs`,
with `///` comments carried over by `ts_documentation.rs`). Two examples in
cohesion-core print them: `ts_client` writes every model, request and
reply, the `Requests`/`Replies` maps and `PROTOCOL_VERSION`, and
`vocabulary` the `Vocabulary` as JSON. `npm run generate` in `web/` runs both
into `web/src/lib/protocol/generated.ts` and `vocabulary.json`; regenerate
and commit both whenever a `TsWire` type changes. cohesion-ts copies the same `generated.ts`.

## Scripts

`ScriptEngine` runs each script on a dedicated thread in a fresh QuickJS
runtime, so scripts cannot leak prototype changes into each other. The
`cohesion` global (`cohesion-js/src/globals/cohesion_global.rs`) and the entity
classes wrap models and send database commands through a
`GuardedDatabaseContext`: a read gate that accepts only `Read` commands and
always works, and a write gate that accepts only `Write` commands and throws a
JS `Error` naming the missing permission when the guard may not write the
command's `WriteArea`. `cohesion run` and the LLM code tool may write every
area.
Fetching an article's page (`article.resnapshot`) takes an
`ArticleFetchAccess` (`cohesion-internet/src/article_snapshot.rs`), which a script
gets only in a sandbox that mounts `Mount::ArticleFetch` (`ArticleFetchMount`): the general-purpose pool
(`cohesion run`) and agents' turns the user started. No extension permission maps to it, so extensions can read and
search articles but not fetch them; the daemon's `ResnapshotArticle` route and
feed reads make theirs with `ArticleFetchAccess::granted`.
`cohesion.sleep` likewise pauses only in a sandbox that mounts `Mount::Sleep`
(`SleepMount`): extensions (on their own thread), `cohesion run` and the
agents' turns (which the user can stop). The cues pool never
mounts it, since every agent's cues share its workers. The sleep blocks the
script's thread in short slices, ending at once when the job's `ScriptStop`
fires or, in an extension, when `shutdown_scope` drops the scope (its
`ScopeClosing`), and throws rather than outlast the job's deadline.
Agent sandboxes also mount `connectors`, which calls connector extensions'
import functions through `ConnectorAccess`, a blocking trait cohesion-js
declares (`cohesion-js/src/connector_access.rs`) and cohesionkit implements
(`connectors/agent_connectors.rs`). The engine the agents get is
`with_connectors`, and each agent job carries the access as context userdata;
cues don't, so a cue can't reach a connector. Cues do reach installed
services, read-only.
Extension scopes and agent sandboxes also mount `storage`
(`cohesion-js/src/globals/storage_global.rs`), a private key/value store in
`storage_entries` keyed by `SandboxKey` (`cohesion-db/src/sandbox_key.rs`):
`extension:<script_identifier>` or `agent:<agent_id>`, which also keys the
engine's persistent scopes. The kind prefix keeps an extension identified
`agent:1` from reaching agent 1. An extension's key comes from its guard's
identifier; each agent job, cues included, carries its agent's as context
userdata (`Sandbox::storing`); scripts never name one. Its writes skip the write gate
(`GuardedDatabaseContext::set_storage_blocking`), so read-only extensions,
cues and forks a cue started keep their state too.
Scripts may be TypeScript: `Sandbox::evaluate` first erases annotations with
`swc_ts_fast_strip` (`cohesion-js/src/sandbox/strip_types.rs`), blanking them to
whitespace so positions are unchanged. Syntax with runtime semantics
(`enum`, value `namespace`s, parameter properties) is rejected, not compiled.

`#[js_entity(Thing)]` generates a class's struct, getters and `toJSON` from the
struct's fields; `Thing` may be a table-backed `Model` or a plain value such
as `ModelUsage`. A macro cannot see another item's fields, so
`#[derive(Fields)]` emits a hidden `cohesion_core::__cohesion_fields_Thing!`
that calls back a given macro with the field list; `#[js_entity]` calls it
with `cohesion_macros::__js_entity_expand`, which then has both the methods and
the fields. The detour also keeps everything in one `#[rquickjs::methods]`
block, which rquickjs requires.

The same macros emit TypeScript declarations. The `script_api!` list in
`cohesion-js/src/script_api.rs` joins them into `TS_DECLARATIONS`, which
`cohesion script new` writes as `cohesion.d.ts`. The list is grouped by
module, so each type is in exactly one of `MODULES`. A pool's modules follow
from its mounts (`Mount::modules`): an agent's code tool declares the ones
that load `upfront` (`core`, its own `agent`, and
`services`) in brief, keeping only each comment's first paragraph
(`TS_BRIEF`), indexes the `on_demand` ones its pool mounts (`agents`,
`articles`, `resources`, `contacts`, `rewind`, `vocabulary`, `cues`, `actors`,
`network`, `connectors`, `storage` and others) and the
installed services, and ends with the user's connected services, naming each
kind of resource (a connector's `kinds` say what each holds); its
`documentation` tool returns an `on_demand` module in brief too, with every
comment in full when the call sets `full` (a load is written to the thread's
prompt cache, and reloaded after a rebuild), an `upfront` one in full, and the
services' declarations as `services`. A new script type goes in one module. The static `services` global declares
`use<K extends keyof ServiceMap>(identifier: K): ServiceMap[K]` and an empty
`ServiceMap`; what fills it depends on what is installed, so
`service_declarations` (`cohesion-js/src/service_declarations.rs`) renders,
at runtime from `ListServicesCommand`, one `interface ServiceMap` merge keyed
by each service's script identifier. Every function is declared in every
context; those not declared `readOnly` say "Not available in read-only
contexts." The `Documentation` command appends the merge to
`TS_DECLARATIONS`, and agents, foreground and background alike, get it
through `InstalledServices`
(`cohesion-intelligence/src/agents/installed_services.rs`).

## Extensions

An extension is a script with a `manifest()` and hook functions
(`docs/extensions.md` is the author's guide). `CohesionContext.extensions` is
an `ExtensionManager` (`cohesionkit/src/extension_manager.rs`), a cheap
clone and the only path to an extension's scoped worker. That worker is a
persistent sandbox, spawned on first use with the mounts its permissions map
to (`Mount::for_permission` in `cohesion-js/src/globals/mount.rs`).
`cohesion:read` and `cohesion:write` both mount the `cohesion` global. The
worker's guard writes the areas its permissions map to
(`WriteArea::for_permission`): `cohesion:write` the user's data, and
`intelligence`, which also mounts the `intelligence` global, the
intelligence settings. Hook arguments (the `Task` a hook receives) follow
the guard. Callers name an extension by id: the
manager keeps a lock per id, held only while the scope is started or shut
down, never for a call. When the scope isn't running it re-reads the
extension and its grants under that lock and spawns from them, so a caller
holding an old row still reaches the installed script, an uninstalled
extension fails with `NotFound` instead of respawning, and a restart waits
for a spawn under way and then discards it. A global the daemon calls is
a type implementing `ExtensionFn` (`extension_manager/extension_fn.rs`):
its `NAME`, the one permission it `REQUIRES`, its arguments and its
`Output`, so `ExtensionManager::call` can't pair a function with the wrong
grant or result (`ListResourcesFn` in `connector_calls.rs`,
`RegistryLookupFn` in `registry_fns.rs`). A permission that only works
beside another says so in `ExtensionPermissionKind::needs` (`connector`
needs `oauth`), which both the manifest check and the grant check read. A
hook call names its hook
(`IsExtensionHookEnabledCommand`, one `EXISTS`); both are checked under the
lock as the call is made, which also takes a `ScopeHandle` on the worker. A
grant change commits and then takes the same lock to shut the scope down
(its timers stop once no call holds it), so every call after it sees the
change, while a call that checked first keeps its handle and runs once on the
scope it found, as a call already under way does.

`RegisterExtension` asks the manager to install: it evaluates the script on
the engine's isolated worker (no globals) to read the manifest, rejects one
listing `connector` without `oauth` or a `service` block it may not declare
(`extension_manager/service_manifest.rs`), drops an icon it doesn't accept
(`cohesionkit/src/extension_icon.rs`), then stores it with its permissions
and hooks in one `InstallExtensionCommand`, all switched off until the user
grants them (a reinstall keeps existing grants the manifest still lists).
Installing, changing a grant (`SetExtensionPermission`/`SetExtensionHook`)
or uninstalling shuts the extension's scope down, so it respawns with the
new script and mounts. A restart queues `launch` for that extension when the
hook is on, at most one pending per extension, and the launch runs only if
the hook is still on when its turn comes. `DatabaseContext` hands each
committed command's `DatabaseEvent`s to `HookManager`
(`cohesionkit/src/hooks/hook_manager.rs`), which queues them on one channel
as hook events (`hook_events.rs`); `day_clock.rs` queues `day:start`,
`day:end` and the minutely `timer` on the same channel from the wall clock,
following each `day:start` with `task:start` and `task:due` for the
incomplete tasks that start or are due that day (`ListTasksCommand`), and
the update check queues `extension:update`. An agent's foreground turn
raises `agent:turn:start` and `agent:turn:end` through
`RecordAgentTurnCommand`, which writes nothing: a turn may post nothing, so
no other write marks it. A delete event (`TaskDeleted`, `FeedDeleted`,
`ResourceDeleted`) carries the entity as it was, since the hook can no
longer look it up. `HookReceiver` (`hook_receiver.rs`)
handles them serially: it looks up the extensions with the event's hook
enabled (`GetExtensionsWithHookCommand`), calls the hook on each through the
manager, then offers the event to the agents (see Intelligence). The
manifest-vetting worker is read-only.
A service extension's `service` block is stored with it, as
`ExtensionService` and its `ExtensionServiceFunction`s. `callExtension`
refuses a name the block doesn't declare, and `ListServicesCommand` lists
the declared services whose `service` grant is on (`service:isolated` ones
never), ordered by identifier, for the agents' index and declarations.
`services.use(identifier)` looks one service up by its extension's script
identifier and throws unless it is installed, granted `service` (not
isolated) and declares a `service` block; there is no way to list services
from a script. The engine reaches them through the `ServiceAccess` the
daemon gives it, which calls `serviceRequest` on the service's scoped worker
through the manager. A caller whose sandbox database is read-only (a
turn a cue started, a cue, or an
extension without `cohesion:write`) may call only
functions declared `readOnly`; any other throws at call time. Read-only does
not carry over: the service runs with its own grants. The nested call is cut
off at the caller's own deadline, throwing in the caller.
The engine tracks which extensions' scopes are waiting on which, so a call
back into an extension already waiting on the caller (or into the caller
itself) throws rather than wait on its own worker forever.
Registry search goes through the manager: `SearchExtensionRegistries` calls
`registryResults(query)` on each extension granted `registry`, and
`PreviewExtension` downloads a script and reads its manifest for review.
The manager also keeps the updates
(`cohesionkit/src/extension_manager/extension_updates.rs`): it calls each
registry's `registryLookup(identifiers)` with the installed identifiers at
start, daily and on `CheckExtensionUpdates`, keeping the entries at a
higher version (versions are whole numbers) for `ListExtensionUpdates`, which offers one only while the
extension with that id still has the update's identifier. An uninstall
drops the updates for, and offered by, that extension. The list lives in
memory, so it has no model; a finished check publishes
`Invalidation::ExtensionUpdates`, and installing or uninstalling raises it
in the command, so clients refetch what still applies. Each extension's row records the version last announced: a
check claims each update's version with `AnnounceExtensionUpdateCommand`
(one `UPDATE … WHERE announced_version IS NOT ?`), and the one that changes
the row queues `extension:update` on the `HookManager`, so a version is
announced once, across restarts and concurrent checks.
`CallExtension` (`extension_routes.rs`) lets a client reach an extension granted
`service`, named by its manifest identifier: it looks the extension up by
that identifier (`NotFound` when none is installed), calls its
`serviceRequest(name, args, caller)` through the manager and answers its JSON (`undefined` as `null`), showing what the script
threw as a validation error. It routes `via = FromClient`, since `caller`
is `{ clientId }`, naming the client asking (`ServiceCaller::Client`), so a
service can keep something for that client and forget it on
`client:revoke`; a script's call through `services` passes `null`.

An extension granted `oauth` signs the user in to a service: the daemon calls
its sign-in functions to add connector accounts, passing each sign-in a PKCE
pair (`cohesion_core::auth::Pkce`) whose verifier `PendingSignIns`
(`cohesionkit/src/connectors/pending_sign_ins.rs`) holds in memory, by
extension, between `beginConnectorAuthentication` and
`completeConnectorAuthentication`, so clients send only the code. Its scope mounts the
`accounts` global (`cohesion-js/src/globals/accounts_global.rs`), which lists the
extension's own accounts.

Credentials are reached only through `CredentialManager`
(`cohesion-db/src/credential_manager.rs`), from `DatabaseContext::credentials`
or a script's `GuardedDatabaseContext::credentials`. `ConnectorAccount` leaves
the credential columns out, and the commands that read and write them are
private to cohesion-db, so no other command, client or script sees a token.
`fresh(extension_id, account_id, refresh)` hands a credential only to the
extension serving the account and, under a lock per account, re-reads it,
calls `refresh` if it expires within a minute, and stores the result whatever
the caller's `cohesion:write` grant. A second caller finding the same
credential expiring waits and gets the stored refresh, so one refresh token is
never spent twice. Asking for a credential while this thread runs a refresh
fails (`CredentialError::Reentrant`) rather than waiting on itself. An
account's `credential()` passes the script's own `completeRefresh`, and the
daemon's `fresh_credential` (`cohesionkit/src/connectors/credential_refresh.rs`)
runs the manager as a job on the extension's scope, so every refresh of an
account runs where a script's would, never awaiting a scope while holding the
lock.

An extension granted `connector` as well is a connector, whose import
functions bring an account's items in as resources. `cohesionkit/src/connectors.rs`
and its files call both sets of functions through the manager, each call
checking its grants (`connector_calls.rs`), and fetch an account's credential
through `fresh_credential` before the daemon uses it; every account call
matches on `ConnectorAccount::host`. Agents reach the same
functions through `connectors` (see Scripts). `resource_refresh.rs` holds the loop that
refreshes imported resources, started with the daemon, and
`refresh_resources`, which `RefreshResources` also calls. A refresh lists each account's items first and fetches only those whose
`lastUpdated` moved or that weren't listed; a fetch replaces the
resource's metadata (`metadata` rows with a `resource_id`,
keys from `MetadataKey`), which scripts read with `resource.get(key)`.
Importing a resource raises the
`resource:import` hook and a refresh that changes one raises `resource:update`.

## Intelligence

- `ModelCatalog` fetches the models.dev catalog (`catalog/models_dev_schema.rs`
  holds its serde shapes) into `CatalogModel` rows: capabilities and pricing per
  provider model. The daemon refreshes it at start; `UpdateModelCatalog`
  refreshes it on demand.
- An `IntelligenceEndpoint` row is a `name` (unique, ignoring case, by its
  column) and an `EndpointBackend`: for `Integrated`, a provider, an API base URL,
  key and `Dialect`; for `Account`, the same with a connector account in
  place of the key. Each kind has a backend:
  `ResponsesBackend` (OpenAI Responses), `MessagesBackend` (Anthropic
  Messages) and `ChatCompletionsBackend` (OpenAI-compatible chat
  completions), each in its own
  file under `cohesion-intelligence/src/providers/`. `StatelessResponses` is
  `ResponsesBackend` with its `ResponsesPreset::Stateless`: nothing stored on
  the server, the whole transcript sent each turn and the reply streamed, as
  a ChatGPT plan requires. A transcript's `CachePlan` says which cache each
  part asks for, as a `CacheLength` the endpoint resolves: the tools and the
  system prompt ask for its `prefix` (`Long`, but `Short` for a cue's background
  run), its first `shared_items` items (what other threads copy)
  ask for `Long`, the rest for its `tail` (`Long` unless the session's
  `cache_tail` says otherwise). `MessagesBackend` marks up to four cache
  breakpoints: the last tool (the tools of one kind of session are the same
  for every agent), the end of the system prompt and of the shared items,
  each for the long cache, and the end of the transcript for the tail's,
  so a new thread reads back whichever prefix an earlier request wrote.
  Anything that varies between requests belongs after them, in the
  transcript. A Responses server writes only where a breakpoint is, by
  default the end of the latest message, which a thread copying the shared
  items never sends (it adds its own), so to an endpoint with a long cache
  `ResponsesBackend` marks the end of the system prompt (sent as a
  developer message; a stateless request's `instructions` can hold none) and
  the last shared item with an explicit `prompt_cache_breakpoint` for the
  long cache, and asks the server's own breakpoint for the tail's
  (`prompt_cache_options`).
  An endpoint has a short prompt cache lasting `cache_minutes` and may have
  a long one, `long_cache` (a `LongCache`: its `minutes` and its
  `write_multiple`, what a write to it costs as a multiple of the short
  cache's write rate, the only one the catalog has). A provider's preset
  gives its own (an hour at 1.6 for Anthropic, twelve hours at 2 for
  OpenAI), a custom endpoint is given one when it is created, and
  `SetIntelligenceEndpointLongCache` changes or removes it.
  `EndpointBackend::cache_lifetime` gives a `CacheLength` its cache, the
  short one for `Long` when the endpoint has no long one, and the tail's
  minutes are what the daemon judges a thread's warmth by. A long cache's `ttl` is its minutes
  (`LongCache::ttl`: `1h`, `30m`), sent on each Messages breakpoint and
  on each Responses breakpoint that asks for it; chat completions offer
  no choice. Each backend reports how many tokens it wrote to the long
  cache, and the session prices them at the short write rate times
  `write_multiple`. Two endpoints may share
  a provider (a work and a personal account), each with its own name and
  models.
- An `Account` endpoint is how an `oauth` extension (the ChatGPT one in
  `extensions`) offers a service whose key expires: the extension signs the
  user in, then `account.createEndpoint(...)` creates it. Its backend's
  `EndpointKey::Account` asks for the account's credential before every
  request through `AccountCredentials`, a trait cohesion-intelligence declares
  and cohesionkit's `EndpointRelay` implements with `fresh_credential`, so
  the extension's `completeRefresh` renews it under the account's lock (see
  Connectors). Removing the account deletes the
  endpoint (a foreign key's cascade), as does uninstalling the extension,
  which orphans the account (a trigger).
- An endpoint's models are `EndpointModel` rows (`endpoint_models`), the
  only models a tier or a picker names; the catalog is browsed
  (`ListCatalogModels`) and imported from, never picked directly.
  `AddEndpointModel` upserts by endpoint and identifier, linking the
  catalog row of the endpoint's provider with that identifier when there
  is one. It writes the whole row: the fields given override the
  catalog's, and one left out clears its override. The catalog sync only fills a missing link, once the catalog
  lists the model, and never writes a row's own fields. The readers
  (`ListEndpointModelsCommand`, `GetEndpointModelCommand`) left-join the
  catalog into one flat `EndpointModel`, a row's own value winning. Creating a provider's endpoint
  imports its preset's `default_model`, so a first run works.
- Sessions don't name a model: each runs on an `IntelligenceTier`'s
  `IntelligenceDefault`, which points at one `endpoint_models` row and is
  unset by the cascade when that row (or its endpoint) goes.
  `GetIntelligenceDefaultCommand` reads it as a `TierModel`, the endpoint
  model with its endpoint, when a session starts (`tier_model`), and
  `Session::new` builds the endpoint's backend from it. `agent` serves every
  agent conversation (and `intelligence.model()` with no tier); `low`,
  `medium` and `high` serve the forks cues wake, as the `effort` of the cue that woke
  them picks.
- `Session` drives one conversation: it creates an `IntelligenceThread`,
  sends turns to the backend, runs tool calls (an agent's code tool runs
  JavaScript through `ScriptEngine`, and its description is the script API
  plus an "Installed services" index of the services `services.use` reaches,
  `InstalledServices`, and a "Connected services" section listing the user's
  connector accounts and their resource kinds, `ConnectedServices`, both loaded
  when the session is built), persists every transcript item (prompts and
  tool results included, which the read-only transcript view shows), and
  records an
  `IntelligenceCostEvent` per turn: its thread, token counts and what they
  cost at the endpoint model's rates (none when it has no prices; a write
  to a long cache is priced at the endpoint's multiple of the short
  cache's write rate, since the catalog lists only that rate). `ListIntelligenceCostEvents` lists them, and
  `GetIntelligenceThread` returns a thread's own. A thread nothing has been
  added to for two weeks is deleted with its messages
  (`DeleteIdleThreadsCommand`); its runs and cost events stay, without a
  transcript.
- `IntelligenceSpendLimit`s cap what every model call together may cost,
  one row per `SpendPeriod` at most (`hour`, `day`, `month`; none to
  start with, and a period without a row has no limit). The periods are
  rolling: the last 60 minutes, 24 hours or 30 days, summed from the cost
  events (`ListIntelligenceSpend` answers each period's `IntelligenceSpend`:
  its limit, what it spent and whether that `reached` the limit, which is
  decided there and nowhere else). `SetIntelligenceSpendLimit` and
  `ClearIntelligenceSpendLimit` answer the same list and raise
  `Intelligence`; Settings › Agents shows them, and an agent's conversation
  says under its composer while a limit is reached.
- An `IntelligenceAgent` is a persistent assistant; its
  `intelligence_agent_messages` are its conversation, its
  `IntelligenceAgentCue`s each name a hook that wakes it (unique by name per
  agent, so setting one again replaces it) and the `IntelligenceEffort` of
  the fork it wakes, and each fork is an
  `IntelligenceAgentRun`, recording the cue's name, whose tokens and cost are summed from its thread's
  cost events when it finishes. An `ActorSessionFollower` says which agent
  follows an actor session its conversation handed work (below).

Agents run in `cohesion-intelligence/src/agents/`. `CohesionContext.agents`
is an `AgentCoordinator`, a cheap clone that keeps one `AgentHost` per agent,
created on first use and evicted when the agent is updated or deleted. A
host's lock serialises the agent's foreground. The requests that start a
turn don't wait for it: `SendAgentMessage` records the user's message, with
any images attached to it (`intelligence_agent_attachments`, base64, which
only that turn's prompt carries to the model), `RespondToAgentChoice`
records the pick in the choice's `selected`, and `SetAgentMessageReaction`
the user's reaction to one of the agent's messages; each then takes its
place in the queue on the host's lock (`AgentCoordinator::queue`, polled
once so turns run in the order their requests came), spawns the turn as the
agent's work and answers `AcceptedResponse`. A message sent while a turn
runs steers it instead: the agent's `ForegroundTurn` (`foreground_turn.rs`,
one per agent, kept across hosts) takes it in, and the session adds it to
the thread before its next model call, under a heading saying it came
while the agent worked (`Session::send_steerable`, through the `TurnInput`
trait). A turn also ends on a tool call the model marks `done`
(`SessionTool::ends_turn`: `send_message`, `ask_choice`, `send_note`,
`react`, `reply_to_agent`, the code tool and a one-way `ask_agent` take
`done`, read with `asks_done`) once every
call in that response succeeded, rather than spending a model call on a
final response nothing reads. Once a turn would end with nothing waiting it stops taking
messages in, under the same lock `send_message` steers through, so a
message either joins the turn or queues its own. `StopAgentTurn` stops the
running turn: the model call it waits on is dropped, its script is
interrupted through a `ScriptStop` (recorded as a `cancelled` code run),
the tool calls it has yet to run are answered as stopped so the thread
stays whole, and messages it never took in get turns of their own.
Clients follow the turn through the agent's invalidations: it has an
`activity` and a `foreground_started` until it ends, and what it posts
lands in the feed. The turn talks to the user only
through its tools: `send_message` posts short messages to the feed as it
goes (`message_tool.rs`), with any images its scripts attached: a script
attaches an image it fetched or made, or an actor session message's, through
the `images` global (`Mount::Images`, in every turn and run but never a
cue), each kept as one more effect (`Effects`) and, once the script
succeeds, numbered in the session's `ImageTray` (`image_tray.rs`, the
latest 10 kept), whose numbers `send_message`'s `images` names;
`CreateAgentMessageWithImagesCommand` stores them as the user's are, in
`intelligence_agent_attachments`, through the same check. `ask_choice` posts a `choice`, a question with
lettered answers, and `react` (`react_tool.rs`) puts an emoji on a message
of the user's (clients offer `AGENT_REACTIONS`). Every message carries an
`excerpt`, one line read through `IntelligenceAgentMessage::EXCERPT_SQL`:
what a reply quoting it, a line about a reaction to it and the reaction's
prompt show, and how a client shows a kind it has no view for, so no client
words a kind itself. The agent's bookkeeping (a memory, a note, a wake)
also carries an `excerpt_verb` (`Remembered`, `Forgot`, `Noted`, `Woke`),
with `excerpt` holding what follows it, so a client can head a run of
such lines with the verb or show one as `Noted: …`. A reacted message's
line about its reaction comes the same way, `reaction_excerpt_verb`
(`Reacted`) and `reaction_excerpt` (`👍 to “Done.”`). A fork's messages carry
its `run_id`, and `run_part` (`IntelligenceAgentMessage::RUN_PART_SQL`, `null`
in the conversation) says what each is to it, so no client sorts kinds itself:
the `trigger` that woke it (its `woke` line, or another agent's question), a
`step` of its work, which a client collapses under the trigger, or what it
`surfaced` to the user (a text, a choice, a failure), which stays in the main
transcript. A fork's messages can fall among others, so a client groups them by
run; a reaction is on the message it answers, never a part of a fork. The model's final reply
stays in its thread and is never posted: a turn that answers a message or
a pick (or took one in as it worked) and ends having sent and reacted to
nothing is told so once (`agent_transcript::UNANSWERED`) and goes on
for one more answer, so a reply left in the final text still reaches the
user. A turn under a reaction is never told; a fork that owes another
agent a reply is told the same way (below). A pick or a reaction posts no message of the user's: a
new thread's status block reads it from the message's line. Either side's
reaction raises `agent:reaction`. An agent's **main thread** is the
conversation's thread as its mark has it, and whatever starts next, a chat turn
or a foreground wake (a fork for an actor session, another agent's question or an
assigned task), first asks `main_thread::prepare` for it, the one place that
decides what happens to it (`ThreadMark::fitness`, `conversation_mark.rs`). A
background wake, a cue, never touches it (below). A main thread
whose cache hasn't expired (each endpoint says how many minutes its long cache
lasts, or its short one when it has none, from its provider's default or the
user) is reused, however long since its latest call. Once it has expired, the
last prompt passed 100K tokens as the provider counted them, the `agent` tier's
default changed, Memory capacity changed, or the last turn failed, the context is
**rebuilt**: nothing of the old thread is edited or carried over, and the next start
builds a new one from the database, with a current system prompt. With no thread yet
it does the same. `prepare` says which reason applies, and the new thread's mark
replaces the old when its first turn ends, so what follows carries it on or copies
it. A live conversation whose mark has moved follows it (`AgentHost::resume`). A
prompt on a thread the conversation just moved to, and every foreground fork's,
opens with the time and cues, since the opening's may be old.
A rebuild opens the main thread: the host with a compact transcript
(`agent_transcript.rs`), the time, cues and the messages still relevant, scored by
depth and age (`relevant_set.rs`), and a fork with the recent conversation. Lines for
scripts and notes are left out past `WORK_LIFETIME_SECS` (an hour), so a long-lived
conversation's work doesn't crowd out what was said. A line keeps only the first 800 characters of a long
message's text (a question between agents' too: the question, then
`Reply: …` or why it went unanswered), noting how many it left out; `read_message`
(`read_message_tool.rs`) returns any range of it, and the history budget
counts the excerpted lines. The code tool's result is cut the same way at
32,000 characters (about 8K tokens, `agent_code_call.rs`), so a script
returning everything doesn't fill the thread; the whole output is the run's
`codeExecution` message, which `read_message` reads. When that transcript holds more than the
message the turn answers, the conversation is being rebuilt rather than
begun (and the provider's cached prompt is lost), so the host posts a
`rebuiltContext` line after that message (a fork posts one when it replaces a thread), whose excerpt is
`Context rebuilt because …`, the reason being a `ContextRebuildReason`; it is bookkeeping like a note. A live thread instead hears what forks posted
(a script that finished only by id and title, which `read_message` reads, since
those lines would fill a long-lived thread)
in front of the next user message. Which messages
a thread holds is on its mark (`agent_thread_marks`, below): `extended_from`,
where its extended context starts (the oldest message the status block
holds, earlier ones being out of the thread), through `heard_through`, the
newest it has been told of. The messages after the status block's newest
are the thread's own turns. A mark is written when a turn ends, so a thread
with no mark (a failed turn, an eviction, or a restart that cut a turn
short, below) is one the next turn rebuilds. The agent's `context_tokens` is how many tokens the
live thread held at its latest request, as the provider counted the prompt
(`Usage::prompt_tokens`: uncached input, cache reads and cache writes, not
the reply). The conversation's `Session` (built `reporting_to` the agent)
sends `SetAgentContextTokensCommand` after every model request of a turn,
so it moves while a long turn works and each request replaces the last; a
fork's session reports nothing, because a fork's prompt holds what the
conversation's thread doesn't (the conversation hears forks only at its
next message). Starting or dropping a thread and the shutdown settle (for
an agent left no conversation mark) clear it, so a new thread has none
until its first request is answered. The
daemon's idle-thread cleanup deletes only stored rows; the count stays
until the next turn replaces a spent thread. The web UI's agent chat draws no rule for either bound. Agents
are organized into teams (`intelligence_agent_teams`, one title each,
unique ignoring case; an agent's `team_id`, set with `SetAgentTeam`), apart
from pinning: a pinned lead stays pinned on its team. A conversation's
system prompt lists its teammates under `# Your team` and every other agent
under `# Other agents`, each by name, id and role (`Roster`, built with the
thread), in id order rather than the client's most-recent-first, so the
prompt, and the cache that holds it, stays the same as agents talk. A memory (the `memories` table) is an agent's, a team's or global
(`MemoryScope`); each scope keeps at most the storage settings'
`memory_limit` (20 by default), and `remember` past it is refused with a
request to compact that scope; lowering the limit forgets nothing. Every
memory the agent sees, its own, then its team's, then the global ones,
each by id, closes the system prompt under `# Memories` (`SystemPrompt`'s
`memories`, after a line stating `memory_limit`, and just below the
`# Your model: <identifier>` line naming the model's `model_identifier`, with
`# About the user` between them: the name and bio on the user's profile, each when set, and no
section when neither is, `SystemPrompt`'s `user`), in every role, so a long thread reads them where its rules
are rather than far back in its first message, as they stood when the
thread began: a thread's prompt never changes, and another model or a changed
`memory_limit` rebuilds it (below), since a changed one
would miss the cached prefix past the tools. A changed profile rebuilds nothing, and no
message tells a thread of it: a thread already stored keeps the user it began with until its next rebuild. For memories,
the thread hears what changed instead
(`Session::hear_memories`, which a fork of the conversation calls too).
Its catch-up opens with `# Memories changed`, each memory remembered
since it last heard as the prompt lists it and `forgot [m12]` for each
gone, its own `remember` and `forget` included, and the session's
`SystemPrompt` keeps the list as heard, so the next change is told
against it. A fork's `remember` and `forget` mark their feed messages
with its `run_id`, as its scripts and sends are. The conversation keeps them with the `remember` and `forget` tools
(`memory_tools.rs`). The user reviews and forgets them in the web UI's
Settings › Memories (`ListMemories`, `ForgetMemory`). Deleting a team
leaves its agents on none and forgets its memories, both through the
schema's foreign keys.

`HookReceiver` hands every broadcast hook event to `AgentCoordinator::fire`
once the extensions have run it, so a cue sees what they changed. `fire`
evaluates each cue on that hook (skipping an agent's own writes, which
are traced with the agent as their origin) and, for each that fires, wakes
a fork of the agent's conversation (`AgentMessenger::cue_fired`), several
for one agent when more than one of its cues does, so the hook queue never
waits on a model. Forks of one cue take turns, in the order they fired;
one whose cue the user removed while it waited is skipped. The fork runs
on the default model of the tier the cue's `effort` names, starts fresh, with only
the system prompt, the time and its own prompt, all on the short cache
(`CachePlan::prefix`), never touching the main thread, and opens under
`# Cue "<name>" fired` with the cue's instructions and what its function
returned, and is recorded as a run (`hook`, `cue_name`) with a `woke` line
in the feed.

**Who started the chain.** Every turn carries an `Initiative` on its
trace: `User` for a turn the user's message or pick started, `Event` for a
fork a cue woke. Every fork a chain wakes keeps it (a question's fork from
the asker's trace, an actor follow-up from the actor session's
`follower_initiative`, which `FollowActorSessionCommand` records from the turn
that told the actor session something), so a chain never gains rights. Every script
an agent runs runs as one `AgentRole`
(`cohesion-js/src/engine/agent_role.rs`), and the role alone says what it
mounts and which areas its database may write; nothing inside a mount
checks who is calling. `Turn(User)` (the chat, and forks the user's work
leads to) mounts read-write `cohesion`, `network`, `agent`,
`connectors`, `actors`, `images`, `services`, `storage` and
`notifications`. `Turn(Event)` mounts the same without `network`, over a
database that writes only notifications. Every turn's code tool documents
`Turn(User)`'s mounts (`AgentRole::DOCUMENTED`) and every fork shares one
system prompt, so the cached prefix is the same whoever started the chain;
the fork's opening says what it may do (`agent_transcript::fork_rights`).
What lives outside the sandbox follows the same role: an agent's tools
write its memories and cues (`remember`, `forget`, and the `agent.cue` and
`agent.removeCue` effects `AppliedEffects::foreground` applies) through the
role's database (`agent_tool::role_database`, `AgentRole::database`), and
`Turn(Event)`'s has no `AgentSettings` area, so those writes are refused
there and nowhere else. `Cue` mounts read-only `cohesion`,
`services`, `storage` and `notifications`. Blocking calls out of a script
(`network.fetch`, `connectors`, `services`) are bounded by the job's
`ScriptDeadline`. While any turn or fork is in
progress the agent's `activity` says what the newest of them is doing
(`agent_activity.rs`): a rotating word while it waits on the model, or what
the tool it runs does (`SessionTool::activity`; a script's title), and
`null` once all have ended.

A conversation asks another agent with its `ask_agent` tool
(`ask_tool.rs`) in one of two modes (`AskMode`). By default it asks for
a reply and waits up to 10 minutes (`ASK_LIMIT`) for it; with `reply`
false it hands the work over one way and returns at once, and no reply
ever comes. `AgentMessenger`
(`agent_messenger.rs`) carries the question: `AskAgentCommand` records the
asked agent's run (`hook` `agent:message`, `sender_agent_id` the asker) and
the question twice, as `askedAgent` in the asker's conversation and
`askedByAgent` in the other's, both `waiting` and neither marking the agent
unread. The messenger then wakes a **fork** of the asked agent's
conversation (`agent_fork.rs`) as that agent's work: a session with the
conversation's system prompt and tools (`conversation_session.rs`, shared
so a fork's prompt is the conversation's), and so its rights. An agent's
tools are `AgentTool`s (`agent_tool.rs`), built with no session in reach:
a call learns which session it runs in from its `Seat` (the turn and the
`Branch`), so no definition can differ between the conversation and a
fork, which would cost the fork the conversation's cached prompt. Its final
response is no one's answer: a fork asked for a reply answers with
`reply_to_agent` (`reply_tool.rs`), and a one-way fork only does the work
and ends with OK. Every conversation session has `reply_to_agent`, so the
tools stay the same and the prompt stays cached; only a fork that owes a
reply can use it. Each host leaves a `ThreadMark`
(`conversation_mark.rs`) when a turn ends: the thread, how many items it
held then, the `memory_limit` its system prompt states and the stretch of the conversation it holds (`extended_from`
through `heard_through`, the newest message it had been told of), the memories it last heard of (`HeardMemories`), and its
latest call. Marks are rows (`agent_thread_marks`, an `AgentThreadMark`,
one per agent), so
they outlive the daemon: a host with no thread (a new one, as after a
restart) carries the conversation on from its mark while that is warm on
the `agent` tier's model, in a copy of the marked items
(`AgentHost::resume`, through `Session::fork`), with the working set read
back from the mark's `extended_from` through `heard_through`, and so
pays a cache read rather than a fresh write. `main_thread::prepare` rebuilds a mark
on another model (`ModelChanged`), since the old model's cache is no use to it, and
one whose `memory_limit` isn't the setting's (`MemoryCapacityChanged`), so changing
Memory capacity reaches each agent's main thread when it next starts, with no turn
interrupted and no host evicted, and a cue's fresh thread just reads the setting. The
restart this matters for is a long cache's: a short one is gone before
most restarts end. A Responses thread's stored chain (`remote_id`,
`remote_cursor`) is on its thread row and the copy carries it on. The
shutdown settle forgets a conversation mark whose thread holds more items
than it says (`SettleAgentThreadMarksCommand`: a turn began after it and
the daemon stopped during it, so the feed, not the mark, holds what that
turn did), and keeps the context tokens of the agents
left with one. A thread's cache lives from the start of the latest request
that read it: its own calls (`LastCall`), and a fork that copies a mark
refreshes it with its first request's time
(`RefreshAgentThreadMarkCommand`, only while the mark still has the items
the fork copied), which wrote the long cache at the copied items' end, so
the conversation's next turn and later forks judge its warmth by the
latest read (`ThreadMark::fitness`). Only the first request counts, since an
OpenAI long cache isn't extended by a later read (a 30-minute read keeps
it at least 30 minutes, a 12-hour one leaves it as it was), while an
Anthropic read refreshes its entry every time. A foreground fork
copies the main thread as `main_thread::prepare` leaves it, in a thread of its own, which reads the items from the thread holding them rather than copying them (`ForkIntelligenceThreadCommand`, `ThreadOrigin::Fork`; a thread's system prompt is one row of `thread_prompts`, shared), keeping a
stored response chain that ends within them), so it shares the cache and
never sees a turn still under way. With no main thread it opens one with the
recent conversation as its shared first item, then the time and cues, and leaves
that as the agent's mark when it ends well. Either
way it is prompted with what the conversation said after `heard_through`,
then the question under `# <name> asks`; what a fork adds past what it
copied or shared asks for the short cache (`CacheLength::Short`), since
nothing reads it back once the fork ends. A fork's `heard_through` is the newest message its prompt
or the thread it copied told it of, never merely the newest in the
database when it ends, so the next fork still hears what a turn running
alongside it said. A fork's scripts are recorded in the feed like the
conversation's, marked with the fork's `run_id`, as is what it sends the
user (`sender` `agent`), which the conversation catches up on as it does a
fork's notes. Every turn has the tool; a cue's function never does.

Only the fork settles the question (`AnswerAgentAskCommand` sets both
messages `answered` or `unanswered`). A fork that owes a reply holds an
`OwedReply`: its `reply_to_agent` settles the question and hands the reply
to the asker's waiting `ask_agent` call, if it still waits. One that ends
its turn without replying is told once (`agent_transcript::UNREPLIED`, on
the same thread, as a turn that answers the user and sends nothing is) and
goes on for one more answer; if it still hasn't replied, the question is
settled `unanswered` and the asker hears "No answer", never a hang. A
one-way fork's final response settles its question and reaches no one;
one that ends without one, on a call marked `done`, settles it `OK`.
Nothing ever wakes the asker's conversation with an answer: an asker that
gave up after `ASK_LIMIT`, or that the user stopped, sees the reply only
in its conversation's transcript, and one-way work that should report back
says so in the question, so the agent asked asks the asker in turn. Every
foreground turn carries a trace (`ForegroundTurn`): a fresh one when the
user starts it, and the question's when it is a fork; its writes and
questions continue it. A trace counts two things apart. Its depth counts
the events a chain of hooks raised, and `DatabaseEventTrace::MAX_DEPTH`
(3) drops the hooks of an event past it. Its hops count the forks a chain
has been handed through, actor-session follow-ups and questions alike: each
fork is one hop further, taken and logged in one place (`hop` in
`agent_messenger.rs`) and never limited; the spend limits are what hold a
chain that runs on.
A message of the user's that joins a running turn also starts its trace
over. The user's stop of an agent stops its
conversation's turn and its running forks (`AgentMessenger::stop_forks`,
which the fork is counted in before it runs), which answer that they were
stopped; the daemon's shutdown stops every fork, a waiting asker hears
that Cohesion stopped, and `ShutdownCleanupCommand` settles any question
still `waiting`. A turn's `ForegroundTurn` is begun and ended by
generation, so a turn whose host lock was released before it ended can't
close the one queued behind it.

An actor session the conversation hands work wakes a fork too, so the agent
needn't cue on it. A foreground script's `actors.dispatch` and
`actors.send` record a `ForegroundEffect::Follow`, which the code tool (or
the fork) applies as `FollowActorSessionCommand`: it sets the
actor session's `follower_agent_id` column (`ActorSession::follower`, with who started the chain) to the agent that last told it
something, and nothing clears it but deleting that agent. Other roles'
scripts follow nothing: the effect is the conversation's. What runs actor
sessions (`ActorSessions`) moves an actor session with an `ActorSessionMove` saying why, which
`SetActorSessionStatusCommand` turns into the status and carries on
`ActorSessionMoved`: the actor's own report (`Working`, `NeedsYou`,
`Finished`, `Failed`), its client going away (`Interrupted`), someone
stopping it (`Stopped`), or a message that never reached the actor
putting it back (`PutBack`). The status alone can't tell these apart: a
finish, a stop and a lost client all leave the actor session idle. When the event
reaches `AgentCoordinator::fire`, `ActorSessionMove::stop` says whether the
actor session stopped on its own (`ActorSessionStop`: needs the user, finished,
failed, interrupted); only then does its follower wake, so a stop or a
actor session starting work wakes no one, and each status change wakes one fork at
most. The follower's messenger records a run (`hook` `actor:moved`, no
cue, `CreateAgentForkRunCommand`) and wakes a fork prompted under
`# Actor session "<title>"` with the actor session's link, how it stopped, its prompt
(what the conversation told it, its first message, cut at 800
characters) and what its actor said since it was last told something
(`actor_exchange.rs`), and that the fork may do all a turn of the
conversation can, so it finishes the work itself,
under the moving event's trace one hop further (the event is already a
level deep, so the wake adds no depth). It carries on the work, such as
telling the user or dispatching the next actor session (which it follows in turn),
and its final reply goes to no one. Clients see the follow as the actor session's
`follower_agent_id`, so following invalidates the actor session; the web UI lists
the actor sessions not yet resolved that an agent follows in the column right of that
agent's conversation (`AgentGutter`, in `ConversationGutter`).
An agent also carries the count of them, `following_count`, computed on each read so a
client that doesn't hold the sessions can show it (the web UI's sidebar does). A write
that changes who counts a session (`ActorSession::counted_by`) invalidates those agents
too: a follow, a move into or out of resolved, or a delete; other moves don't, so they
don't refetch an open conversation.

A task assigned to an agent wakes a fork too, with no cue: assigning is
the user's go-ahead. When `task:assigned` or `task:create` reaches
`AgentCoordinator::fire`, `wake_assignee` reads the task as it is now and,
when its assignee is an agent, wakes a fork of that agent's conversation
(`AgentMessenger::assigned`), unless the assigning write was the agent's
own (the trace's origin, as for cues) or came from a chain an event
started (a job, which never wakes a fork). So the user, a script, an
extension and another agent's turn or fork all wake it, each fork with the
user's rights. The fork is recorded as a run (`hook` `task:assigned`, no
cue, `CreateAgentForkRunCommand`) with a `woke` line (`AgentWake::AssignedTask`,
`“<title>” was assigned to you`), opens under `# Task "<title>" assigned
to you` with the task's link and description, under the event's trace one
hop further, and is held by the spend limits like any fork. Since it
rides the hook queue, an assignment made more than `MAX_DEPTH` levels
down a chain of hooks wakes nothing.

**Notes and wake lines.** Two kinds of feed message (three with the
`rebuiltContext` line above, which carries no run) are the agent's own
bookkeeping (`AgentMessageContent::is_bookkeeping`): a `note`, which the
`send_note` tool (every conversation and fork has it) writes for the
agent's later turns, cut at 1,000 characters, and a `woke` line
(`AgentWake`), which says why work apart from the conversation started: a
cue firing (written as its fork wakes) or an actor session it follows
stopping on its own (written as the follow-up fork starts). Both carry the
run's id, so what that work posts stands under them. They raise no
`agent:message` hook and, like a script the agent ran, leave the agent read
(the `intelligence_agent_messages_unread` trigger's list of kinds). A live
thread and a fork of it hear both, and a thread opened from the database
hears them through the compact transcript while the relevant set holds them,
so a wake line outlives a cold thread and a restart. A note does so only for
its first hour: no cleanup deletes a note, and it stays in the feed, but
`WORK_LIFETIME_SECS` leaves it out of a rebuilt thread's relevant set after
that. So `send_note` and the system prompt tell an agent to restate what is
pending in its newest note and to keep plans that must outlast the thread in
`storage` or on their task. A note doesn't count as a turn answering the
user.

**At a glance.**

| | Turn the user started | Fork an event started | Cue function |
|---|---|---|---|
| Started by | the user's message, pick or reaction; questions and follow-ups from them | a cue firing; questions and follow-ups from it | its hook firing |
| `AgentRole` | `Turn(User)` | `Turn(Event)` | `Cue` |
| Writes user data | yes | no | no |
| `network` | yes | no | no |
| `actors` | yes, followed | yes, followed (as `Event`) | no |
| Sets cues, memories | yes | no | no |
| Asks other agents | yes | yes (their fork is `Event` too) | no |
| Talks to the user | `send_message`, `ask_choice`, `react`, `send_note` | the same | notifications |
| Held by spend limits | no (forks yes) | yes | no model |

A cue's function that fails (throws, isn't a function, runs out of time)
says so every time it fails, with no dedupe or limit, so a broken timer cue
says so each minute: `ReportAgentErrorCommand` posts a `backgroundAgent` `error`
message to its agent's feed, `The cue "Name" failed on hook: …` with the
error cut to 500 characters, so a broken cue doesn't look like a quiet
agent. Nothing is kept between failures. The daemon sends no notification
for it: the report wakes no model and, like any message, the post marks
the agent unread and raises `agent:message`. A fork the cue woke that
fails posts its own `error` under what woke it.

**Spend limits.** Every kind of agent work but the foreground turn is
held to the spend limits: its `Session` is built `capped` with a
`SpendCap`, and before each model call `SpendLimits::reached` reads
`ListIntelligenceSpend`. A reached limit ends the answer with
`IntelligenceError::SpendLimitReached`, and a cue's fork checks before it
starts, so a cue firing every minute records no run and opens no thread
while it is held back. All spending counts toward a limit, the user's
own turns and scripts' model calls included, but only forks
stop; the user's messages still get answers. The first time a period
holds an agent back, `SpendLimits` reports it with
`ReportAgentErrorCommand`, a background `error` message in its feed
(`The daily spend limit of $5.00 is reached ($5.12 in the last 24
hours). …`) and no notification, once per agent and period, kept in memory: once a check
finds the period back under its limit, the next time it holds the agent
back posts again (as does a restart). A run that a limit ends mid-way
is recorded as failed and posts nothing of its own; a fork that owed a
reply answers its asker with why.

## Activities

An `Activity` is long work the user follows from one place: a title, an
`ActivityKind` (what it is, and where a client jumps to show it: today
only an actor's `ActorSession { actor_session_id }`), an
`ActivityStatus` and a progress line. The
work is owned elsewhere; the activity only shows it. Work opts in by running
through `ActivityTracker::start(title, kind, |activity| work)`
(`cohesion-db/src/activity_tracker.rs`), which does the paperwork: it
records the activity, counts it in the `WorkTracker` from the call, hands
`work` an `ActivityHandle` to report through, records it succeeded or
failed by what `work` returns, and cancels it, dropping `work`, once the
tracker closes. The caller holds the future. The tracker's commands are
crate-private, so it is the only writer of activities and the only source
of `Invalidation::Activities`. `ListActivitiesCommand` lists the running
and waiting ones, then the most recent that ended. A new kind is a variant
of `ActivityKind`, a column, and a case in the `activities` triggers, which
hold the table's rules so that kind can be added without rebuilding it.

## Actors

An `Actor` is an agent on a client's device that Cohesion hands actor sessions to,
such as Claude Code in one directory: a client lists its actors in its
`Hello` (`ClientCapabilities.actors`, each an identifier, a product, a
scope, and the modes and models (with their efforts) it offers), and
`SyncCapabilitiesCommand` keeps their rows (`actors`) in step, soft-deleting
one no longer offered so its actor sessions stay. An actor has no name of its own:
it reads as "<product> on <client name>", joined from `clients`.

An `ActorSession` is one thread in the actor's own system (a Claude Code
session); its transcript is `actor_session_messages`, each an
`ActorSessionMessageContent` tagged by `sender`: the actor's text, tool uses
and requests (questions, permission to run a command, change files or use
a tool, a plan to approve), or the user's text and responses. A response
names its request, is checked against its kind and shape by
`AddActorSessionMessageCommand`, and a unique index answers each request once. Images the user
attaches to a message, and those an actor shows with one, are kept beside
it (`attachments`, which `attachment_count` counts), as an agent's
are, and pass the same check (`commands/image_upload.rs`); they cross the
session both ways as `ClientImage`s, in the calls and in a `message`
update's `images`.

`ActorSessions` (`cohesionkit/src/actor_sessions.rs`) carries the requests
through: `StartActorSession`, `SendActorSessionMessage`, `StopActorSession` and
`SetActorSessionMode` (`Local`) record what the user said or picked and call
the client (`ClientCall::StartActorSession`, `SendActorSessionMessage`,
`StopActorSession`, `SetActorSessionMode`, each answered `Accepted`; refused at
once when the client is offline, which leaves what was recorded undone). The
client reports back in `ActorSessionUpdate` frames, taken in order and each
answered by its `id`: a message (answered with the stored row, whose id a
response, or an end, names), a status, progress, the end of a tool or
of thinking (`EndActorSessionMessageCommand`, which stamps `ended_at` on a
`toolUse`'s or `thinking`'s content rather than adding to the
transcript), or a title. An actor session is
titled by the `title` it is started with or else by its prompt's first
line, and `SetActorSessionTitleCommand` takes a client's own (one it
generated, say), which `ActorSessions` also gives the open stretch's
activity; `ClientCall::StartActorSession` carries the title only when one
was given, so a client knows whether to generate its own. An actor session's `status` is stored
(`actor_sessions.status`), and is one of five: `working`, `needsAttention`,
`idle` (done, until the user says more), `failed`, and `resolved`, which
only the user, or an agent for them, sets (`ResolveActorSession`,
`session.resolve()`): actors report the other four
(`ReportedActorSessionStatus`), and a move to rest (`ActorSessionMove::keeps_resolved`)
leaves a resolved actor session resolved, so an actor winding down after it was
resolved doesn't bring it back. `ListActorSessions` leaves resolved actor sessions out;
`GetActorSession` and search still find them. No actor session ends for good: a
message to one that is idle, failed or resolved sets it working again,
`StopActorSession` leaves it idle, and `ResolveActorSession` stops one that is
going on first. While an actor session is working or needs
attention, `ActorSessions` holds a stretch of work for it, an activity of
kind `ActorSession` run through `ActivityTracker` (running, or paused while
it needs attention), whose future waits for the actor session to go idle
(succeeded), fail (failed) or be stopped (cancelled); the activity only
shows the status, which never reads from it. A stretch's activity ending
succeeded or failed marks the actor session unread (`actor_sessions.unread`, a trigger
on `activities`), since only its actor finishing it ends it so;
`MarkActorSessionRead` clears it. `ShutdownCleanupCommand`
leaves working actor sessions, and those needing attention, idle, and a client
whose last session closes does the same to its actor sessions. A status that
`SetActorSessionStatusCommand` changes raises `ActorSessionMoved`, the
`actor:moved` hook, with the actor session as written, which is how an agent's
fork hears of an actor session it follows, and how a cue or an extension watches
any actor session.

An actor session's context window is a `context` report, which
`SetActorSessionContextCommand` keeps in the actor session's `context_*` columns and
raises only `ActorSession(id)` for, as it changes every step and no list
shows it. How much of its plan's limits an actor has used (`ActorUsage`)
is the one thing about actors the daemon keeps only in memory. The client reports it in `ActorUsage` frames, each
replacing the last for that actor, and `CapabilityClients` keeps it on the
session's registration, so it goes with the session (and an actor's goes
when a `Hello` stops offering it) without any cleanup of its own. A
report may name a `pool`, the account it draws on: `CapabilityClients`
groups the reports by pool id across every client (an actor naming none
is a pool of its own) and lets the latest in each stand for it, for both
`ListActorUsage` and `actor.usageLimits()`, so no pool state outlives
its members. `ListActorUsage` (`Local`) lists one entry a pool, naming its
actors by Cohesion's actor ids, and a report,
or a session that served a client ending, raises `ActorUsage`.

Scripts reach `ActorSessions` through the `actors` global (`Mount::Actors`):
an agent's conversation and its jobs mount it, cues don't, and an extension does with the `actors` permission. It reads
actors, actor sessions and transcripts from the database, and starts, replies in
and stops actor sessions through the `ActorAccess` trait cohesion-js declares,
which `ScriptActors` (`cohesionkit/src/script_actors.rs`) implements by
running `ActorSessions` on the runtime. The call runs as an actor session of its own,
so a script whose time runs out stops waiting without cutting a start
short. Every sandbox but the isolated worker's carries the access, as it
carries services (`Hosts`); the mount decides who may use it.

Nothing waits on an actor session inside a turn: a conversation dispatches and ends
its turn, and hears through its follow. See
[actors.md](actors.md) for implementing an actor on a client. `cargo run
-p cohesiond --example fake_actor -- <socket>` is a fake actor that steps
an actor session through every state.

## Capabilities

Any client may also offer capabilities: on its session it offers the
actors on its device (see Actors) and answers the daemon's calls to them,
while it shows Cohesion to a person or not. Clients have no kind; one that
offers nothing is simply never called.

**Clients and identity.** Every session belongs to a client its transport
identified before the session began; a `Client` row records how
(`key_kind`) and the fingerprint that does it (`key`, unique among live
clients). A paired client is identified by its certificate
(`key_kind = certificate`), registered once at `/register` with a setup
key and checked by `PeerAcceptor` on every connection. A setup key exists
only during a pairing window: `Secrets` (`cohesion-db/src/secrets.rs`)
holds the current key, which dies after one registration or ten minutes.
`BeginPairing` (`FromClient`, from any paired client or the host's CLI,
or only from a local client, and only from where new clients may pair, as
the security settings say) opens a window, and the daemon opens the first itself when no client exists yet,
printing its code to stdout, never the log. The code is
`base64("key|address|fingerprint")`; `address` names a `Host`, an ip or a DNS
name (`--host`), as `host:port` (`[v6]:port`), with the port clients connect
on (`--public-port`, else `--port`). A paired CLI keeps the port
beside the host in its configuration. `/register` checks the key
before any database work and answers every failure with the same 401;
`PeerAcceptor` remembers recently unknown fingerprints briefly
(`UnknownPeers`) so a stranger reconnecting costs no database job, and a
registration makes its certificate forgotten. The Unix socket has
no TLS handshake, so it has its own (`cohesion-core/src/auth/local_handshake.rs`):
the client's first line is a `LocalIdentity { key, name }` and the daemon
answers one `LocalHandshakeReply` line, `accepted` or `refused` with why.
`local_acceptor` reads and answers it before the connection becomes a
session, and `IdentifyLocalClientCommand` resolves it: the client whose
`key_kind = local` key is the SHA-256 of `key`, created under `name` the
first time the key is seen (the socket's file permissions trust whoever
connects) and refused once that client was deleted. `cohesion setup
--local` makes the key and keeps it in the configuration, as a paired
configuration keeps its certificate. A client's name is only how it is
listed: `RenameClient` renames any client, and `Hello` carries the name so
a client knows what Cohesion calls it. A client row also records when it
paired (`created_at`) and when and from where it last connected
(`last_seen_at`, `last_address`, `None` on the socket), which the
transports note through `Security::seen` without a write while the address
is unchanged and the last note is under a minute old.

**Security.** The daemon decides nothing by address: behind a router that
forwards the port and loops LAN traffic back through itself, every client
arrives from the router's address, so an address can't tell clients
apart. An address is recorded only to be shown. The security settings'
one row (`security_settings`) says which clients may open pairing (any,
or only local ones, `BeginPairing` above), whether a new client sends a
notification, and after how many days without connecting a client is
revoked, which the hourly cleanup does, never to a client connected now;
everything defaults to what the daemon did before the settings existed.
The `security_events` table records pairing, renames, revocations,
settings changes and refused pairings, which `CohesionContext.security`
(`cohesionkit/src/security.rs`) records at most once an hour per address,
and the hourly cleanup forgets them after a fixed 30 days
(`DeleteOldSecurityEventsCommand`).

**Transport.** A client offering capabilities uses the ordinary session and adds its
own frames (`cohesion-core/src/protocol/envelope.rs`, payloads in `capabilities.rs`).
`ClientFrame` is what a client sends: a `Request`, told apart by its
command's `type` as before, a `Subscribe { id }`, a `Hello { id, actors }`
(`ClientHello`), a `CallResult { call_id, ok | error }`, and the actor
updates (`actorSessionUpdate`, `actorUsage`; see Actors). `Hello` carries
`ClientCapabilities`: `actors`, identifiers unique among them. A client
that serves something sends it first, and again when what it offers
changes; one that serves nothing never sends one, so the CLI, the web UI
and the Mac WebUI harness, which share a client, don't take each other's place.
It is answered in order like a request, with a `Reply` carrying
its `id`: `accepted`, or an error (malformed offers, say). Saying hello again on the same session replaces only the
offers, and keeps the calls waiting on it. The daemon calls a client with
`ServerMessage::Call { call_id, call: ClientCall }`. `ClientCall` is an
enum of the actor calls (`StartActorSession`, `SendActorSessionMessage`,
`StopActorSession`, `SetActorSessionMode`), each answered with
`ClientResult::Accepted` or an error. When the daemon ends a client session it sends a last
`ServerMessage::Evicted { reason }`: `replaced`, another session sent a `Hello`
for the same client, or `revoked`, the client was deleted. A client must
not reconnect on its own after either: a replaced one would only take its
place back from the other, and a revoked one has to be paired again. A
dropped connection with no `Evicted` frame is the one to reconnect after.
`Call` and a `replaced` `Evicted` go only to a session that sent a `Hello`
(a `revoked` one goes to every session of the client), and invalidations
only to one that subscribed, so a client that only offers hears nothing
else.

**The session loop.** `Session` splits its connection (`Exchange` into an
`ExchangeReader` and an `ExchangeWriter`) and runs in three tasks. The
reader completes a `CallResult` at once, handles a `Subscribe` or a
`Hello`, and queues each request with their replies in their place; the handler runs
the queued requests side by side (up to 64) and replies to each as it
finishes, so a long command holds up nothing behind it and replies come
back in any order, matched by id; the writer drains one outbound queue (replies and calls)
together with the invalidations once subscribed, numbering every frame. So
a session reads, and is told about changes, while one of its commands runs, and a
write waiting on a full socket never stops it reading the answer to a
call. The session ends when the client hangs up, a write fails, the
handler stops (a command panicked), the registry evicts it, or its client
is revoked; an evicted session writes what it had queued, then the `Evicted` frame. Over a
WebSocket (`Exchange::KEEPALIVE`) the writer also pings every 30 seconds,
and a session that hears nothing, frame or pong, for 60 ends as
dropped rather than evicted, so a client whose connection went half-open
goes offline, and a client, such as a sleeping Mac, reconnects instead of
leaving each call to wait out its deadline. The Unix socket reports a
hang-up itself and never pings.

**Presence.** Every session, whether or not it offers, joins
`CohesionContext.presence` (`cohesionkit/src/presence.rs`) for its
client when it opens and leaves when it stops reading, so a client is
online while any session with it is open. This is the one place presence
lives: `ListClients` (via `FromClient`) lists each client with `online`, and
an actor names its client rather than repeating the flag, so every UI
looks the client up. A client's first
session and its last one's close publish `Clients`. Presence also ends a
revoked client's sessions: every revoke path (`DeleteClient`,
`RevokeOtherClients`, the hourly cleanup) calls `presence.revoke` once the
revocation is written. The revoke itself (`revoke` in
`cohesion-db/src/commands/client.rs`, which all three share) raises
`DatabaseEventKind::ClientRevoked`, the `client:revoke` hook, with the
client's id and name. Each of the client's sessions, however it
connected and whether or not it offers, ends at once as evicted
(`revoked`), dropping its registration with it; one that opens later over
a connection accepted before ends as soon as it opens. `handle_request`
refuses anything a revoked client sends (`unauthorized`), so a request its
session read before ending, or an HTTPS one-off over a kept-alive
connection, is not carried out.

**Registry.** `CohesionContext.capability_clients` is `CapabilityClients`
(`cohesionkit/src/capability_clients.rs`), a cheap clone mapping each connected
client to its session's outbound queue, the `ClientCapabilities` it offers,
its actors' usage and the calls waiting on it; it is the only
writer of a client's actor rows. `register` takes the client's slot
first, then runs `SyncCapabilitiesCommand` through `CapabilityStore` (the
database in the daemon, a fake in the registry's tests), and gives the
slot up if that fails; a sync runs only if its session still holds the
slot, one at a time, so the rows are what the session serving the client
last offered however two racing sessions interleave. It returns the
`ClientSessionHandle` the session keeps: the session answers its own calls
and replaces its offers through it (`offer`, which syncs the same way),
and dropping it, when the session closes, unregisters the client and fails
its pending calls as offline (`ClientCallError::Offline`). A second session
offering for the same client evicts the first as replaced, failing its
calls, so a client whose connection dropped reconnects without waiting for
the stale session to time out; a revoked client's session ends through
presence (above). `call` takes a client id and an `ActorCall` (the actor
and its `ClientCall`); it sends one `Call` and awaits the answer for up to
`ActorCall::DEADLINE`, and stops waiting however it ends, dropped
included. A call for a client no session serves fails as offline at once;
for a connected client that no longer offers the actor, as `NotOffered`.
Registering publishes nothing itself: presence says who is online, and
the sync's command raises `Actors` when the rows change. Lower crates
never see a connection.

**Connectors.** A connector is one definition, a `Connector` row
(`connectors`: `identifier`, unique ignoring case, `version`, `title`,
`icon`, `unit`, `metadata_keys`) and its `ResourceKind` rows
(`resource_kinds`: `identifier`, `title`, `plural`, `contents`), made by
the extension serving it from its identifier, manifest version and icon
and what its `serviceDescription()` returns. The first extension to serve
a connector creates the row and a later one with a higher `version`
rewrites it (`upsert_connector`, `cohesion-db/src/commands/connector.rs`,
one upsert with `when_greater("version")`, whose write replaces the kinds,
keeping each kept kind's id; a kind it leaves out goes, and its resources
cascade with it); installing an `oauth` extension reads
`serviceDescription()` in the isolated sandbox beside the manifest and
stores it in the same `InstallExtensionCommand`, and signing in stores it
again. A `ConnectorAccount` has a `connector_id`, a `description` and an
`AccountHost`: an `Extension` with the account's identifier and
credential, or `Orphaned`, which the table's checks keep to a credential
only with an extension. Uninstalling an extension
(`extensions_orphan_accounts`) orphans its accounts, clearing the
credential and keeping the description and the resources; an orphan is
listed, can't be browsed or refreshed ("Ada is no longer connected; move
its resources to another account."), and can be removed.
`MigrateResources` moves every resource of one account to another; the
`resources_same_connector` trigger refuses an account of another
connector, and an item the target already holds in a workspace keeps the
target's copy.

In `connector_calls.rs` an account's items are reached through a
`Connection`: an extension's with its fresh credential, and an orphan's
not at all. `ListConnectorOrganizationUnits` and
`SearchConnectorResources` (and an agent's `organizationUnits()` and
`items()`) are live calls on `getOrganizationUnits` and `listResources`;
an import or refresh calls `getResource`. A preview and an item name their
kind by its key: a search lists one kind and fails on a key the connector
doesn't declare, and an import or refresh of such an item is refused
(`declared_kind`). A resource points at its kind (`kind_id`); reads
compose `<connector identifier>.<kind>` (`Resource::KIND_SQL`). A
workspace shows a kind only once it is turned on there
(`workspace_resource_kinds`, `SetResourceKindShown`), last among the kinds
it shows until `ReorderWorkspaceResourceKinds` moves it; the web sidebar
lists the shown kinds in that order by `plural`, each a page of its own.

## Shutdown

`CohesionContext.work` is a `WorkTracker` (`cohesion-db/src/work_tracker.rs`)
counting the work in flight under a label each: a client's request
(`handle_request`, and `/register`), every queued hook until it has run
(`HookManager::queue`), every agent turn and job (taken in
`AgentCoordinator::fire` before the job is spawned) and a resource refresh
pass. The database worker hands a command's events to `HookManager` before
it replies, so the hooks a request raises are counted before the request
ends.

`PeriodicCleanupCommand` (`cohesionkit/src/periodic_cleanup.rs`) runs at
start and every hour after as one more timed pass: it removes what has
outlived its use, in one transaction. How long most are kept is the
storage settings' singleton row (`StorageSettings`, `GetStorageSettings`
and `UpdateStorageSettings`, raising `storage`; Settings › Storage on the
web, beside the file's size from `GetDatabaseSize`), read once per pass
and handed to each cleanup command as `days`: idle model threads
(`thread_retention_days`, 14 by default, since their last message), the
contents of thread messages older than that same retention, forks' included
(`BlankOldThreadMessagesCommand` empties the text, tool arguments and
results and thoughts but keeps the row and the call identifiers, so a
request still pairs with its response; a blank row is skipped), and script
results in the agent chat older than it too (`BlankOldCodeResultsCommand`,
which keeps the script and how it ended), actor sessions resolved at least
`actor_session_retention_days` (7) ago, and no other (`DeleteOldActorSessionsCommand`, through `DeleteActorSessionCommand`,
so their messages, images, activities and follower cascade), security
activity older than a fixed 30 days, Rewind's
versions last changed more than `rewind_retention_days` (7) ago
(`DeleteOldVersionsCommand`, which no request reaches), and
tasks and documents deleted longer ago than `deleted_item_retention_days` (30) or
Rewind's retention, whichever is longer (`PurgeDeletedItemsCommand`; `deleted_at`,
which a trigger keeps from `deleted`, starts the clock). A changed setting takes
effect at the next pass. The rest of the agent chat and its images are never
trimmed. Then the pass
vacuums: `DatabaseContext::vacuum_if_reclaimable` is a job of its own on
the database worker, outside any command's savepoint (`VACUUM` cannot run
in one), and runs `VACUUM` only when the free pages are at least 8 MB and a
quarter of the file (`worth_vacuuming`), since it rewrites the whole file
while every other command waits and SQLite reuses free pages anyway. The
pass then revokes the clients
that stopped connecting (which needs presence, so it is a command of its
own), so a new cleanup of that kind is one more `ctx.run` in it, as one a
stopped daemon must not leave behind is in `ShutdownCleanupCommand`.

The file is in WAL mode, with SQLite's default `synchronous = FULL`, so a
copy of a live database goes through SQLite, never `cp` alone. `BackupDatabase`
(Settings › Storage, or `cohesion api`) has the worker `VACUUM INTO` a private
file in a `backups` directory beside the database, then empties the tables a
backup leaves out, away from the worker. Every model says which it is in its
`#[model(table = ..., backup = Included | LeftOut)]`, which doesn't compile
without it (`BackupClass`, `cohesion-db/src/backup.rs`). Only images are
left out (`attachments`, one table of bytes for both kinds of conversation, whose
foreign keys cascade with the message, and `intelligence_thread_images`), since
they don't compress and nothing depends on them. A backup is
written on request; nothing schedules one or removes old ones.

Work that ends where it began is scoped with `WorkTracker::run_new`, which
holds the guard exactly across one future (`WorkTracker::every` runs each
timed pass that way); work that must carry its guard elsewhere, through a
channel or into a spawned task, takes `begin` or `begin_new`.

On `SIGINT` or `SIGTERM`, or stdin closing under `--exit-with-parent`,
cohesiond (`cohesiond/src/shutdown.rs`) runs `CohesionContext::shutdown`: it
closes the tracker, so `run_new` and `begin_new` refuse new work (every client request
on any transport gets a "shutting down" error, the day clock and the resource
refresh stop), while `begin` still admits what running work starts (hooks,
cue runs, scripts, model calls). Settled hooks still waiting are queued at
once (`HookManager::settle_all`). Once the tracker is idle the database
closes: `DatabaseContext::close` runs the commands already sent, then
`ShutdownCleanupCommand` as the last write, so nothing in flight can undo
it (today it settles agents left `working`, fails runs left `running` and
cancels activities that never ended). Closing the tracker also cancels
the activities still running, so their work stops rather than holds the
shutdown.
Every five seconds the daemon logs what it is still waiting for; a second
signal, or five minutes passing, gives up on the work but still closes the
database, then exits with status 1. The daemon's start runs the same
cleanup first, for a daemon that stopped without closing (a crash,
`kill -9`), so a new cleanup is one `ctx.run` in that command and covers
both.

## Logging

Logging is stateless and global: a line carries only its `Module`, so any code
can log without a handle. Transports put the caller in the message text.
`cohesiond/src/main.rs` installs `ConsoleAdapter` at `--log-level`; other
sinks implement `cohesion_core::log::Adapter` and are chained with
`.with(adapter)` on the `Logger`.

## Referring to items

A table that belongs to one of several kinds of thing names it with one nullable
foreign key per kind, read as a `ColumnGroup` enum, never a kind and an id: rows
cascade with what they belong to, and a `CHECK` holds them to one. `versions`
(`VersionedItem`), `attachments` (`AttachmentMessage`), `metadata`
(`MetadataOwner`, `HasMetadata`) and `storage_entries` (`StorageOwner`) do this.
Where a thing must also share a workspace, the foreign key is composite, so the
schema refuses a mismatch: `versions`, `task_tags` and `saved_task_view_tags` name
`(workspace_id, ...)`. `search_entries` is
the one left as a kind and an id, since every search reads it as one list. A
generic table every item hangs off isn't used: it would add a row to every insert
and these tables would still need to know each kind.

## Rewind

Rewind keeps earlier copies of what the user wrote, in one table (`versions`,
`Version`): a task's title, description and status, or a document's or journal
entry's title and contents. A version names its item through `VersionedItem`,
one real foreign key for each kind, so its rows cascade with the item and the
table's checks hold it to one item, and to a status only for a task. A new kind
of item is a column, a variant and two triggers. Only text and text enums are
kept, so a restore can never point at something gone, such as a deleted folder.

Triggers on `tasks` and `documents` keep the versions, so no write path can
miss one (each writes through the `version_writes` view, whose `INSTEAD OF`
trigger holds the rules below once), and every version-making write already raises `taskList` or
`documents`, which is what a client refetches a history on. A trigger can't
see the command, so one more mechanism is needed for attribution:
`rewind_writer`, a one-row table the database worker sets from the command's
trace (`Worker::run`, inside the command's savepoint, only when the writer
changes and only for `Write<UserData>`), read through the `rewind_author`
view. An author is the user, an agent, an extension, or a script run by hand
(`DatabaseEventTrace::writer`); an agent's or extension's name is copied onto
the version, since either can be removed.

Edits keep versions at least two minutes apart: an edit within two minutes of
the latest version's `created` joins it, whoever makes it, and takes over its
author and `modified`, so a version says who edited it last. A deletion or a
restore always keeps a version of its own, however recent the one before it,
so those can land closer than two minutes; an edit joining a `restored` version
makes it `edited`, and an `earlier` or `deleted` version is never joined. Only
the latest version ever changes, so once a restore has kept its version, what it
replaced can't change. `rewind/tests.rs` pins this. The first change to a row with no versions (older
than the feature, or expired) first keeps what the row held as an `earlier`
version, so nothing is backfilled and nothing unchanged is stored twice.

`RestoreVersionCommand` makes the one
write that puts the fields back, with `rewind_writer.restoring` set so the
trigger keeps a `restored` version, and raise the same events editing would
(a task coming back raises `TaskCreated`), so hooks and two-way extensions
keep up. A task whose folder has since been deleted comes back to the inbox;
its other fields are untouched. The partial unique index
`documents_live_journal` holds one live journal a day, which is what refuses
a deleted entry whose day has a new one. Clients and scripts read versions
and restore from them; no request or script changes or removes one, so the
hourly cleanup is the only way one goes. Scripts reach them behind
`cohesion:read` and restore behind `cohesion:write`, like any other read and
write of user data.

## The web UI

`web/` is a SvelteKit single-page app (`web/README.md` has its layout,
styling and Copy rules; `web/src/lib/stores/README.md` its store pattern).
It holds no state of its own: each store fetches through the protocol and
refetches when an `invalidate` it `matches` arrives, so what the page shows
is always what the daemon last said.

The daemon carries the UI it was built with. `npm run build` writes
`web/build/`; `cohesionkit/build.rs` compresses it file by file into the
binary (`web_bundle.rs`), so build the web UI before cohesiond. `hello`
carries the bundle's hash, and a client fetches it with `WebBundle` when
its copy differs. `cohesion open` serves it from memory on loopback; the
WebUI harnesses serve it through CohesionWebKit. Either way the page
speaks only its session API (`#token=`, `/client`, `/connect`), which is
why that API changes in all three places at once
(`docs/serving-the-webui.md`). `VITE_MOCK=1 npm run dev` swaps the
transport for an in-memory daemon (`web/src/lib/protocol/mock.ts`).

Items link to each other with one scheme, written as Markdown links in
messages, documents and task descriptions:
`cohesion://task/<workspaceId>/<id>`, `cohesion://document/<workspaceId>/<id>`
(a journal entry is a document), `cohesion://agent/<id>` and
`cohesion://actor-session/<id>` (singular, lowercase and hyphenated, numeric
ids), such as `[Fix login](cohesion://task/3/42)`. Agents and actor sessions are
global, so their links name no workspace. Agents are told to write them
(`LINKS` in `cohesion-intelligence/src/system_prompt.rs`, and the `Task`,
`Document`, `Agent` and `ActorSession` script docs), and `LINKS` also tells them
to read the item a link in the user's message names with the code tool.
Forwarding an item to an agent sends one: `SendAgentMessage`'s `forward`
names the task or document, and the daemon stores a `forward` message
carrying the item, with its title as it then was, and the user's note
(`AgentForwardedItem`). The agent reads it as the note followed by
`Forwarded task [Fix login](cohesion://task/3/42).`
(`AgentForwardedItem::reference`), and the feed shows the item as a chip
that opens as the link would (in its panel over the page). The links agents write
are stored as text; each client opens them. An agent also draws a diagram as a fenced block with the info string `mermaid`; the web UI's agent chat, run transcripts and actor session transcripts (the actor's messages and plans; `Markdown` with `diagrams`) render it as SVG through `RenderMermaid` once its fence has closed, keeping each reply by source and width, inlined and coloured from the page's tokens (the `.diagram` class in `tokens.css` sets its variables), and a block that is refused, still streaming or waiting for its reply stays a code block. The web UI's `Markdown` rewrites each to an

The daemon draws them too: `RenderMermaid` (`cohesionkit/src/routes/mermaid_routes.rs`) takes a fence's source, the width to lay out for and a `format`, whose only case is `svg`, so a client needs no renderer of its own. It runs the same Pele (vendored in `cohesion-js/vendor/pele`, whose README holds the version a test checks it against) in a QuickJS runtime with nothing mounted, on two workers of the `ScriptEngine`'s (`engine/mermaid_pool.rs`), each keeping one renderer and starting another after a render that ran out of time or memory (`SandboxOptions::MERMAID`, 64 MB and 15 s). It draws on read and keeps nothing. A diagram that won't draw is a reply, `refused` with a `MermaidRefusal` and a message, and the client shows the fence as code; one waiting over 30 s is refused as `busy`, and a shutdown waits for none. The SVG has no background and takes its colours from the `--pele-*` CSS variables, with Pele's light fallbacks only for one a client doesn't set, so a theme change needs no request as long as the SVG is inline; colours a diagram's author wrote stay as written. It scales from its `viewBox`, so a resize needs no request; only a new width to lay out for does. The web UI calls it; Cohesion+ doesn't yet.
ordinary in-app route, `/open/[kind]/[ws]/[id]` for a task or document and
`/open/[kind]/[id]` for an agent or actor session, which finds the item
(`web/src/lib/shell/links.ts`) and replaces itself with the item's page, or
says it was not found; a task or document is looked for only in the
workspace its link names. A click on a link or a forwarded item's chip
opens the item over the current page instead (`linkedItem` in
`web/src/lib/shell/linkedItem.svelte.ts`), in one side panel
(`web/src/lib/ui/ItemPanel.svelte`) that is the same for every kind but its
content: its kind after the title (`Task #42`), then the item's details as rows of an icon, a label and a value (a
task's status, assignee, priority, size, days, folder and tags each change in place, the size on a slider and the assignee in a picker of the user, the agents and the workspace's contacts), then
its text (a task's description, still editable; a document's contents; an agent's directive;
the last thing an actor session's actor said). Documents, journal entries,
agents and actor sessions have pages of their own, so their panels add Open,
which goes there. The Open page is only reached by opening the link
elsewhere (a new tab, or with a modifier key), and goes to the item's page,
a task's in its list. Links first named no workspace
(`cohesion://task/<id>`, `cohesion://document/<id>`), and older messages and
documents still hold them, so those still open: the item is looked for in
the current workspace first, then the others. Any other `cohesion:` link,
such as one with an extra segment, keeps only its text.

## Other repos

Each client lives in its own repo under `github.com/cohesionhq`, and
cohesion is the one they all follow:

| Repo | What it is | How it connects |
|---|---|---|
| `apple` | Cohesion for iPhone and iPad, a native app with a share extension and a notification service. | Hand-written Swift models and commands: requests over `POST /command`, invalidations over a subscribed `/connect` WebSocket. |
| `webshell` | The WebUI harnesses, starting with the Mac app (`mac/`): pairing and setup are native, the interface is the web UI. Its CohesionWebKit package serves the web UI in a `WKWebView`. | Pairs at `/register` from a setup code, fetches `WebBundle`, serves it under a custom scheme, and stands in for `/connect` and `/client` over its own session. |
| `cohesion-ts` | A TypeScript client library for Bun. | The Unix socket or `/connect`; copies `generated.ts` and hand-writes the session frames (`hello` offers, calls, actor updates) from `envelope.rs` and `capabilities.rs`. |
| `actor-agentsdk` | Offers Claude Code to Cohesion as actors, one per directory, and runs the actor sessions it is handed through the Claude Agent SDK. | A cohesion-ts client offering actors (`docs/actors.md`); its sessions' transcripts become actor sessions. |
| `actor-opencode`, `actor-codex` | Offer OpenCode and Codex as actors the same way. | cohesion-ts clients offering actors. |
| `extensions` | Extensions published to the registry, such as the iPhone app's notification companion. | Installed into a daemon like any extension (`docs/extensions.md`). |
| `relay` | The service extensions call for OAuth sign-in and push notifications. | Never connects to the daemon; extensions reach it with `network`. |

Cohesion Cloud, the hosted service at hellocohesion.cloud, runs this
repository's `cohesiond` for people who'd rather not host it; its own code
isn't part of Cohesion.

A change to anything they read (a model's fields, a frame, the page's
session API) is a change to them too. Nothing here is kept for an older
client ("Compatibility" in `docs/product-direction.md`): a breaking change
leaves `PROTOCOL_VERSION` at 1, names the clients it breaks under
"Unreleased" in `CHANGELOG.md`, and they're updated after it.

### Push notifications

The daemon pushes nothing to devices itself, and has no cloud in it. It
raises hooks (`agent:message`, `notification:send`, the activity hooks,
`client:revoke`…), and an extension the user installs and grants turns
them into pushes. For the iPhone app that is the iOS companion
(`com.cohesion.cloud.ios-companion`, from the `extensions` repo):

- The app registers its device through `callExtension` on the companion's
  `service:isolated` service, so only clients reach it, not scripts or
  agents.
- On a hook, the companion calls the relay with `network`, signed in
  through its `oauth` account, and the relay passes the push or Live
  Activity update to APNs. Only what the push shows (a message's text,
  the agent's name and colour, an activity's progress) leaves the daemon.
- On `client:revoke` it forgets that client's registration.

The user installs the companion and turns on its permissions and hooks
like any extension's; the daemon treats it no differently. A feature that
should notify a device therefore raises a hook (or reuses one) and lets an
extension push; it never calls a push service from the daemon. The full
path is in `docs/ios-companion.md` in the `relay` repo.
