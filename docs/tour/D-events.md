# Part D: Events, hooks and time

Nothing in Cohesion polls your data. When a command commits, it can raise a *database event*; the **HookManager** turns that (and wall-clock ticks) into *hook events* on one ordered queue; the **HookReceiver** runs them one at a time, first in extensions, then in agents' cues. This is the "what wakes it up" for everything automatic.

## D1. The event bus

```mermaid
flowchart TB
    subgraph SOURCES["Where hook events come from"]
        direction TB
        S1["A committed DB command raises DatabaseEvents<br/>TaskCreated, TaskStatusChanged, TaskRenamed,<br/>TaskMoved, TaskPriorityChanged, TaskDescriptionChanged,<br/>TaskStartDateChanged, TaskDueDateChanged,<br/>TaskTagged, TaskDeleted,<br/>TaskFolderCreated/Deleted, WorkspaceCreated/Deleted,<br/>JournalSaved, DocumentSaved,<br/>ResourceImported, ResourceUpdated, ResourceDeleted,<br/>FeedArticleAdded, FeedDeleted,<br/>ArticleRead,<br/>AgentMessageCreated, AgentMessageReacted,<br/>AgentTurnStarted/Ended"]
        S2["Day clock (wall-clock read each minute)<br/>day:start 06:00 (then task:start, task:due), day:end 23:00, timer"]
        S3["Daemon boot / grant change<br/>launch (scoped to one extension)"]
        S4["Extension update check<br/>extension:update (once per announced version)"]
    end

    S1 -- "DatabaseEventListener::send" --> HM
    S2 --> HM
    S3 -- "send_scoped(extension_id)" --> HM
    S4 --> HM

    HM["HookManager<br/>event_hook(kind) -> HookEvent<br/>each queued event = WorkGuard (counted as work)"]
    JS["JournalSaved and DocumentSaved are special:<br/>do not queue now. Starts / restarts a<br/>120 s settle timer per document (see D2)"]
    S1 -.-> JS -- "after 120 s of no saves" --> HM

    HM == "unbounded mpsc channel<br/>strictly ordered" ==> HR

    HR["HookReceiver (one task, serial)"]
    HR --> DEPTH{"trace.depth() &gt; 3 ?"}
    DEPTH -- "yes" --> DROP["log 'Hook loop terminated'<br/>drop event"]
    DEPTH -- "no" --> EXT["Step 1: dispatch_hook to extensions"]
    EXT --> Q1["GetExtensionsWithHookCommand(kind)<br/>= extensions with this hook switched ON"]
    Q1 --> SKIP{"event written by this<br/>extension itself?<br/>trace.is_from_extension"}
    SKIP -- "yes" --> NEXT["skip it"]
    SKIP -- "no" --> CALL["ExtensionManager.hook(id, kind, ...)<br/>runs onTaskStatus(task) etc. in that<br/>extension's persistent sandbox"]
    CALL --> AG
    NEXT --> AG
    AG{"event scoped to one<br/>extension (launch)?"}
    AG -- "yes: agents don't see it" --> END(["next event"])
    AG -- "no" --> FIRE["Step 2: AgentCoordinator.fire(kind, args, trace)<br/>AFTER extensions, so cues see what<br/>extensions changed"]
    FIRE --> END

    classDef src fill:#f3f4f6,stroke:#6b7280,color:#111
    classDef mgr fill:#ede9fe,stroke:#7c3aed,color:#111
    class S1,S2,S3,S4 src
    class HM,HR,EXT,FIRE,CALL mgr
```

## D2. Journal "settling" (why `journal:settled` is delayed)

A journal is saved on every pause while you type. Waking an agent on each save would be absurd, so the save only restarts a timer. Any other document does the same with `DocumentSaved` and `document:settled`.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Editor (web / iOS / CLI)
    participant W as DB worker
    participant HM as HookManager
    participant HR as HookReceiver

    You->>UI: type in today's journal
    UI->>W: EditJournal or AppendToJournal
    W-->>HM: DatabaseEvent JournalSaved { document, workspace }
    HM->>HM: settling[document] = timer(120 s), cancel the previous one
    You->>UI: type some more
    UI->>W: save again
    W-->>HM: JournalSaved
    HM->>HM: abort old timer, start a new 120 s timer
    Note over HM: ...120 s pass with no save...
    HM->>HR: queue JournalSettledHook (with the LAST save's trace)
    HR->>HR: extensions' onJournalSettled(document), then agents' cues on journal:settled
    Note over HM: On shutdown, every settled hook still waiting is queued immediately (settle_all),<br/>so a pending reaction isn't lost.
```

## D3. The day clock

```mermaid
flowchart TD
    A["tokio sleep until the next minute<br/>(then read the clock again:<br/>a sleeping laptop pauses tokio timers)"] --> B["now = Zoned::now() (local wall clock)"]
    B --> C["passed(last, now):<br/>which of day:start (06:00), day:end (23:00),<br/>and the latest minute start have a time<br/>after 'last' and no later than 'now'?"]
    C --> D["queue each one, oldest first;<br/>after day:start, task:start then task:due<br/>for each incomplete task starting or due that day"]
    D --> E["last = now"]
    E --> A
    C --> F["Slept through 3 hours?<br/>timer fires ONCE for the latest minute<br/>and carries since = the last minute it fired for,<br/>so a cue can still catch a missed 9:00 reminder"]
    G["Daemon stopped / shutting down:<br/>times passed in that gap do not fire"] -.-> C
```

## D4. Loop protection: why agents and extensions can't wake each other forever

Every database event carries a **trace**: a depth counter plus an origin (which extension or which agent wrote). A write made while handling an event continues that trace. The trace also counts **hops**, the forks a chain was handed through, separately from the depth that limits hooks (3). An actor-session follow-up is a hop, and so is a question between agents: its fork carries its asker's trace one hop further. Hops are logged, never limited; the spend limits hold a chain that runs on.

```mermaid
flowchart LR
    U["You create a task<br/>fresh trace, no origin"] --> E1["TaskCreated event<br/>depth +1"]
    E1 --> X["Extension A hook runs,<br/>writes task metadata"]
    X --> E2["TaskDescriptionChanged ...<br/>depth +1, origin = extension A"]
    E2 --> SK1["Extension A: SKIPPED<br/>is_from_extension(A)"]
    E2 --> CUE["Agent cue on this hook fires,<br/>its fork posts a message"]
    CUE --> E3["AgentMessageCreated<br/>depth +1, origin = agent 7<br/>(a cue's fork counts one level deeper)"]
    E3 --> SK2["Agent 7's own cues: SKIPPED<br/>is_from_agent(7)"]
    E3 --> OTHER["Agent 8 cue on agent:message fires"]
    OTHER --> E4["AgentMessageCreated<br/>depth +1 ..."]
    E4 --> STOP["once depth &gt; 3 (MAX_HOOK_DEPTH)<br/>'Hook loop terminated', event dropped"]

    classDef stop fill:#fee2e2,stroke:#dc2626,color:#111
    class SK1,SK2,STOP stop
```
