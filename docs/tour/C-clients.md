# Part C: Clients, identity and pairing

There is no username or password anywhere. A **client** is a row in the `clients` table, identified by *how it proved itself before the session began*: a TLS certificate fingerprint (`key_kind = certificate`) or a local key hash (`key_kind = local`). A client has no "type"; whether it offers capabilities is just whether it sends a `Hello`.

## C1. The family of clients

```mermaid
flowchart TB
    subgraph NEEDS_SERVER["All of them need cohesiond; none keeps data of its own"]
        direction TB

        subgraph CLIFAM["cohesion CLI family"]
            direction TB
            CLI["cohesion &lt;noun&gt; &lt;verb&gt;<br/>task, journal, agent, ai, article, ..."]
            CLIAPI["cohesion api '&lt;json&gt;'<br/>any protocol Command, raw"]
            CLIRUN["cohesion run script.js<br/>run a script in the daemon"]
            CLIOPEN["cohesion open<br/>serve web UI on 127.0.0.1:7343"]
            CLIPAIR["cohesion pair<br/>BeginPairing -> setup code"]
        end

        subgraph WEBFAM["Web UI (SvelteKit, embedded in the CLI binary)"]
            WEBUI["Svelte stores per resource<br/>Resource.fetch + matches(invalidation)"]
        end

        subgraph APPFAM["Native apps"]
            direction TB
            MACAPP["macOS menu bar app<br/>server mode: runs cohesiond as a child<br/>client mode: pairs with another Mac"]
            IOSAPP["iOS app<br/>tasks, journal, articles, agent chat,<br/>model settings, APNs push via companion extension"]
            SHARE["iOS Share extension<br/>send a URL into the app"]
        end
    end

    CLIOPEN --> WEBUI
    MACAPP -- "bundles + spawns" --> CLIOPEN

    CLI & CLIAPI & CLIRUN & CLIPAIR -- "Connection::Local<br/>Unix socket (same machine)" --> D
    CLI & CLIAPI -- "Connection::Remote<br/>mutual TLS (other machine)" --> D
    IOSAPP -- "WebSocket /connect<br/>pins the server key" --> D
    SHARE --> IOSAPP
    MACAPP -- "socket (server mode)<br/>or TLS (client mode)" --> D

    D(["cohesiond"])

    classDef c fill:#dbeafe,stroke:#2563eb,color:#111
    classDef d fill:#ede9fe,stroke:#7c3aed,color:#111
    class CLI,CLIAPI,CLIRUN,CLIOPEN,CLIPAIR,WEBUI,MACAPP,IOSAPP,SHARE c
    class D d
```

## C2. Pairing a new client (remote, over the network)

A **setup key** exists only during a *pairing window*: it dies after one registration or ten minutes. The setup code is `base64("key|host|server-fingerprint")`, so it carries the secret, where to connect and which server key to expect. The host is an ip or a DNS name (`--host`). When clients connect on a port other than the default 3000 (`--public-port`, else `--port`), the address is `host:port` (`[v6]:port` for IPv6); a code without a port means 3000.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant Host as Host terminal (cohesion pair)
    participant D as cohesiond
    participant Sec as Secrets
    participant New as New client (cohesion setup CODE)

    alt first ever start (no clients yet)
        D->>Sec: begin_pairing()
        D-->>You: prints "Setup code: ..." to STDOUT (never the log)
    else later
        You->>Host: cohesion pair
        Host->>D: BeginPairing (FromClient route, from a paired client the security settings allow)
        D->>Sec: begin_pairing() -> new key, 10 min, single use
        D-->>Host: setup code
        Host-->>You: code on screen
    end
    You->>New: paste code
    New->>New: decode key + ip + server fingerprint<br/>generate client keypair + certificate
    New->>D: TLS connect, pinned to server fingerprint
    New->>D: POST /register { setup_key, client_name } presenting its client cert
    D->>Sec: verify_setup_key BEFORE any database work
    alt wrong, expired or reused key
        D-->>New: 401 (same answer for every failure)
    else key ok
        D->>D: RegisterClientCommand (fingerprint -> clients row, key_kind=certificate)
        D->>D: UnknownPeers forgets this fingerprint
        D-->>New: registered
    end
    New->>D: later: every connection presents the same cert
    D->>D: PeerAcceptor looks the fingerprint up in clients<br/>(unknown ones are remembered briefly so a stranger costs no DB job)
    Note over D,New: Only now is a Session built. The session is handed a Client<br/>and never sees how it was identified.
```

## C3. Local clients on the Unix socket

No TLS here. The socket file's permissions are the trust.

```mermaid
sequenceDiagram
    autonumber
    participant CLI as cohesion setup --local
    participant FS as Client config dir
    participant D as cohesiond (local_acceptor)
    participant DB as IdentifyLocalClientCommand

    CLI->>FS: make a random local key, save it
    Note over CLI: --name or hostname becomes the client's display name
    CLI->>D: connect to Unix socket
    CLI->>D: line 1: LocalIdentity { key, name }
    D->>DB: resolve: client whose key_kind=local AND key = SHA-256(key)
    alt first time this key is seen
        DB->>DB: create client row under that name
    else client was deleted
        DB-->>D: refused
    end
    D-->>CLI: LocalHandshakeReply accepted | refused(why)
    D->>D: accepted -> Session::new(exchange, context, Caller, Client)
    Note over CLI,D: A client's name is only how it is listed.<br/>RenameClient can change it, Hello carries it back.
```

## C4. The web UI path: `cohesion open`

The browser has no identity of its own. A loopback proxy owned by the CLI lends it the CLI's.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant CLI as cohesion open
    participant LN as launch_link (nonce, 30 s)
    participant Br as Browser
    participant Px as Loopback proxy 127.0.0.1:7343
    participant Gd as Guard
    participant D as cohesiond

    You->>CLI: cohesion open
    CLI->>CLI: load or create launch token (private file in config dir)
    CLI->>LN: mint one-time nonce
    CLI->>Br: open http://127.0.0.1:7343/launch/NONCE
    Br->>Px: GET /launch/NONCE
    Px->>LN: redeem (once, within 30 s)
    Px-->>Br: redirect to UI with token in the URL FRAGMENT
    Note over Br: Token never appears on a command line,<br/>in output or in a server log.
    Br->>Px: GET static UI (embedded web/build via rust-embed)
    Br->>Px: API calls + WebSocket /connect, header carries token
    Px->>Gd: Host must be the bound loopback addr,<br/>Origin must be none or the proxy itself,<br/>token must match
    Gd-->>Px: ok
    Px->>D: relays over the CLI's own Connection (socket or TLS)
    D-->>Px: replies + Invalidate pushes
    Px-->>Br: frames passed through unchanged (bridge.rs)
    Note over Br,D: To the daemon, the web UI IS the CLI's client.<br/>Revoke that client and the browser is locked out too.
```

## C5. What happens when you delete a client (revoke a device)

```mermaid
flowchart TD
    A["DeleteClient<br/>(Local route)"] --> B["clients row soft-deleted"]
    B --> C["DB trigger soft-deletes the client's<br/>actors (their actor sessions stay)"]
    B --> E["CapabilityClients.remove(client_id)<br/>if connected: session is evicted"]
    E --> F["session writes what it queued,<br/>then ServerMessage::Evicted { revoked }"]
    F --> G["client must NOT reconnect by itself<br/>(must be paired again)"]
    B --> H["PeerAcceptor: fingerprint no longer live,<br/>new TLS connection is refused"]
    C --> I["publishes a Clients<br/>invalidation"]
```
