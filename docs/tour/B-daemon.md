# Part B: Inside the daemon

## B1. The parts of `cohesiond`, and the threads they run on

`CohesionContext` (in `cohesionkit`) is the bag of handles every request handler gets. Everything is a cheap clone around shared state. The one thing that is *not* async is the database: a single OS thread owns the SQLite connection.

```mermaid
flowchart TB
    subgraph TRANSPORTS["Transports (cohesiond/src/server)"]
        direction LR
        HTTPS["axum HTTPS :3000 by default (--port)<br/>/register  /command  /connect"]
        UNIX["Unix socket<br/>local_acceptor handshake"]
        PEER["PeerAcceptor<br/>looks up cert fingerprint<br/>UnknownPeers cache"]
    end

    SESSION["Session (per connection)<br/>reader task + handler task + writer task<br/>writer also pushes: Invalidate batches and actor Call frames"]

    subgraph CTX["CohesionContext (cohesionkit): the facade every route runs against"]
        direction TB
        ROUTES["Route impls<br/>database / script / intelligence / local"]
        DBCTX["DatabaseContext<br/>mpsc channel to ONE worker thread"]
        ENGINE["ScriptEngine<br/>QuickJS pools on dedicated threads"]
        HOOKS["HookManager<br/>ordered hook queue"]
        RECEIVER["HookReceiver<br/>serial consumer of the queue"]
        AGENTS["AgentCoordinator<br/>one AgentHost per agent"]
        EXTMGR["ExtensionManager<br/>scoped persistent workers"]
        CAPREG["CapabilityClients<br/>registry of connected clients"]
        CATALOG["ModelCatalog<br/>models.dev snapshot"]
        SECRETS["Secrets<br/>setup keys for pairing"]
        WORK["WorkTracker<br/>counts work in flight"]
    end

    subgraph WORKER["Database worker thread (std::thread, not tokio)"]
        direction TB
        CONT[("Container<br/>rusqlite connection<br/>cohesion.db")]
        INV["InvalidationContext"]
        LISTENER["DatabaseEventListener<br/>= HookManager"]
    end

    subgraph LOOPS["Background loops started at boot"]
        direction LR
        DAYCLOCK["day clock<br/>every 60 s"]
        RESOURCELOOP["resource refresh<br/>1 min after start, then 30 min"]
        FEEDLOOP["feed refresh<br/>hourly"]
        UPDATES["extension update check<br/>at start, daily"]
        LAUNCH["launch hooks<br/>per enabled extension"]
    end

    HTTPS --> PEER --> SESSION
    UNIX --> SESSION
    SESSION -- "Command::dispatch" --> ROUTES
    ROUTES --> DBCTX
    ROUTES --> ENGINE
    ROUTES --> AGENTS
    ROUTES --> EXTMGR
    ROUTES --> CAPREG
    DBCTX -- "Job (boxed closure)" --> WORKER
    LISTENER --> HOOKS --> RECEIVER
    RECEIVER --> EXTMGR
    RECEIVER --> AGENTS
    AGENTS --> ENGINE
    AGENTS --> DBCTX
    EXTMGR --> ENGINE
    ENGINE --> DBCTX
    DAYCLOCK --> HOOKS
    UPDATES --> HOOKS
    LAUNCH --> HOOKS
    RESOURCELOOP --> DBCTX
    FEEDLOOP --> DBCTX
    CATALOG --> DBCTX

    classDef t fill:#dbeafe,stroke:#2563eb,color:#111
    classDef c fill:#ede9fe,stroke:#7c3aed,color:#111
    classDef w fill:#fef3c7,stroke:#d97706,color:#111
    classDef l fill:#f3f4f6,stroke:#6b7280,color:#111
    class HTTPS,UNIX,PEER,SESSION t
    class ROUTES,DBCTX,ENGINE,HOOKS,RECEIVER,AGENTS,EXTMGR,CAPREG,CATALOG,SECRETS,WORK c
    class CONT,INV,LISTENER w
    class DAYCLOCK,RESOURCELOOP,FEEDLOOP,UPDATES,LAUNCH l
```

## B2. Crate layering (what may depend on what)

One-way dependencies. The CLI never links SQLite, QuickJS or the LLM code. A lower crate that needs something from a higher one declares a tiny trait (`ConnectorAccess`, `ServiceAccess`, `AccountCredentials`) and the higher crate implements it.

