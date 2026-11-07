# Part K: Putting it all together

## K1. The master map: every moving part and the arrows between them

Read this once, then use Parts A to J as zoom-ins. Letters in brackets point at the part that expands each box.

```mermaid
flowchart TB
    PERSON(["You"])

    subgraph CLIENTS["Clients [C]"]
        direction LR
        WEB["Web UI"] --- PROXY["cohesion open proxy"]
        CLI["cohesion CLI"]
        IOS["iOS app"]
        ACTORCLI["Actor client<br/>(e.g. actor-agentsdk)"]
    end

    subgraph DAEMON["cohesiond"]
        direction TB
        subgraph FRONT["Front door [B1, C]"]
            direction LR
            TLS["HTTPS :3000 by default (--port)<br/>/register /command /connect"]
            SOCK["Unix socket"]
            SESSION["Session<br/>reader / handler / writer"]
        end
        subgraph BRAIN["cohesionkit facade [B5]"]
            direction LR
            ROUTES["Route impls"]
            HOOKQ["HookManager -> HookReceiver [D]"]
            EXTM["ExtensionManager [H]"]
            CAPREG["CapabilityClients registry [G]"]
            COORD["AgentCoordinator -> AgentHost [E]"]
            CONNS["connectors + credentials [H5-H7]"]
        end
        subgraph INTEL["Inference [F]"]
            direction LR
            TIER["Tiers: agent / low / medium / high"]
            SESS["Session (turn loop, tools, cost)"]
            BACKENDS["Backends: Messages, Responses,<br/>ChatCompletions"]
            CATA["ModelCatalog"]
        end
        subgraph SCRIPTS["Scripts [H2, I3]"]
            direction LR
            POOLS["general / turns /<br/>cues pools"]
            SCOPES["extension scopes"]
        end
        subgraph DATA["Data [A3, A4, B6]"]
            direction LR
            WORKER["DB worker thread<br/>SAVEPOINT per command"]
            SQLITE[("SQLite<br/>cohesion.db")]
        end
        subgraph TIMERS["Time [D3, H7, I1]"]
            direction LR
            CLOCK["day clock"]
            REFRESH["resource + feed refresh"]
        end
    end

    subgraph OUTSIDE["Outside world"]
        direction LR
        CLOUDLLM["LLM providers"]
        SERVICES["OAuth services"]
        WEBP["Pages + feeds + models.dev"]
    end

    PERSON --> CLIENTS
    CLIENTS --> TLS & SOCK --> SESSION --> ROUTES
    ROUTES --> WORKER --> SQLITE
    WORKER -- "invalidations" --> SESSION
    WORKER -- "events" --> HOOKQ
    CLOCK --> HOOKQ
    HOOKQ --> EXTM --> SCOPES
    HOOKQ --> COORD
    ROUTES --> COORD
    ROUTES --> EXTM
    ROUTES --> CONNS
    COORD --> TIER --> SESS --> BACKENDS
    COORD --> POOLS
    SCOPES & POOLS --> WORKER
    BACKENDS --> CLOUDLLM
    COORD --> CAPREG
    CAPREG <-- "Call / CallResult<br/>same session" --> SESSION
    ACTORCLI -. "Hello (actors)" .-> SESSION
    CONNS --> EXTM
    SCOPES --> SERVICES
    REFRESH --> CONNS
    REFRESH --> WEBP
    CATA --> WEBP

    classDef cl fill:#dbeafe,stroke:#2563eb,color:#111
    classDef dm fill:#ede9fe,stroke:#7c3aed,color:#111
    classDef db fill:#fef3c7,stroke:#d97706,color:#111
    classDef out fill:#f3f4f6,stroke:#6b7280,color:#111
    classDef ed fill:#ffedd5,stroke:#ea580c,color:#111
    class WEB,PROXY,CLI,IOS cl
    class ACTORCLI,CAPREG ed
    class TLS,SOCK,SESSION,ROUTES,HOOKQ,EXTM,COORD,CONNS,TIER,SESS,BACKENDS,CATA,POOLS,SCOPES,CLOCK,REFRESH dm
    class WORKER,SQLITE db
    class CLOUDLLM,SERVICES,WEBP out
```

## K2. A whole story: "Tell me when something urgent lands"

