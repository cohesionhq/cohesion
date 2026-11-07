# Part A: The product

## A1. What Cohesion is, as a product

One person, one daemon, many screens. Everything the person owns lives in a single database on the machine that runs `cohesiond`; every app is just a window onto it. Agents and extensions are the two ways the system does things on its own.

```mermaid
flowchart TB
    PERSON(["You"])

    subgraph SCREENS["Ways you look at and change your stuff"]
        direction LR
        WEB["Web UI<br/>(opened by cohesion open)"]
        CLI["cohesion CLI<br/>(task, journal, agent, ai ...)"]
        MAC["macOS menu bar app<br/>(runs the server, shows the web UI)"]
        IOS["iOS app<br/>(tasks, journal, articles, agent chat)"]
    end

    subgraph DAEMON["cohesiond: the one place the truth lives"]
        direction TB
        subgraph STUFF["Your stuff (per workspace)"]
            direction LR
            DOCS["Documents<br/>+ daily Journal"]
            TASKS["Tasks<br/>+ folders, priority, size, dates"]
            RESOURCES["Resources<br/>read-only copies from other services"]
        end
        subgraph SHARED["Shared across workspaces"]
            direction LR
            FEEDS["RSS / Atom feeds<br/>+ the articles they bring in"]
            ACCTS["Connector accounts<br/>(Notion ...)"]
            CLIENTS["Paired clients (clients)"]
        end
        subgraph DOERS["Things that act on their own"]
            direction LR
            AGENTS["Agents<br/>persistent assistants with a chat,<br/>cues (wake-ups) and choices"]
            EXTS["Extensions<br/>JS/TS plugins with hooks,<br/>services and connectors"]
            SCRIPTS["Scripts<br/>cohesion run + the agent code tool"]
        end
        SEARCH["Full-text search<br/>over documents, tasks, resources"]
    end

    subgraph BRAINS["Where the thinking happens (inference)"]
        direction LR
        CLOUD["Cloud LLM endpoints<br/>Anthropic / OpenAI / compatible"]
    end

    subgraph WORLD["The outside world"]
        direction LR
        SVC["Services you sign in to<br/>via OAuth (through extensions)"]
        WWW["Web pages, feeds,<br/>models.dev catalog"]
    end

    PERSON --> WEB & CLI & MAC & IOS
    WEB & CLI & MAC & IOS == "mutual-TLS / Unix socket<br/>JSON protocol" ==> DAEMON
    AGENTS -- "think with" --> BRAINS
    AGENTS -- "read/write through scripts" --> STUFF
    EXTS -- "hooks, connectors, services" --> STUFF
    EXTS <-- "OAuth, API calls" --> SVC
    DAEMON -- "fetch pages, read feeds,<br/>refresh model catalog" --> WWW

    classDef client fill:#dbeafe,stroke:#2563eb,color:#111
    classDef daemon fill:#ede9fe,stroke:#7c3aed,color:#111
    classDef brain fill:#dcfce7,stroke:#16a34a,color:#111
    classDef world fill:#f3f4f6,stroke:#6b7280,color:#111
    class WEB,CLI,MAC,IOS client
    class DOCS,TASKS,RESOURCES,ARTS,FEEDS,ACCTS,CLIENTS,AGENTS,EXTS,SCRIPTS,SEARCH daemon
    class CLOUD brain
    class SVC,WWW world
```

## A2. Who talks to whom (runtime topology)

Three kinds of party: the daemon, clients, and the outside world. The thick arrows are the paired, authenticated links.

```mermaid
flowchart LR
    subgraph HOST["The machine running cohesiond (Linux box, Mac mini ...)"]
        direction TB
        D["cohesiond process<br/>HTTPS :3000 by default (--port) + Unix socket"]
        DB[("SQLite file<br/>cohesion.db")]
        KEY[/"server key<br/>(self-signed cert)"/]
        LCLI["cohesion CLI<br/>(same machine)"]
        LWEB["cohesion open<br/>127.0.0.1:7343<br/>loopback proxy"]
        BROWSER["Browser tab<br/>(web UI)"]
        D --- DB
        D --- KEY
        LCLI == "Unix socket<br/>local key handshake" ==> D
        LWEB == "Unix socket" ==> D
        BROWSER -- "HTTP + launch token" --> LWEB
    end

    subgraph OTHERMAC["Another Mac (client mode)"]
        MACHARNESS["Mac WebUI harness"]
    end

    subgraph PHONE["iPhone / iPad"]
        IOSAPP["iOS app"]
        IPADHARNESS["iPad WebUI harness"]
        SHARE["Share extension"]
        NOTIF["Notification service"]
    end

    subgraph REMOTECLI["Another computer"]
        RCLI["cohesion CLI<br/>+ cohesion open"]
    end

    subgraph CLOUDLLM["LLM providers"]
        ANTH["Anthropic Messages"]
        OAI["OpenAI Responses"]
        COMPAT["Chat-completions compatible<br/>(local servers, OpenRouter ...)"]
    end

    subgraph NET["Internet"]
        MDEV["models.dev<br/>(model catalog)"]
        PAGES["Article pages + RSS/Atom feeds"]
        OAUTHSVC["OAuth services<br/>(Notion ...)"]
    end

    MACHARNESS == "mutual TLS /connect<br/>(WebSocket) with paired cert" ==> D
    IOSAPP == "mutual TLS /connect<br/>(WebSocket)" ==> D
    IPADHARNESS == "mutual TLS /connect<br/>(WebSocket)" ==> D
    RCLI == "mutual TLS /command + /connect" ==> D

    D -- "HTTPS, per turn" --> ANTH & OAI & COMPAT
    D -- "HTTPS" --> MDEV & PAGES
    D -- "HTTPS via extension scripts" --> OAUTHSVC

    classDef proc fill:#ede9fe,stroke:#7c3aed,color:#111
    classDef cl fill:#dbeafe,stroke:#2563eb,color:#111
    classDef ext fill:#f3f4f6,stroke:#6b7280,color:#111
    class D proc
    class LCLI,LWEB,BROWSER,MACHARNESS,IOSAPP,IPADHARNESS,SHARE,NOTIF,RCLI cl
    class ANTH,OAI,COMPAT,MDEV,PAGES,OAUTHSVC ext
```

