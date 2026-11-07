# Part G: Capabilities (clients that serve the daemon)

Ordinary clients *ask* the daemon for things. A client may also *offer* **capabilities**: the **actors** on its device (an agent such as Claude Code in one directory, that Cohesion hands actor sessions to), and then *answers the daemon's calls* to them. It is the same session, the same connection, the same identity; it just sends a few extra frames (`Hello`, `CallResult`, `ActorSessionUpdate`, `ActorUsage`) and handles a few extra inbound ones (`Call`, `Evicted`). [actors.md](../actors.md) walks through an actor's session frame by frame.

A `Hello` lists the client's actors, each an `ActorDefinition` (identifier, product, scope, modes, models and efforts). Each becomes an `Actor` row, read as "Claude Code on Ada's MacBook Pro"; one the client stops offering is soft-deleted and keeps its actor sessions.

## G1. The big picture: who owns what

```mermaid
flowchart TB
    subgraph DEVICE["The client (e.g. actor-agentsdk on a Mac)"]
        direction TB
        APP["Client's session<br/>connect, Hello, answer Calls,<br/>report each actor session's progress"]
        ACT["Actors<br/>(Claude Code in /src/cohesion, ...)"]
        APP --- ACT
    end

    subgraph DAEMON["cohesiond"]
        direction TB
        SESS["Session<br/>(the same one every client uses)"]
        REG["CapabilityClients registry<br/>client_id -> session queue, offers, actor usage, pending calls<br/>THE ONLY WRITER of a client's actor rows"]
        STORE["CapabilityStore -> SyncCapabilitiesCommand<br/>(one DB transaction)"]
        DB[("Database<br/>actors, actor_sessions, actor_session_messages")]
        TASKS["ActorSessions<br/>start / send / stop / set mode"]
    end

    APP == "Hello (once per connection,<br/>again when offers change)" ==> SESS
    SESS --> REG --> STORE --> DB
    TASKS -- "call(ActorCall)" --> REG
    REG == "ServerMessage::Call { call_id, call }" ==> APP
    APP == "CallResult { call_id, ok | error }" ==> SESS
    APP == "ActorSessionUpdate / ActorUsage" ==> SESS
    SESS -- "complete(call_id)" --> REG

    classDef dev fill:#ffedd5,stroke:#ea580c,color:#111
    classDef dae fill:#ede9fe,stroke:#7c3aed,color:#111
    class APP,ACT dev
    class SESS,REG,STORE,TASKS dae
```

> Lower crates never see a connection. The database only sees `SyncCapabilitiesCommand`.

## G2. Connecting and offering (sequence)

```mermaid
sequenceDiagram
    autonumber
    participant App as Actor client
    participant TR as Transport (socket or mTLS)
    participant S as Session (reader)
    participant R as CapabilityClients registry
    participant ST as CapabilityStore / DB
    participant Others as Subscribed screens

    App->>TR: connect (local key handshake, or paired cert)
    TR->>S: Session::new(conn, Client)
    S->>S: presence.join(client) -> client ONLINE (any session, offering or not)<br/>first session publishes Clients
    S-->>App: ServerMessage::Hello { protocol, server, client_name }
    App->>S: ClientFrame::Hello { id, actors: [ { cohesion, "Claude Code", /src/cohesion, modes, models } ] }
    S->>S: check_actors: identifiers present and unique,<br/>product and scope present, modes / models / efforts named once
    S->>R: register(client_id, offers, session outbound queue)
    R->>R: take the client's slot FIRST
    alt a session already holds that client's slot
        R-->>R: old session.evict(Replaced) -> its pending calls fail as Offline
    end
    R->>ST: sync(client_id, offers) (one at a time, only if this session still holds the slot)
    ST->>ST: SyncCapabilitiesCommand in one transaction:<br/>- an actor offered anew is created<br/>- one offered again is rewritten and restored<br/>- one no longer offered is soft-deleted, keeping its actor sessions
    ST-->>Others: Invalidate Actors (only when the rows changed)
    alt sync failed
        R-->>R: handle dropped -> slot given up
        S-->>App: Reply error
    else ok
        S-->>App: Reply accepted (same id)
        Note over App: Only now is the client "serving".<br/>Saying hello again on the same session replaces ONLY the offers<br/>and keeps calls already waiting.
    end
```

## G3. Inside one session: three tasks and a registry