```mermaid
flowchart BT
    MACROS["cohesion-macros<br/>derives: Model, Commands, Replies,<br/>js_entity, TsWire ..."]
    CORE["cohesion-core<br/>models, wire protocol, Route,<br/>Caller, invalidations, auth, logging"]
    DB["cohesion-db<br/>Container, query builders, migrations,<br/>DatabaseContext, all commands,<br/>CredentialManager, Secrets, WorkTracker"]
    NET["cohesion-internet<br/>article_snapshot, RSS/Atom feed reader"]
    JS["cohesion-js<br/>ScriptEngine (QuickJS),<br/>globals + mounts, entities, .d.ts"]
    INTEL["cohesion-intelligence<br/>backends, ModelCatalog, Session,<br/>AgentCoordinator / AgentHost"]
    KIT["cohesionkit<br/>CohesionContext, Route impls,<br/>CapabilityClients, ExtensionManager,<br/>HookManager, connectors"]
    DAEMON["cohesiond<br/>HTTP / WS / Unix socket,<br/>Session, TLS, shutdown"]
    CLI["cohesion (CLI)<br/>commands, setup, cohesion open<br/>web proxy + embedded web UI"]
    WEBUI["web/ (SvelteKit)<br/>built, then embedded in the CLI"]

    MACROS --> CORE
    CORE --> DB
    CORE --> NET
    DB --> JS
    NET --> JS
    JS --> INTEL
    INTEL --> KIT
    KIT --> DAEMON
    CORE --> CLI
    WEBUI -. "rust-embed" .-> CLI

    DB -. "traits implemented higher up:<br/>ConnectorAccess, ServiceAccess,<br/>AccountCredentials" .-> KIT

    classDef base fill:#f3f4f6,stroke:#6b7280,color:#111
    classDef mid fill:#ede9fe,stroke:#7c3aed,color:#111
    classDef top fill:#dbeafe,stroke:#2563eb,color:#111
    class MACROS,CORE base
    class DB,NET,JS,INTEL mid
    class KIT,DAEMON,CLI,WEBUI top
```

## B3. Daemon startup, in order

```mermaid
flowchart TD
    A["parse args, read config.json<br/>--config --socket --host --port --public-port --log-level"] --> B["install ConsoleAdapter logger<br/>install signal handlers"]
    B --> C["open cohesion.db<br/>run MIGRATIONS (append-only)"]
    C --> D["load or create server key<br/>derive self-signed cert + fingerprint"]
    D --> E["CohesionContext::new<br/>DB worker thread, ScriptEngine,<br/>HookManager, ExtensionManager,<br/>AgentCoordinator, CapabilityClients"]
    E --> F["CohesionContext::start: settle agents<br/>any agent left 'working' by a crash<br/>is reset to waiting or idle"]
    F --> G["refresh the model catalog<br/>fetch models.dev in the background"]
    G --> H["queue onLaunch for each extension<br/>with the hook on"]
    H --> I["start the day clock<br/>60 s wall-clock reader"]
    I --> J["start the resource, feed<br/>and update-check passes"]
    J --> K{"any clients<br/>registered yet?"}
    K -- "no" --> L["open a pairing window<br/>print setup code to STDOUT<br/>(never the log)"]
    K -- "yes" --> M
    L --> M["serve: HTTPS :3000 by default (--port) AND Unix socket<br/>(select: if either dies, daemon stops)"]
    M --> N["wait for SIGINT / SIGTERM<br/>or stdin close (--exit-with-parent)"]
    N --> O["shutdown::run<br/>(see Part J)"]
```

## B4. One request, end to end (the canonical request flow)

Example: you tick a task as done in the web UI. The same path serves every command from every client.

```mermaid
sequenceDiagram
    autonumber
    participant UI as Client (web UI / CLI / iOS)
    participant TR as Transport (TLS or Unix socket)
    participant SE as Session (reader / handler / writer)
    participant RH as request_handler
    participant CT as CohesionContext (Route)
    participant DC as DatabaseContext
    participant W as DB worker thread
    participant HM as HookManager

    UI->>TR: frame { id, type: "setTaskStatus", args }
    Note over TR: identity was fixed BEFORE the session:<br/>cert fingerprint (TLS) or local key hash (socket)
    TR->>SE: Session::new(conn, context, Caller, Client)
    SE->>SE: reader parses ClientFrame::Request, pushes to handler queue
    SE->>RH: handle_request (side by side, each replies when done)
    RH->>RH: WorkTracker.run_new (refuses if shutting down)
    RH->>CT: Command::dispatch -> Route#lt;Database, Req#gt;
    CT->>DC: send(Command::from(request))
    DC->>W: Job (boxed closure over mpsc)
    W->>W: SAVEPOINT cmd
    W->>W: cmd.handle(ctx): SQL + raise invalidations + raise events
    alt command ok
        W->>W: RELEASE cmd
        W-->>SE: broadcast InvalidationBatch (to subscribed sessions)
        W->>HM: DatabaseEvent (TaskStatusChanged ...)
        HM->>HM: queue as hook event (counts as work)
    else command failed
        W->>W: ROLLBACK TO cmd, clear invalidations
    end
    W-->>DC: result (oneshot)
    DC-->>CT: reply value
    CT-->>RH: Into#lt;ReplyData#gt;
    RH-->>SE: Reply { id, Ok(data) | Error }
    SE-->>UI: writer sends numbered frame
    Note over SE,UI: Subscribed clients ALSO get ServerMessage::Invalidate,<br/>so every other open screen refetches
```

## B5. Which kind of Route handles a command?

`#[derive(Commands)]` on `Command` generates `dispatch`. Each variant says which kind of route it uses; the default is `Database`.

