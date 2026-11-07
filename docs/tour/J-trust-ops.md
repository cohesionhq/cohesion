# Part J: Trust boundaries and operations

## J1. Who is trusted for what

The design rule: validate at these boundaries and nowhere else. A capability a pool lacks is a *mount it does not have*, not a check inside a mount.

```mermaid
flowchart TB
    subgraph UNTRUSTED["UNTRUSTED: parse defensively, bound size and time"]
        direction LR
        STR["Strangers on the LAN<br/>and other local processes"]
        EXTSVC["External services<br/>provider APIs, models.dev,<br/>fetched pages and feeds"]
        TXT["Text the LLM reads<br/>articles, feeds, imported resources"]
    end

    subgraph GATE0["Gate 0: before pairing"]
        direction TB
        REG["/register + setup key<br/>(checked before any DB work,<br/>every failure = same 401)"]
        SOCK["Unix socket: file permissions<br/>+ LocalIdentity handshake"]
        PROXY["cohesion open proxy:<br/>loopback only, Host + Origin check,<br/>launch token, one-time nonce link"]
    end

    subgraph TRUSTEDPAIRED["TRUSTED: a paired client may do anything the protocol offers"]
        direction TB
        CL["Paired clients<br/>(wire input must be well-formed or refused)"]
        DEV["A client's offers, call results and actor updates<br/>are wire input (validated: titles, ids, duplicates, sizes)"]
    end

    subgraph EXTT["TRUSTED once installed AND a grant is on"]
        EXTS["Extensions<br/>grant => mount; nothing inside a mount re-checks"]
    end

    subgraph SEMI["SEMI-TRUSTED: the LLM"]
        direction TB
        FG["Conversation and its forks:<br/>your rights, on the strength of YOUR message"]
        BG["Fork a cue started:<br/>reads, no network,<br/>asks you before any change"]
        CU["Cue:<br/>reads and notifies, nothing more"]
        PR["A change waits for YOUR pick"]
    end

    STR -. "gets nothing" .-> GATE0
    GATE0 --> TRUSTEDPAIRED
    TXT -- "prompt injection risk is why" --> BG
    BG -- "ask_choice" --> PR --> FG
    EXTSVC -. "bounded + parsed defensively" .-> EXTS
    EXTSVC -. "same" .-> SEMI

    SEC["Secrets: connector credentials reachable ONLY via CredentialManager;<br/>ConnectorAccount type omits the columns; API keys redacted in RedactedIntelligenceEndpoint"]
    TRUSTEDPAIRED -.- SEC

    classDef bad fill:#fee2e2,stroke:#dc2626,color:#111
    classDef good fill:#dcfce7,stroke:#16a34a,color:#111
    classDef mid fill:#fef3c7,stroke:#d97706,color:#111
    class STR,EXTSVC,TXT bad
    class CL,DEV,EXTS good
    class FG,BG,PR mid
```

## J2. Graceful shutdown

```mermaid
sequenceDiagram
    autonumber
    participant SIG as SIGINT / SIGTERM / stdin closed
    participant SD as shutdown::run
    participant WT as WorkTracker
    participant HM as HookManager
    participant DB as DB worker
    participant CL as Clients

    SIG->>SD: requested
    SD->>WT: close(): run_new() and begin_new() now refuse
    Note over WT: Refused: every client request on any transport ("shutting down"),<br/>/register, offers, the day clock, the resource refresh.<br/>Still admitted via begin(): hooks, cue runs, scripts, model calls<br/>that running work starts.
    SD->>HM: settle_all(): queue every settled hook still waiting NOW (onJournalSettled per journal)
    loop until the tracker is idle
        WT-->>SD: still waiting on: "agent 3: cue 'Urgent tasks'", "hook task:create" ...
        SD->>SD: logs what it waits for every 5 s
    end
    SD->>DB: close(): run commands already sent, then close the connection
    SD-->>SIG: exit 0
    Note over SD: A second signal, or 5 minutes, forces exit 1.
    Note over CL: The server keeps answering during shutdown (refusing),<br/>so clients hear WHY rather than finding nothing there.
```

## J3. What counts as "work in flight"

```mermaid
flowchart LR
    WT["WorkTracker<br/>label -> count"]
    R1["client request<br/>(handle_request, /register, offers)"] --> WT
    R2["queued hook<br/>(from queue() until it has run)"] --> WT
    R3["agent turn / fork<br/>(taken in AgentCoordinator BEFORE the task is spawned)"] --> WT
    R4["resource refresh pass"] --> WT
    NOTE["The DB worker hands a command's events to HookManager BEFORE replying,<br/>so the hooks a request raises are counted before the request ends:<br/>there is no gap where the tracker looks idle but a hook is about to run."]
    WT -.- NOTE
```

## J4. Logging

```mermaid
flowchart LR
    CODE["log_info!(Module::X, ...) anywhere<br/>(no handle needed: global, stateless)"] --> LG["Logger (installed once in main)"]
    LG --> CON["ConsoleAdapter at --log-level<br/>debug | info | warn | error<br/>(SQL statements at debug)"]
    LG --> MORE["other Adapters chained with .with(adapter)"]
    TR["Transports put the Caller into the message text"] -.-> CODE
    SECRET["Setup code goes to STDOUT only,<br/>never the log (others may read logs)"] -.- CODE
```

## J5. Development and local deployment (on this machine)

```mermaid
flowchart TD
    DEV["scripts/server.sh<br/>debug daemon with -c ./dev"] --- DDIR[("dev/ (git-ignored)<br/>YOUR real local state:<br/>never write to dev/cohesion.db in tests;<br/>copy to a scratch dir")]
    CLIDEV["scripts/client.sh<br/>debug CLI with -c ./dev/client"] --> DEV
    SMOKE["smoke test (CONTRIBUTING.md):<br/>scratch config + short --socket /tmp/x.sock<br/>+ cargo run -p cohesiond --example fake_actor"] --> DEV
    CI["Checks, by hand: cargo fmt --all --check<br/>cargo clippy ... -D warnings<br/>cargo test --workspace --locked"]
    INST["scripts/install-server.sh -> cargo install cohesiond<br/>then systemctl restart --user cohesion<br/>(the installed binary is what the local unit runs)"]
```
