# Part H: Extensions, connectors and scripts

An **extension** is one JS/TS file with a `manifest()` and some hook functions. It is the way to add behaviour *without* changing Rust: react to events (hooks), expose callable functions (services), sign you in to outside services (OAuth accounts), import items (connectors), offer other extensions to install (registries), or run on a timer.

Trust rule: an extension is trusted **once installed and a grant is on**. Everything starts **off**. A grant decides which *mounts* (JS globals) its sandbox gets; nothing inside a mount second-guesses the grant.

## H1. Lifecycle of an extension

```mermaid
flowchart TD
    subgraph INSTALL["Install"]
        direction TB
        I0["cohesion extension install ext.js<br/>or web UI Find / registry preview"] --> I1["RegisterExtension (Local route)"]
        I1 --> I2["ExtensionManager evaluates the file in an<br/>ISOLATED read-only worker with NO globals<br/>(only entity classes + enums)"]
        I2 --> I3["call manifest()"]
        I3 --> I4{"vet manifest"}
        I4 -- "unknown permission or hook<br/>connector without oauth<br/>service + service:isolated both listed<br/>bad service block" --> IFAIL["install rejected (validation error)"]
        I4 -- "ok" --> I5["drop icon if not data:image/... or https,<br/>or &gt; 64 KB"]
        I5 --> I6["InstallExtensionCommand (one transaction):<br/>extension + permissions + hooks + service block<br/>ALL SWITCHED OFF<br/>(reinstall keeps grants the manifest still lists)"]
        I6 --> I7["shut down any running scope for this id"]
    end

    subgraph GRANT["You decide"]
        direction TB
        G1["Extensions > Installed<br/>or setExtensionPermission / setExtensionHook"] --> G2["commit the change"]
        G2 --> G3["THEN take the per-extension lock<br/>and shut the scope down<br/>(timers stop once no call holds it)"]
        G3 --> G4["if `launch` hook is ON:<br/>queue onLaunch (at most one pending per extension)"]
    end

    subgraph RUN["Running"]
        direction TB
        R1["first use spawns the persistent scope<br/>(its own QuickJS thread)"]
        R2["mounts = Mount::for_permission(each ON permission)<br/>+ log + storage (always)"]
        R3["calls: hooks, timers, services, OAuth,<br/>connector import fns, registry fns"]
        R1 --> R2 --> R3
    end

    subgraph END["Update / remove"]
        direction TB
        U1["registry offers a new version<br/>-> extension:update hook (announced once)"]
        U2["Uninstall: scope shut down,<br/>its accounts + their resources deleted,<br/>updates dropped"]
    end

    INSTALL --> GRANT --> RUN --> END
    RUN -. "grant changed" .-> GRANT
```

## H2. Script sandboxes: who gets which globals

Every script runs on a dedicated thread in a fresh QuickJS runtime; scopes never share prototypes. What differs is the pool and the mounts.

```mermaid
flowchart TB
    subgraph POOLS["Script pools / scopes"]
        direction TB
        P1["general pool<br/>cohesion run + RunScript command"]
        P2["turns pool (AgentRole::Turn)<br/>agent chat and forks' code-tool"]
        P3["cues pool (AgentRole::Cue)<br/>cue functions"]
        P4["extension scope (persistent, one per extension)"]
        P5["manifest-vetting worker<br/>read-only, no globals"]
    end

    subgraph MOUNTS["Mounts (globals)"]
        direction TB
        M1["cohesion<br/>workspaces, documents, tasks, search, metadata"]
        M2["network.fetch"]
        M3["connectors<br/>(via ConnectorAccess)"]
        M4["services.use(identifier)<br/>(via ServiceAccess)"]
        M5["storage<br/>(SandboxKey: agent:ID or extension:IDENT)"]
        M6["agent<br/>cue / removeCue"]
        M8["accounts<br/>(oauth extensions: own accounts)"]
        M9["timer.every"]
        M10["usage<br/>token + cost totals"]
        M11["ArticleFetch (no global):<br/>lets resnapshot fetch pages"]
        M12["log"]
        M13["intelligence<br/>(run a model from a script)"]
    end

    P1 --> M1 & M4 & M11 & M12 & M13
    P2 --> M1 & M2 & M3 & M4 & M5 & M6 & M11
    P3 --> M1 & M4 & M5
    P4 -. "only if the matching permission is ON" .-> M1 & M2 & M4 & M8 & M9 & M10
    P4 --> M5 & M12

    DBG["Database access through GuardedDatabaseContext<br/>read gate: always works, only Read commands<br/>write gate: only Write commands, throws JS Error when read-only<br/>cohesion:write alone makes an extension's guard writable<br/>background agents: read-only. foreground agents and `cohesion run`: read-write"]
    MOUNTS -.- DBG
```