## A3. What your data looks like (the content model)

Tables that matter to a person, and how they relate. `Workspace` is the container for your work; agents, feeds, accounts and clients sit outside any one workspace.

```mermaid
erDiagram
    WORKSPACE ||--o{ DOCUMENT : "contains"
    WORKSPACE ||--o{ TASK_FOLDER : "has"
    WORKSPACE ||--o{ TASK : "has"
    TASK_FOLDER ||--o{ TASK : "files (null = Inbox)"
    WORKSPACE ||--o{ RESOURCE : "imports into"
    FEED ||--o{ ARTICLE : "yields"
    CONNECTOR_ACCOUNT ||--o{ RESOURCE : "source of"
    EXTENSION ||--o{ CONNECTOR_ACCOUNT : "serves (extension accounts)"

    DOCUMENT {
        int id
        text title
        text contents
        text document_type "normal | journal"
        text journal_date "YYYY-MM-DD for journals"
    }
    TASK {
        int id
        text title
        text description
        text status "incomplete | completed | cancelled"
        int priority "none..urgent"
        int size "1..5 or null"
        bool assigned_to_user
        int assignee_agent_id "or null"
        int assignee_contact_id "or null; at most one of the three"
        text start_date
        text due_date
    }
    RESOURCE {
        text kind "the connector's short kind key"
        text contents "markdown, text or json"
        int last_updated
    }
    ARTICLE {
        text url
        text text "page as Markdown"
        bool read
    }
    CLIENT {
        text client_name
        text key_kind "certificate | local"
        text key "fingerprint, unique among live"
    }
    METADATA {
        text key
        text value
    }
    SEARCH_ENTRY {
        text entity_kind
        int entity_id
    }
    DOCUMENT ||--o| SEARCH_ENTRY : "indexed by trigger"
    TASK ||--o| SEARCH_ENTRY : "indexed by trigger"
    RESOURCE ||--o| SEARCH_ENTRY : "indexed by trigger"
    TASK ||--o{ METADATA : "key/value notes (any entity: kind + id)"
    DOCUMENT ||--o{ METADATA : "same"
```

## A4. What lives *outside* any workspace: intelligence and agent data

```mermaid
erDiagram
    INTELLIGENCE_ENDPOINT ||--o{ ENDPOINT_MODEL : "offers"
    CATALOG_MODEL ||--o{ ENDPOINT_MODEL : "optionally supplies prices + context"
    ENDPOINT_MODEL ||--o| INTELLIGENCE_DEFAULT : "chosen for a tier"
    INTELLIGENCE_AGENT ||--o{ AGENT_MESSAGE : "conversation feed"
    AGENT_MESSAGE ||--o{ AGENT_ATTACHMENT : "images"
    INTELLIGENCE_AGENT ||--o{ AGENT_CUE : "wake-ups"
    AGENT_TEAM |o--o{ INTELLIGENCE_AGENT : "organizes (an agent is on at most one)"
    INTELLIGENCE_AGENT |o--o{ MEMORY : "remembers (personal)"
    AGENT_TEAM |o--o{ MEMORY : "shares (a team's; global ones have neither)"
    INTELLIGENCE_AGENT ||--o{ AGENT_RUN : "background wake-ups, recorded"
    AGENT_CUE ||--o{ AGENT_RUN : "fired"
    AGENT_RUN }o--|| INTELLIGENCE_THREAD : "its model conversation"
    INTELLIGENCE_THREAD ||--o{ THREAD_MESSAGE : "every prompt, answer, tool result"
    INTELLIGENCE_THREAD ||--o{ COST_EVENT : "tokens + dollars per turn"
    INTELLIGENCE_AGENT ||--o{ STORAGE_ENTRY : "private key/value (agent:ID)"
    EXTENSION ||--o{ STORAGE_ENTRY : "private key/value (extension:IDENT)"
    EXTENSION ||--o{ EXTENSION_PERMISSION : "grants (all start OFF)"
    EXTENSION ||--o{ EXTENSION_HOOK : "hooks (all start OFF)"
    EXTENSION ||--o| EXTENSION_SERVICE : "declares a service block"
    EXTENSION_SERVICE ||--o{ SERVICE_FUNCTION : "functions, readOnly or not"

    INTELLIGENCE_ENDPOINT {
        text name "unique, case-insensitive"
        text server "Integrated{key} | Account{connector account}"
    }
    INTELLIGENCE_DEFAULT {
        text tier "agent | low | medium | high"
    }
    AGENT_MESSAGE {
        text sender "user | agent | backgroundAgent"
        text content "text | forward | codeExecution | error | documentation | choice | memory | note | woke | rebuiltContext"
        text reaction "emoji"
    }
    MEMORY {
        text scope "agent | team | global, at most 20 each by default"
        text memory "one sentence"
    }
    AGENT_TEAM {
        text title "unique, case-insensitive"
    }
    AGENT_CUE {
        text name "unique per agent"
        text hook "e.g. task:create"
        text script "JS predicate"
        text instructions "notes to future self"
        text effort "low | medium | high"
    }
    INTELLIGENCE_AGENT {
        text agent_name
        text color
        text emoji
        text role
        text directive
        text activity "null unless working"
    }
```
