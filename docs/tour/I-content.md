# Part I: Content flows (documents, tasks, articles, feeds, search, scripts)

## I1. Following a feed and reading its articles

Feeds are shared by every workspace, and every article comes from a feed. Read state and text belong to the article itself. The Articles page reads them; Research searches them.

```mermaid
flowchart TD
    subgraph FEED["Follow a feed (shared by every workspace)"]
        direction TB
        F1["addFeed { url }"] --> F2["feed_routes.rs: read_feed (RSS / Atom, quick-xml)"]
        F2 --> F3["feeds row + its items as articles<br/>page text read as Markdown<br/>(Coherence, size + time bounded)"]
        LOOP["start_feed_refresh: reads EVERY followed feed hourly from daemon start<br/>+ refreshFeed / refreshFeeds on demand"] --> F2
        F3 --> F4["Invalidate Feeds + FeedArticles<br/>(global keys)"]
    end

    subgraph REFETCH["Fetch an article again"]
        direction TB
        RS["resnapshotArticle"] --> R2["article_routes.rs (Local route)"]
        R2 --> R3["blocking thread:<br/>cohesion_internet::article_snapshot::snapshot(url)"]
        R3 --> R4["ONE database command replaces the text"]
        R4 --> F4
    end

    ART[("articles")] --- F3
    ART --- R4
    classDef w fill:#f3f4f6,stroke:#6b7280,color:#111
    class R3,F2 w
```

## I2. Search

```mermaid
flowchart LR
    subgraph WRITE["Writes (documents, tasks, resources, thread messages)"]
        direction TB
        W1["INSERT / UPDATE / DELETE on the entity table"]
        W2["SQLite TRIGGERS keep search_entries<br/>(entity_kind, entity_id, workspace_id) in step<br/>+ the full-text index"]
        W1 --> W2
    end
    subgraph READ["Reads"]
        direction TB
        R1["cohesion search dentist<br/>web UI palette<br/>cohesion.search() in scripts<br/>workspace.search() for agents"]
        R2["One full-text query over everything searchable<br/>-> SearchResult { kind, id, title, snippet ... }"]
        R1 --> R2
    end
    W2 -.-> R2
    NOTE["Resources: Markdown and text contents are indexed by title + contents,<br/>JSON resources by title only. Their metadata lives in the metadata table."]
    R2 -.- NOTE
```

## I3. Running a script yourself (`cohesion run`)

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant CLI as cohesion run script.js
    participant D as cohesiond (Script route)
    participant SE as ScriptEngine (general pool)
    participant ST as swc_ts_fast_strip
    participant QJ as QuickJS runtime (fresh, own thread)
    participant G as GuardedDatabaseContext (read-write)
    participant DB as DB worker

    You->>CLI: cohesion run script.js
    CLI->>D: runScript { code }
    D->>SE: general-pool job
    SE->>ST: TypeScript? blank type annotations to whitespace (positions unchanged)<br/>enum / value namespace / parameter properties are REJECTED, not compiled
    SE->>QJ: evaluate with mounts: cohesion, intelligence, log, services, ArticleFetch
    QJ->>G: cohesion.workspaces()[0].createTask(...)
    G->>DB: Write command through the write gate
    DB-->>G: result (events + invalidations as usual)
    G-->>QJ: JS entity (Task) wrapping the model
    QJ-->>SE: last expression as JSON, or error with position
    SE-->>D: ScriptOutput
    D-->>CLI: printed result
    Note over QJ: ScriptDeadline bounds blocking calls (network.fetch, connectors, services).<br/>No leaked prototype changes between scripts: a fresh runtime per job.
    Note over DB: Writes raised by a script trigger hooks exactly like writes from a client.
```

## I4. How the script API reaches an LLM (one source of truth)

```mermaid
flowchart LR
    RS["Rust structs with #[js_entity]<br/>/// comments on fields and methods"] --> MAC["cohesion-macros expand to:<br/>- JS class (getters, methods, toJSON)<br/>- TypeScript declaration text"]
    MAC --> API["script_api! list in cohesion-js<br/>-> TS_DECLARATIONS and MODULES<br/>(CORE, cues, network, connectors, storage ...)"]
    API --> NEW["cohesion script new<br/>writes cohesion.d.ts for humans"]
    API --> TOOL["Agent code-tool description<br/>= CORE + module index<br/>+ Installed services + Connected services"]
    API --> DOC["documentation tool<br/>returns a module on demand"]
    SVC["Installed services (runtime)<br/>ListServicesCommand -> interface ServiceMap merge"] --> TOOL
    SVC --> DOC
    CA["Connectors and their accounts (runtime)<br/>ConnectedServices"] --> TOOL
```

## I5. Journal: one document per workspace per day

```mermaid
flowchart TD
    A["cohesion journal / journal add text / web / iOS"] --> B{"does today's journal exist?"}
    B -- "no" --> C["create Document { type: journal, title: YYYY-MM-DD, journal_date }"]
    B -- "yes" --> D["EditJournal (replace) or AppendToJournal (append)"]
    C --> D
    D --> E["JournalSaved event<br/>-> 120 s settle timer (Part D2)"]
    E --> F["journal:settled -> extensions' onJournalSettled<br/>and agents' cues on journal:settled"]
    D --> G["Invalidate Documents(ws)"]
    D --> H["search triggers re-index the document"]
```