```mermaid
flowchart TD
    CMD["Command variant arrives<br/>(type = camelCase name)"] --> KIND{"variant's via = ?"}

    KIND -- "Database (default)" --> DBR["one blanket impl<br/>Route&lt;Database, R: DbRequest&gt;<br/>= R::Command::from(request)<br/>then DatabaseContext.send"]
    KIND -- "Script" --> SCR["script_routes.rs<br/>RunScript, Documentation"]
    KIND -- "Intelligence" --> INT["catalog_routes.rs<br/>UpdateModelCatalog<br/><br/>agent_routes.rs<br/>SendAgentMessage,<br/>RespondToChoice, SetReaction,<br/>Update/DeleteAgent (evict host),<br/>RemoveAgentCue"]
    KIND -- "Local" --> LOC["client_routes.rs<br/>About, DeleteClient, BeginPairing,<br/>ListClients + 'online' (Presence)<br/><br/>extension_routes.rs<br/>install / grants / registry / CallExtension<br/><br/>connector_routes.rs<br/>OAuth, ImportResource, RefreshResources<br/><br/>article_routes.rs / feed_routes.rs<br/>fetch the web, then one DB command"]

    DBR --> W[("DB worker thread")]
    SCR --> ENG["ScriptEngine<br/>general pool, read-write"]
    INT --> COORD["AgentCoordinator<br/>or DB (plain agent CRUD)"]
    LOC --> MIX["CapabilityClients / ExtensionManager /<br/>ConnectorCalls / cohesion-internet"]

    classDef k fill:#ede9fe,stroke:#7c3aed,color:#111
    class DBR,SCR,INT,LOC k
```

## B6. Inside the database worker: why every command is atomic and observable

```mermaid
flowchart TD
    J["Job arrives from DatabaseContext<br/>(sent with a DatabaseEventTrace)"] --> SP["SAVEPOINT cmd"]
    SP --> VIEW{"cmd.Access ?"}
    VIEW -- "Read" --> RV["ctx.db = ReadContainer<br/>(only ReadableDatabase reads; cannot write)"]
    VIEW -- "Write" --> WV["ctx.db = WriteContainer"]
    RV --> RUN["cmd.handle(ctx)"]
    WV --> RUN
    RUN --> OK{"Ok?"}
    OK -- "yes" --> REL["RELEASE cmd"]
    OK -- "no" --> RB["ROLLBACK TO cmd<br/>drop pending invalidations<br/>(events were never published)"]
    REL --> PUB["publish(events)"]
    PUB --> P1["1. invalidations.take()<br/>-> broadcast InvalidationBatch<br/>(keys de-duplicated in a HashSet)"]
    PUB --> P2["2. each DatabaseEvent -> listener.send<br/>(HookManager) BEFORE the reply is sent,<br/>so hooks are counted as work before<br/>the caller hears 'done'"]
    P1 --> REPLY["reply on oneshot"]
    P2 --> REPLY
    RB --> REPLY

    GUARD["Scripts reach the worker through<br/>GuardedDatabaseContext:<br/>read gate accepts only Read commands<br/>write gate accepts only Write commands<br/>and throws JS Error when sandbox is read-only"] -. "same worker" .-> J
```

## B7. Live updates: how every screen stays in sync

The server never pushes data, only *what is now stale*. Clients refetch what they are showing.

```mermaid
sequenceDiagram
    autonumber
    participant A as Screen A (web UI)
    participant B as Screen B (iOS app)
    participant S as Daemon
    participant W as DB worker

    A->>S: ClientFrame::Subscribe { id }
    S-->>A: Reply accepted
    B->>S: ClientFrame::Subscribe { id }
    S-->>B: Reply accepted
    Note over A,B: Web and iOS subscribe FIRST on each connection,<br/>before loading anything. The CLI subscribes only<br/>while following an agent's turn.
    A->>S: renameTask(...)
    S->>W: command runs
    W-->>S: broadcast InvalidationBatch [ TaskList{workspace, folder} ... ]
    S-->>A: Reply (updated Task)
    S-->>A: Invalidate { keys }
    S-->>B: Invalidate { keys }
    B->>S: refetch ONLY active resources whose key matches
    Note over B: web stores: Resource.matches(invalidation, key)<br/>iOS: Changes.swift
    S-->>B: fresh data
    Note over S,B: If a subscriber falls behind (broadcast lag),<br/>it gets ServerMessage::Reset and refetches everything it shows
```

### Invalidation keys (what "stale" can mean)

```mermaid
flowchart LR
    subgraph PERWS["Per workspace"]
        direction TB
        K1["Workspace(id)"]
        K2["Documents(ws)"]
        K3["TaskFolders(ws)"]
        K4["TaskList { ws, folder | inbox }"]
        K5["Resources(ws)"]
    end
    subgraph PERAGENT["Per agent"]
        K8["Agent(id)<br/>row, messages, cues, runs"]
    end
    subgraph GLOBAL["Global"]
        direction TB
        K9["Feeds"]
        K10["FeedArticles"]
        K11["ConnectorAccounts"]
        K12["Intelligence<br/>endpoints, models, tier defaults"]
        K13["IntelligenceCostEvents"]
        K14["Clients<br/>registered / deleted / connected"]
    end
```