Quick reference of the permission → mount map (`Mount::for_permission`):

| Permission | Mount / effect |
|---|---|
| `cohesion:read`, `cohesion:write` | `cohesion` global (write makes the guard writable and hook arguments writable) |
| `network` | `network` |
| `timer` | `timer` |
| `oauth` | `accounts`; daemon may call the sign-in functions |
| `services` | `services` (call other extensions' declared functions) |
| `usage` | `usage` |
| `registry`, `connector`, `service`, `service:isolated` | **no global**: they let the *daemon* (or clients) call certain functions in the extension |

## H3. A hook reaching an extension (concurrency detail)

```mermaid
sequenceDiagram
    autonumber
    participant HR as HookReceiver
    participant EM as ExtensionManager
    participant LK as per-extension lock
    participant DB as DB
    participant SC as Extension scope (QuickJS worker)

    HR->>DB: GetExtensionsWithHookCommand(task:status)
    DB-->>HR: extensions with the hook ON
    loop each extension (one failing doesn't stop the rest)
        HR->>HR: skip if trace.is_from_extension(identifier)
        HR->>EM: hook(extension_id, kind, closure)
        EM->>LK: lock(id)
        EM->>DB: re-read extension + grants + IsExtensionHookEnabledCommand (one EXISTS)
        alt scope not running
            EM->>SC: spawn with mounts from the grants
        end
        EM->>EM: take a ScopeHandle (keeps the scope alive for this call)
        EM->>LK: unlock (lock is NOT held during the call)
        EM->>SC: sandbox.traced(trace, evaluate onTaskStatus(task))
        SC->>DB: reads / writes (events raised carry this extension's trace)
        SC-->>HR: ok | error -> logged, never blocks other extensions
    end
    Note over EM,SC: A grant change commits, then takes the same lock to shut the scope down.<br/>Calls after that see the change, a call that checked first keeps its handle<br/>and runs ONCE on the scope it found.
```

## H4. Services: extensions calling each other, and clients calling extensions

```mermaid
flowchart TB
    subgraph CALLERS["Who can call a service function"]
        direction TB
        C1["A client (CLI, web UI, app)<br/>callExtension { identifier, name, args }<br/>needs the extension granted `service` or `service:isolated`"]
        C2["An agent script<br/>services.use('com.example.mailer').send(args)"]
        C3["Another extension holding `services`"]
        C4["A cue predicate / job<br/>(only functions declared readOnly)"]
    end

    EM["ExtensionManager.service_call<br/>looks the extension up by manifest identifier<br/>(NotFound if none)"]
    CHK{"declared in the manifest's `service` block?"}
    SCOPE["calls serviceRequest(name, args) on the<br/>extension's own scoped worker<br/>WITH ITS OWN GRANTS (read-only does not carry over)"]
    RES["JSON result (undefined -> null)<br/>or the thrown message as a validation error"]

    C1 --> EM
    C2 & C3 & C4 --> GATE{"not isolated?<br/>`service` grant ON?<br/>declares service block?"}
    GATE -- "no" --> THROW["services.use throws"]
    GATE -- "yes" --> EM
    EM --> CHK
    CHK -- "no" --> REF["callExtension refuses an undeclared name"]
    CHK -- "yes" --> SCOPE --> RES

    DEAD["The nested call is cut off at the CALLER's deadline.<br/>A call back into an extension already waiting on the caller<br/>(or into the caller itself) throws instead of deadlocking."]
    SCOPE -.- DEAD

    LIST["Agents' index of services:<br/>ListServicesCommand = declared services with `service` ON<br/>(service:isolated never listed), ordered by identifier"]
    LIST -.-> C2
```

## H5. Connectors: adding an account (OAuth) and importing

An extension holding `oauth` + `connector` is a connector. The daemon calls *its* sign-in and import functions.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Connectors > Accounts
    participant D as cohesiond (connector_routes)
    participant EM as ExtensionManager
    participant X as Extension (e.g. Notion)
    participant SVC as Outside service
    participant DB as DB (CredentialManager)

    You->>UI: Add account > Notion
    UI->>D: listConnectorServices (oauth extensions: serviceDescription())
    UI->>D: beginConnectorAuthentication { extension_id }
    D->>EM: call authenticationURL() (grants checked: oauth)
    EM->>X: authenticationURL()
    X-->>UI: sign-in URL
    UI->>SVC: you sign in in the browser
    SVC-->>You: a code
    You->>UI: paste / return with code
    UI->>D: completeConnectorAuthentication { extension_id, code }
    D->>X: completeAuthentication(code) (network: token exchange)
    X-->>D: Credential { authorization, refresh?, expiresAt? }
    D->>X: getAccountDescription(credential)
    X-->>D: { identifier, description } ("Acme workspace")
    D->>DB: store account + credential<br/>(same identifier again = replace the credential)
    DB-->>UI: Invalidate ConnectorAccounts
    Note over DB: ConnectorAccount leaves the credential columns OUT.<br/>The credential is reachable only through CredentialManager<br/>(DatabaseContext::credentials). No command, client or script sees a token,<br/>except the owning extension via accounts.all()[i].credential().

    You->>UI: Import > browse account
    UI->>D: listConnectorOrganizationUnits / searchConnectorResources
    D->>DB: fresh_credential(extension_id, account_id) (refresh if expiring in #lt; 60 s)
    D->>X: getOrganizationUnits / listResources(filters, credential)
    X->>SVC: API calls (network)
    X-->>UI: previews (identifier, title, lastUpdated, kindIdentifier, metadata)
    You->>UI: import item to workspace W
    UI->>D: importResource { account_id, workspace_id, identifier }
    D->>X: getResource(identifier, credential)
    X-->>D: ConnectorResource { title, contents (markdown | json), kindIdentifier ... }
    D->>DB: insert resource (kind = EXTENSION_IDENT.KIND), search index trigger
    DB-->>UI: Invalidate Resources(W)
    DB-->>D: event ResourceImported -> hook resource:import
```

## H6. Credentials are refreshed exactly once

```mermaid
flowchart TD
    A["Something needs account A's credential<br/>(daemon import call, or the extension via credential())"] --> B["CredentialManager.fresh(extension_id, account_id, refresh)"]
    B --> C["take the per-ACCOUNT lock"]
    C --> D["re-read the credential from the DB"]
    D --> E{"expires within 60 s?"}
    E -- "no" --> F["hand it over<br/>(only to the extension serving that account)"]
    E -- "yes" --> G["call refresh = the extension's completeRefresh(credential)<br/>run as a job ON THE EXTENSION'S OWN SCOPE<br/>(never awaiting a scope while holding the lock)"]
    G --> H["store the result<br/>(even if the extension lacks cohesion:write)"]
    H --> F
    I["A second caller finding it expiring waits on the lock<br/>and receives the already-stored refresh<br/>=&gt; one refresh token is never spent twice"] -.-> C
    J["Asking for a credential from inside a refresh on the same thread<br/>fails with CredentialError::Reentrant<br/>(instead of deadlocking on itself)"] -.-> B
```

## H7. Resource refresh loop

```mermaid
flowchart TD
    S["daemon start"] --> W1["wait 1 minute"]
    W1 --> P["refresh pass"]
    P --> NEXT["wait 30 minutes"] --> P
    MANUAL["refreshResources { workspace_id }<br/>(web UI: Refresh now)"] --> P

    P --> L["for each imported resource"]
    L --> X["fresh_credential -> getResource(identifier)"]
    X --> CMP{"lastUpdated or contents changed?"}
    CMP -- "no" --> NOWRITE["write nothing"]
    CMP -- "yes" --> WR["update resource -> event ResourceUpdated<br/>-> hook resource:update"]
    P --> REP["answers { checked, updated, failed }<br/>one failure is logged and doesn't stop the pass"]
    SHUT["pass counted by WorkTracker;<br/>stops when shutdown closes the tracker"] -.-> P
```

## H8. Registries and update announcements

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Extensions > Find
    participant EM as ExtensionManager
    participant R as Registry extension (granted `registry`)
    participant DB as DB
    participant HM as HookManager
    participant X as Installed extension X

    You->>UI: search "calendar"
    UI->>EM: SearchExtensionRegistries
    EM->>R: registryResults(query) for EACH extension granted `registry`
    R-->>UI: entries (identifier, version, source URL ...)
    You->>UI: Preview
    UI->>EM: PreviewExtension: download the script, read manifest (isolated worker), show for review
    You->>UI: Install > goes through H1 (all grants OFF)

    Note over EM: Update check: at start, then daily, or CheckExtensionUpdates
    EM->>R: registryLookup(installed identifiers)
    R-->>EM: entries at higher versions
    EM->>EM: keep in memory for ListExtensionUpdates<br/>(offered only while the extension with that id still has that identifier)
    EM->>DB: AnnounceExtensionUpdateCommand<br/>UPDATE ... WHERE announced_version IS NOT ?
    alt the UPDATE changed the row (first time this version is seen)
        EM->>HM: queue extension:update
        HM->>X: onExtensionUpdate({ identifier, installedVersion, version, sourceURL ... })
    else already announced
        EM-->>EM: nothing (once across restarts and concurrent checks)
    end
```