One timeline touching clients, DB, hooks, cue evaluation, inference tiers, a choice, and the phone. Every step maps to a diagram above.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant Web as Web UI
    participant D as cohesiond
    participant DB as DB worker
    participant Cloud as Cloud LLM (agent tier)
    participant Haiku as Cloud LLM (low tier)
    participant HR as HookReceiver
    participant Phone as iOS app

    rect rgb(240,248,255)
    Note over You,D: SETUP (once)
    You->>Web: Settings > add Anthropic endpoint, tier agent = claude-sonnet
    Web->>D: createIntelligenceEndpoint, addEndpointModel, setIntelligenceDefault
    D->>DB: rows + Invalidate Intelligence [F2]
    You->>Web: tier low = claude-haiku (cheap, can use tools)
    D->>DB: setIntelligenceDefault(low)
    You->>Web: create agent "Ada"
    Web->>D: createIntelligenceAgent [E2]
    end

    rect rgb(240,255,240)
    Note over You,Cloud: GIVE THE AGENT A JOB (foreground turn, cloud model) [E3, E5]
    You->>Web: chat to Ada: "tell me when something urgent lands"
    Web->>D: sendAgentMessage -> AcceptedResponse
    D->>Cloud: Session on tier agent: status block + your message
    Cloud-->>D: tool call code-tool: agent.cue({ hook: task:create, code: task => task.priority === urgent, effort: low })
    D->>DB: SetAgentCueCommand + status "Watching urgent tasks"
    Cloud-->>D: send_message "Done. I'll wake on urgent tasks."
    DB-->>Web: Invalidate Agent(Ada): message appears, status = waiting
    end

    rect rgb(255,248,240)
    Note over You,Phone: LATER: the event happens [D1, E6]
    You->>Phone: create task "Pay rent" priority urgent
    Phone->>D: createTask (over TLS /connect)
    D->>DB: INSERT task (savepoint), raise TaskCreated
    DB-->>HR: queued hook task:create
    HR->>HR: extensions first (none) then AgentCoordinator.fire
    HR->>D: evaluate_cue on background pool: priority === urgent => fired
    D->>DB: CreateAgentRunCommand (running)
    D->>Haiku: Session on tier low: wake message (cue instructions + the task as payload)
    Haiku-->>D: tool call send_message "Urgent task: Pay rent"
    D->>DB: agent message (sender backgroundAgent, trace continues from the event)
    DB-->>Phone: Invalidate Agent(Ada)
    Phone-->>You: Ada: "Urgent task: Pay rent"
    D->>DB: FinishAgentRun(completed), status back to waiting
    end

    rect rgb(255,245,255)
    Note over You,DB: THE AGENT WANTS TO CHANGE SOMETHING [E7]
    Haiku-->>D: (same run) tool call ask_choice("Move Pay rent to Bills folder?")
    D->>DB: choice message, waiting for a pick
    DB-->>Phone: Invalidate Agent(Ada): question with lettered answers
    You->>Phone: picks "Yes"
    Phone->>D: respondToAgentChoice(Yes)
    D->>D: a turn under # User's pick, started by you, makes the change
    D->>DB: moveTask (trace origin = Ada, so Ada's own cues won't re-wake)
    DB-->>Web: Invalidate TaskList(...) : every open screen updates [B7]
    end
```

## K3. "Where does X live?" cheat sheet

```mermaid
flowchart LR
    Q1["Add a new kind of event that wakes things"] --> A1["DatabaseEventKind + hook_events! table<br/>(or day_clock.rs for timed)"]
    Q2["Add a script capability"] --> A2["a Mount behind an extension permission<br/>Mount::for_permission"]
    Q3["Add state a client shows"] --> A3["a model + commands that raise Invalidations;<br/>add to every_model!"]
    Q4["Add something a client can do"] --> A4["a new list in ClientCapabilities + a ClientCall variant<br/>(typed answer, deadline) + mirror rows"]
    Q5["Add a new LLM wire format"] --> A5["IntelligenceBackend impl + Dialect variant<br/>(cohesion-intelligence/src/providers)"]
    Q6["Add a protocol command"] --> A6["Request struct + Command variant (via = kind)<br/>DbRequest -> DatabaseCommand (Access = Read | Write)"]
    Q7["Let a lower crate use a higher one's thing"] --> A7["declare a small trait below, implement it above<br/>(ConnectorAccess, ServiceAccess, AccountCredentials)"]
    Q8["Enforce a rule"] --> A8["schema constraint > type (enum) > one command > route<br/>(once, at the lowest layer that can hold it)"]
```
