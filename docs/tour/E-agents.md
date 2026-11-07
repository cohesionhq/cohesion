# Part E: Agents

An **agent** is a persistent assistant: one standing conversation, an activity line while it works, and **cues** (hooks that wake it up in the background). It is *not* a long-running process. Between events it is just rows in the database; a thread to a model is built when something needs it.

Two modes, with deliberately different powers:

| | Foreground (chat) | Background (a cue's run) |
|---|---|---|
| Started by | you: message, choice pick, reaction | a hook event whose cue predicate returns truthy |
| Model | the `agent` tier's default | the tier named by the cue's `effort` (`low` / `medium` / `high`) |
| Script role | `AgentRole::Turn(User)`: read-write `cohesion`, `network`, `connectors`, `storage`, `agent` | `AgentRole::Turn(Event)`: the same tools, read-only `cohesion`, **no `network`**, no cues or memories set |
| May change data | yes, on the strength of your message | **no**: reads, messages you, notes, and asks with `ask_choice` before any change |
| Can set cues / remember | yes | no (reads memories only) |

## E1. Anatomy of an agent (what is stored, who uses it)

```mermaid
flowchart TB
    subgraph ROWS["Rows in the database"]
        direction TB
        AG["intelligence_agents<br/>name, color, emoji, voice,<br/>role, directive, team,<br/>activity (while working)"]
        TEAM["intelligence_agent_teams<br/>title"]
        MSG["intelligence_agent_messages<br/>the feed: text, forward, codeExecution,<br/>error, documentation, choice, memory,<br/>askedAgent, askedByAgent<br/>+ reaction, sender, excerpt"]
        ATT["intelligence_agent_attachments<br/>base64 images on a message"]
        CUE["intelligence_agent_cues<br/>name, hook, script, instructions, effort"]
        RUN["intelligence_agent_runs<br/>cue name, hook, outcome, thread,<br/>tokens + cost (summed from the thread)"]
        THR["intelligence_threads +<br/>thread_messages + cost_events<br/>what the model actually saw"]
        STO["storage_entries<br/>key agent:ID: private key/value"]
        MEMS["memories<br/>scope: an agent's, a team's or global,<br/>at most 20 each by default; memory"]
        MARK["agent_thread_marks<br/>conversation + opening: thread, items,<br/>model, memory capacity, heard memories, latest call"]
        AG --> MSG --> ATT
        AG --> CUE --> RUN --> THR
        AG --> STO
        AG --> MEMS
        AG --> MARK --> THR
        TEAM --> AG
        TEAM --> MEMS
    end

    subgraph MEM["In memory (rebuilt from the rows when needed)"]
        direction TB
        HOST["AgentHost (one per agent, behind a Mutex)<br/>working_set: recent relevant messages<br/>session: live model thread (or None)<br/>seen_through, answered, sent, reacted"]
    end

    AG -. "AgentCoordinator.host(id)<br/>created on first use,<br/>evicted on update / delete / cue removal" .-> HOST
    MSG -. "relevant_set::load" .-> HOST
    CUE -. "listed in the status block" .-> HOST
    MEMS -. "matched to each prompt" .-> HOST
    MARK -. "a new host carries a warm thread on" .-> HOST
```

## E2. "User creates an agent"

Note what is *not* chosen at creation: the model. An agent has no model of its own; it uses whatever the `agent` tier points at *at the time of each turn*.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Web / iOS / CLI (agent create)
    participant S as Session + request handler
    participant R as Route (Database kind)
    participant W as DB worker
    participant AC as AgentCoordinator
    participant Other as Every subscribed screen

    You->>UI: name (optional), colour, emoji (optional)
    UI->>S: createIntelligenceAgent { name?, color, emoji? }
    S->>R: Command::dispatch
    R->>W: CreateIntelligenceAgentCommand
    Note over W: blank / absent name -> pick an unused first name like "Ada"
    W->>W: INSERT intelligence_agents<br/>activity = null (not working)
    W-->>Other: Invalidate Agent(id)
    W-->>S: the new IntelligenceAgent row
    S-->>UI: Reply
    Note over AC: No AgentHost exists yet.<br/>It is created lazily on the first message.
    Note over You,W: No model is stored on the agent.<br/>If the `agent` tier has no default, the FIRST message fails with<br/>"No model is set for agents, choose one in Settings."
```

## E3. "User sends a message": the foreground turn

The request returns immediately (`AcceptedResponse`); the turn runs on its own task. Clients follow it through invalidations: the activity is set while it works, messages land in the feed as the agent's tools post them.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Client
    participant RT as agent_routes
    participant AC as AgentCoordinator
    participant H as AgentHost (locked)
    participant DB as DB worker
    participant SE as Session (cohesion-intelligence)
    participant LLM as Backend (HTTP provider)
    participant JS as ScriptEngine foreground_agents

    You->>UI: "watch my inbox folder for new tasks"
    UI->>RT: sendAgentMessage { agent_id, message, attachments? }
    RT->>AC: send_message
    AC->>AC: queue(): poll the host lock ONCE to take a FIFO place<br/>(turns run in the order requests arrived)
    AC->>DB: CreateUserAgentMessageCommand (+ attachments)
    DB-->>UI: Invalidate Agent(id) (your message appears)
    AC->>AC: spawn(job) on its own task, counted in WorkTracker
    AC->>AC: activity.begin -> activity = "Thinking"
    AC-->>RT: returns once the activity is set
    RT-->>UI: AcceptedResponse
    Note over AC,H: --- the turn, in the background ---
    AC->>H: answer_message (lock acquired when its turn comes)
    H->>DB: tier_model(Agent): current default endpoint_model + endpoint
    alt default changed since the thread began OR no thread OR working set mostly stale OR cache expired OR prompt over 100K tokens
        H->>DB: relevant_set::load + list cues
        H->>DB: ListMemoriesCommand: every memory it sees (its own, its team's, global; at most 20 each by default)
        H->>SE: Session::new(model, ForegroundAgent prompt + # Your team / # Other agents + # Memories, tools, cache_key)
        SE->>DB: CreateIntelligenceThreadCommand
        H->>H: prompt = status block (time, cues, recent messages)
    else live thread still fresh
        H->>DB: ListMemoriesCommand
        H->>SE: hear_memories: what changed since the thread last heard (its system prompt never changes)
        H->>H: prompt = "# Memories changed" + "# Since your last reply" catch-up + your message
    end
    H->>SE: send(prompt, images)
    loop up to 50 rounds (MAX_ROUNDS)
        SE->>LLM: transcript + tool definitions
        LLM-->>SE: assistant text and/or tool calls
        SE->>DB: record every transcript item + IntelligenceCostEvent
        opt tool call: code-tool
            SE->>JS: foreground_agent_run(agent_id, code)
            JS->>DB: read / write commands via GuardedDatabaseContext
            JS-->>SE: output + AgentEffects (cue, removeCue, follow)
            SE->>DB: record codeExecution message, apply effects
        end
        opt tool call: send_message / ask_choice / react
            SE->>DB: CreateAgentMessageCommand (agent's text, choice, or reaction)
            DB-->>UI: Invalidate Agent(id) (appears live in the feed)
        end
        opt tool call: remember / forget
            SE->>DB: CreateMemoryCommand / DeleteMemoryCommand + a memory message in the feed
        end
        opt tool call: memories
            SE->>DB: ListMemoriesCommand
        end
        opt tool call: documentation
            SE-->>LLM: declarations of a script module on demand
        end
    end
    H->>AC: turn ends
    AC->>DB: activity.end -> activity = null (or the older work's)
    DB-->>UI: Invalidate Agent(id)
    Note over SE: The model's FINAL text reply is never shown.<br/>The user sees only what send_message / ask_choice / react posted.
```

## E4. What the model "remembers": context, memories and the rolling thread

The model's thread is disposable. What survives is the feed (database) plus memories and `storage`.

```mermaid
flowchart TD
    NEW["A turn begins"] --> CHK{"Rebuild the model's thread?"}
    FIND["Every memory the agent sees:<br/>its own, its team's, then the global ones,<br/>at most 20 in each scope by default,<br/>under # Memories at the end of the system prompt"]
    CHK -- "no thread yet" --> REB
    CHK -- "agent tier's default model changed<br/>(different endpoint_model id)" --> REB
    CHK -- "most of the working set is stale<br/>(share &gt;= 0.8)" --> REB
    CHK -- "the provider's prompt cache has expired<br/>(the endpoint's cache minutes since the last model call)" --> REB
    CHK -- "the last prompt passed 100K tokens<br/>(as the provider counted them)" --> REB
    CHK -- "none of these" --> LIVE["Keep the live thread and its system prompt as they are.<br/>Prompt = catch-up:<br/># Memories changed (remembered or forgotten since it last heard),<br/>background messages not yet seen (up to 100)<br/>and choices answered since,<br/>then the new text"]

    REB["Start a fresh Session (new IntelligenceThread)"] --> SCORE
    SCORE["Score every recent message:<br/>relevance = index factor x age factor<br/>index: decays with depth, floor 0.38, half-life 40 messages<br/>age: half-life 100 h with power 0.45<br/>keep if relevance &gt;= 0.44"] --> STATUS
    STATUS["First message = status block<br/>- current time<br/>- each cue (name, hook, effort, instructions)<br/>- the surviving working set"] --> SEND["session.send(prompt)"]
    LIVE --> SEND
    FIND -.-> LIVE
    FIND -.-> REB

    SEND --> FORGET["Anything older than the block<br/>is gone from the model's context for good"]
    TOOLS["remember / forget (conversation only)<br/>memories (runs too)"] -. "what the user said, in the agent's words:<br/>personal, team or global (at most 20 each by default)" .-> FIND
    STORE["storage (key/value, agent:ID)"] -. "data the agent wants to keep:<br/>tallies, what a cue already handled" .-> SEND
```

## E5. An agent sets a cue ("what wakes it up")

A cue is a (hook, predicate-script, instructions, effort) tuple. The agent writes it from inside the chat by running a script that calls `agent.cue(...)`.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant Chat as Foreground turn
    participant LLM as Model
    participant JS as Script (foreground pool)
    participant AE as AppliedEffects::foreground
    participant DB as DB worker

    You->>Chat: "ping me when an urgent task shows up"
    Chat->>LLM: prompt
    LLM->>Chat: tool call: documentation { cues }
    Chat-->>LLM: declarations of `cues` module (HookKind, agent.cue ...)
    LLM->>Chat: tool call: code-tool
    Note over LLM,Chat: agent.cue({<br/>name: "Urgent tasks",<br/>hook: "task:create",<br/>code: (task) => task.priority === TaskPriority.urgent,<br/>instructions: "Tell the user which task and why it matters",<br/>effort: "low" })
    Chat->>JS: foreground_agent_run
    JS-->>Chat: output + [Cue] effects (nothing written yet)
    Chat->>AE: apply effects after the script has finished
    AE->>DB: SetAgentCueCommand (unique by name per agent = replace)
    DB-->>Chat: Invalidate Agent(id)
    AE-->>LLM: notes: 'Cue "Urgent tasks" set on task:create, at low effort.'
    LLM->>Chat: send_message "Done: I'll wake when an urgent task is created."
```

## E6. A cue fires: a fork started by an event

This is the whole "something happened, so the agent wakes and acts" path. Cheap first (a JS predicate on a small pool), expensive only if it passes (an LLM call).

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant W as DB worker
    participant HM as HookManager
    participant HR as HookReceiver
    participant AC as AgentCoordinator.fire
    participant JS as ScriptEngine (cue evaluation on background pool)
    participant BR as run_background
    participant DB as DB (commands)
    participant SE as Session
    participant LLM as Model of the cue's effort tier
    participant BJ as cues pool

    You->>W: create task "Pay rent" priority urgent
    W-->>HM: TaskCreated { task, workspace } (after commit)
    HM->>HR: queued hook event task:create
    HR->>HR: extensions with task:create ON run first
    HR->>AC: fire(task:create, [Task args], trace)
    AC->>DB: ListAgentCuesForHookCommand(task:create)
    loop each cue
        AC->>AC: skip if trace.is_from_agent(cue.agent_id)
        AC->>JS: evaluate_cue(agent, cue.script, args)
        JS-->>AC: CueEvaluation { fired, subject, payload }
        Note over AC,JS: predicate returns true -> payload is the whole subject.<br/>Any other truthy value -> that value is the payload (saves tokens).<br/>Falsy -> nothing happens.
        alt fired
            AC->>AC: WorkTracker.begin (taken BEFORE spawn so shutdown waits)
            AC->>BR: tokio::spawn(run_background)
        end
    end
    Note over AC: fire() returns once cues are evaluated.<br/>Model calls happen on spawned tasks, so the hook queue never waits on a model.
    BR->>DB: ListMemoriesCommand (its own, its team's and the global ones)
    BR->>DB: tier_model(cue.effort -> low|medium|high)
    BR->>SE: fork of the conversation (cue effort's model, same prompt and tools)
    Note over BR,SE: the conversation's tools, but writes throw,<br/>no network, no cues or memories set
    BR->>DB: CreateAgentRunCommand (outcome running, thread id)
    BR->>SE: send(wake message: cue instructions + hook + payload)
    loop model turns
        SE->>LLM: transcript + tools
        LLM-->>SE: tool calls
        opt code-tool
            SE->>BJ: traced_foreground_agent_run(code, Event)
            BJ->>DB: READ-only commands
            BJ-->>SE: output + effects
            Note over SE: writes throw; it asks with ask_choice instead.
        end
        opt send_message
            SE->>DB: message to the feed (trace continues from the event)
        end
    end
    BR->>DB: FinishAgentRunCommand(completed | failed)
    Note over DB: failure -> run `failed` + an `error` message posted in the feed
    DB-->>You: Invalidate Agent(id) (phone / web shows the message)
```

## E7. Asking first: how a turn the user didn't start changes anything

A fork a cue started only reads. To change something it asks with `ask_choice`, putting in `context` what to do on each answer; your pick starts a turn of the conversation, started by you, which makes the change.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant F as Cue's fork (read-only)
    participant DB as DB
    participant UI as Client
    participant AC as AgentCoordinator
    participant H as AgentHost (the conversation)

    F->>DB: ask_choice("Archive 14 completed tasks?", [Yes, No], context)
    DB-->>UI: Invalidate Agent(id)
    UI-->>You: the question, with lettered answers
    You->>UI: picks "Yes"
    UI->>AC: respondToAgentChoice { message_id, option }
    AC->>H: a turn under # User's pick, started by you
    H->>DB: makes the change (full rights)
```

## E8. State machines

```mermaid
stateDiagram-v2
    direction LR
    state "Agent activity" as AGS {
        [*] --> none: created
        none --> working: any turn or fork begins
        working --> working: a model call (rotating word) or a tool call (its label, a script's title)
        working --> none: all work ended
    }
```

```mermaid
stateDiagram-v2
    direction LR
    [*] --> running: CreateAgentRunCommand
    running --> completed: session.send ok
    running --> failed: model / setup error, or panic
    failed --> [*]: error message posted to feed
    completed --> [*]
```

## E9. Choices and reactions (the lightweight ways to talk to an agent)

```mermaid
flowchart TD
    subgraph AGENTSIDE["Agent -> you"]
        M1["send_message<br/>short text, posted immediately<br/>(several per turn, like texting;<br/>reply_to quotes the message it answers)"]
        M2["ask_choice<br/>a question with lettered answers A, B, C"]
        M3["react(messageId, emoji)<br/>acknowledge instead of replying"]
        M4["send_note<br/>for its own later turns, nobody told"]
    end
    subgraph YOUSIDE["You -> agent"]
        U1["Message (+ images)"]
        U2["Pick an option on a choice"]
        U3["React to an agent message"]
    end
    U1 --> T1["foreground turn<br/>MUST call send_message or react"]
    U2 --> T2["foreground turn under<br/># User's pick<br/>(no message of yours is posted)"]
    U3 --> T3["foreground turn under<br/># User's reaction<br/>usually ends having sent nothing"]
    T1 & T2 & T3 --> AGENTSIDE
    AGENTSIDE --> HK["each posted message raises AgentMessageCreated<br/>=&gt; agent:message hook (extensions, other agents' cues)<br/>reactions raise agent:reaction"]
    HK --> PUSH["e.g. the iOS companion extension<br/>can send a push notification"]
```