```mermaid
flowchart LR
    subgraph CONN["One connection"]
        direction TB
        CRD["reader task (the session's own)<br/>- completes a CallResult IMMEDIATELY<br/>- handles Subscribe / Hello<br/>- hands actor updates on, in order<br/>- queues each Request"]
        HND["handler task<br/>runs queued requests side by side (up to 64)<br/>and replies to each as it finishes"]
        WRT["writer task<br/>drains ONE outbound queue:<br/>replies + Calls from the registry<br/>+ Invalidate batches once subscribed<br/>numbers every frame<br/>pings every 30 s over WebSocket"]
    end

    CRD -- "Queued::Request / Answered" --> HND
    HND -- "Reply" --> OUT[("outbound mpsc queue")]
    REGY["CapabilityClients.call()"] -- "ServerMessage::Call" --> OUT
    BCAST["DB invalidation broadcast"] -- "only if subscribed" --> WRT
    OUT --> WRT
    CRD -- "CallResult -> pending[call_id].send" --> REGY

    WHY["Why three tasks:<br/>- a session keeps reading and hearing about changes while a command runs<br/>- a write waiting on a full socket never stops it reading the ANSWER to a call"]
    WHY -.- CONN
```

## G4. What ends a session (and what the client must do)

```mermaid
stateDiagram-v2
    direction TB
    [*] --> connected: transport identified the client
    connected --> serving: Hello accepted (register + sync ok)
    serving --> serving: Hello again (offers replaced, pending calls kept)
    serving --> dropped: hang-up, write fails, or silent 60 s (no frame, no pong)
    serving --> replaced: another session sent a Hello for the same client
    serving --> revoked: client deleted (DeleteClient)
    serving --> failed: handler stopped (a command panicked)

    dropped --> connected: client RECONNECTS (backoff 1 s up to 60 s)
    failed --> connected: client reconnects
    replaced --> [*]: Evicted { replaced } - client must NOT reconnect by itself
    revoked --> [*]: Evicted { revoked } - must be paired again

    note right of dropped
        Leaving presence takes the client offline (any session,
        offering or not, silent 60 s over WebSocket is dropped).
        Dropping the session handle unregisters the client
        and fails its pending calls as Offline.
        Actor rows STAY (shown offline, as the clients list says).
    end note
    note right of replaced
        Reconnecting would just take the slot back
        from the other session in an endless fight.
    end note
```

## G5. Everything that can go wrong calling a client

```mermaid
flowchart TD
    CALL["CapabilityClients.call(client, ActorCall)"] --> S1{"session registered<br/>for this client?"}
    S1 -- "no" --> OFF["ClientCallError::Offline<br/>(instant)"]
    S1 -- "yes" --> S2{"does it offer<br/>this actor?"}
    S2 -- "no" --> NO["ClientCallError::NotOffered"]
    S2 -- "yes" --> SEND["send Call, await CallResult<br/>up to ActorCall::DEADLINE (60 s)"]
    SEND --> R1{"outcome"}
    R1 -- "client answers accepted" --> OK["Ok"]
    R1 -- "client answers error" --> FAIL["Failed(message)<br/>as people read it"]
    R1 -- "no answer before deadline" --> TO["TimedOut"]
    R1 -- "session dropped / replaced / revoked<br/>while waiting" --> OFF2["Offline<br/>(pending calls failed on unregister)"]

    OFF --> MAP
    NO --> MAP
    FAIL --> MAP
    TO --> MAP
    OFF2 --> MAP
    MAP["Mapped by ActorSessions:<br/>validation error 'Claude Code on Mac is offline.' / '... no longer offers this.'<br/>upstream error '... did not answer in time.'"]

    classDef bad fill:#fee2e2,stroke:#dc2626,color:#111
    class OFF,NO,FAIL,TO,OFF2 bad
```

> A session on an actor that is offline **fails with that error rather than waiting**. Nothing queues for it.

## G6. Frame vocabulary (the whole protocol on one page)

```mermaid
flowchart LR
    subgraph C2S["Client -> daemon: ClientFrame"]
        direction TB
        F1["Request { id, type, args }<br/>any protocol Command"]
        F2["Subscribe { id }<br/>start receiving Invalidate pushes"]
        F3["Hello { id, actors }<br/>OFFERING: here are my actors"]
        F4["CallResult { call_id, ok | error }<br/>OFFERING: the answer to a Call"]
        F5["ActorSessionUpdate { id, session_id, kind, ... }<br/>OFFERING: what an actor session said or where it stands"]
        F6["ActorUsage { id, actor, plan }<br/>OFFERING: how much of its limits an actor used"]
    end
    subgraph S2C["Daemon -> client: ServerMessage"]
        direction TB
        G1["Hello { protocol, server, client_name }<br/>first thing, always"]
        G2["Reply { id, ok(ReplyData) | error }<br/>one per Request / Subscribe / Hello / update"]
        G3["Invalidate { keys }<br/>only if subscribed"]
        G4["Reset<br/>subscriber fell behind: refetch everything"]
        G5["Call { call_id, call }<br/>OFFERING: only to a session that sent a Hello"]
        G6["Evicted { reason }<br/>replaced | revoked, then the session closes"]
    end
```
