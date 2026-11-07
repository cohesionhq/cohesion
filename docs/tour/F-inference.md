# Part F: Inference (how a model gets chosen and called)

Four ideas, in order of indirection:

1. An **endpoint** is *where* a model runs: a provider account (Anthropic, OpenAI, an OpenAI-compatible server), reached with an API key or by signing in through an extension's connector account.
2. An **endpoint model** is *which* model that endpoint offers, optionally linked to a **catalog model** (models.dev) for price and context size.
3. A **tier** (`agent`, `low`, `medium`, `high`) is a named slot that points at exactly one endpoint model.
4. A **session** never names a model. It asks for a tier, at the moment it starts.

## F1. The indirection chain

```mermaid
flowchart LR
    subgraph WHO["Who needs a model"]
        direction TB
        FG["Foreground agent turn"]
        BGL["Cue's fork<br/>cue effort = low"]
        BGM["Cue's fork<br/>cue effort = medium"]
        BGH["Cue's fork<br/>cue effort = high"]
        SCR["Script: intelligence.model()<br/>with no tier"]
    end

    subgraph TIERS["IntelligenceDefault (one row per tier)"]
        direction TB
        T1["tier: agent"]
        T2["tier: low"]
        T3["tier: medium"]
        T4["tier: high"]
    end

    subgraph EM["endpoint_models"]
        direction TB
        M1["claude-sonnet-5-5<br/>on endpoint Anthropic"]
        M2["claude-haiku-5-5<br/>on endpoint Anthropic"]
        M3["gpt-5 on endpoint Work OpenAI"]
    end

    subgraph EP["intelligence_endpoints"]
        direction TB
        E1["Anthropic<br/>Integrated { Messages, base_url, key }"]
        E2["Work OpenAI<br/>Integrated { Responses, base_url, key }"]
    end

    CAT["catalog_models<br/>models.dev: context size, prices,<br/>capabilities per provider model"]

    FG --> T1
    SCR --> T1
    BGL --> T2
    BGM --> T3
    BGH --> T4
    T1 --> M1
    T2 --> M3
    T3 --> M2
    T4 --> M1
    M1 --> E1
    M2 --> E1
    M3 --> E2
    CAT -. "optional link<br/>(row's own fields win)" .-> M1 & M2 & M3
```

Deleting a model or an endpoint **unsets the tiers that used it** (database cascade), so a tier can be empty; asking an empty tier fails with `NoDefaultModel`.

## F2. "User sets up inference"

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant UI as Web Settings / CLI (ai ...)
    participant D as cohesiond
    participant DB as DB worker
    participant MD as models.dev

    Note over D,MD: At boot (and on updateModelCatalog):
    D->>MD: GET catalog.json
    MD-->>D: providers -> models (context, prices, capabilities)
    D->>DB: UpdateCatalogModelsCommand (upsert catalog_models)<br/>also links endpoint_models that were waiting on a catalog row

    You->>UI: ai endpoint create anthropic --api-key ...
    UI->>D: createIntelligenceEndpoint { preset, name?, api_key }
    D->>DB: insert endpoint (unique name, case-insensitive)<br/>+ import the preset's default_model as an endpoint_model
    DB-->>UI: Invalidate Intelligence

    You->>UI: ai catalog anthropic haiku (browse)
    UI->>D: listCatalogModels { provider, query }
    D-->>UI: matching catalog rows

    You->>UI: ai models add Anthropic claude-haiku-5-5
    UI->>D: addEndpointModel { endpoint_id, model_identifier, overrides? }
    D->>DB: upsert by (endpoint, identifier), link catalog row if one exists
    Note over DB: A model the catalog does not list:<br/>you supply --name --context --input --output (USD per million tokens)

    You->>UI: ai use Anthropic claude-sonnet-5-5 (tier agent)
    UI->>D: setIntelligenceDefault { tier: agent, endpoint_model_id }
    DB-->>UI: Invalidate Intelligence
    Note over D: Existing agents pick it up on their NEXT message or run.<br/>(a changed agent-tier default also forces a fresh model thread)
```

## F3. "Which model does *this* run use?" (decision flow)

```mermaid
flowchart TD
    START["Something needs a model"] --> WHAT{"What is it?"}
    WHAT -- "chat turn / choice / reaction" --> TA["tier = agent"]
    WHAT -- "fork for cue X" --> EFF{"cue.effort"}
    EFF -- "low" --> TL["tier = low"]
    EFF -- "medium" --> TM["tier = medium"]
    EFF -- "high" --> TH["tier = high"]

    TA & TL & TM & TH --> READ["tier_model(): GetIntelligenceDefaultCommand<br/>read NOW (not cached), as a TierModel =<br/>endpoint_model + its endpoint"]
    READ --> SET{"tier has a default?"}
    SET -- "no" --> NO["SessionConstructionError::NoDefaultModel<br/>chat: 'No model is set for agents; choose one in Settings.'<br/>background: run ends `failed`, error posted to the feed"]
    SET -- "yes" --> BK{"endpoint.dialect"}
    BK -- "Messages" --> B1["MessagesBackend<br/>(Anthropic Messages)"]
    BK -- "Responses" --> B2["ResponsesBackend<br/>(OpenAI Responses, cache_key routing)"]
    BK -- "ChatCompletions" --> B3["ChatCompletionsBackend<br/>(OpenAI-compatible)"]

    B1 & B2 & B3 --> NET["POST to provider over HTTPS<br/>30 s connect, 10 min per turn,<br/>no streaming"]

    classDef bad fill:#fee2e2,stroke:#dc2626,color:#111
    class NO bad
```

## F4. One model turn, inside `Session`

`Session` is the conversation engine for *every* model use; agents are just callers with particular tools and system prompts.

```mermaid
flowchart TD
    A["Session::new(model, system prompt, tools, cache_key)"] --> B["CreateIntelligenceThreadCommand<br/>(thread row holds the system prompt)"]
    B --> C["pick backend from endpoint.dialect"]
    C --> D["send(prompt): record UserMessage in transcript + DB"]
    D --> E{"round &gt; 50 ?"}
    E -- "yes" --> ERR["IntelligenceError::TooManyRounds<br/>(looping model)"]
    E -- "no" --> F["backend.send(transcript)<br/>one HTTP call"]
    F --> G["record_cost: IntelligenceCostEvent<br/>input / output / cache tokens x model prices<br/>(skipped if the model has no prices)<br/>recorded BEFORE tools run"]
    G --> H["for each output item in the turn"]
    H --> I{"item"}
    I -- "Thinking" --> J["record (redacted thoughts kept only for their provider)"]
    I -- "Message" --> K["remember as the reply"]
    I -- "ToolCall" --> L["find tool by name -> tool.call(args)<br/>unknown name -> 'This tool is unknown.'"]
    J & K & L --> M["record each item in thread_messages"]
    M --> N["record tool responses AFTER all outputs"]
    N --> O{"stop reason"}
    O -- "ToolCalls" --> E
    O -- "Message" --> P["return the trimmed reply"]
```

Every session but the user's own foreground turn (every fork) is built `capped` with a `SpendCap`: before each `backend.send` it checks the spend limits (Settings › Agents: per hour, day and month, each rolling and unset by default), and a reached one ends the answer with `IntelligenceError::SpendLimitReached`. The agent's feed hears so once per period, not once per run. A job checks before it starts too, so a timer cue held back records nothing.

## F5. Tools available to each kind of session

```mermaid
flowchart LR
    subgraph FGT["Foreground agent session"]
        F1["code-tool<br/>(read-write JS)"]
        F2["documentation<br/>(+ feed access)"]
        F3["send_message"]
        F4["ask_choice"]
        F5["react"]
    end
    subgraph BGT["Cue's fork session"]
        B1["code-tool<br/>(read-only JS)"]
        B2["documentation"]
        B3["send_message"]
        B4["ask_choice"]
    end

    subgraph DESC["What the code-tool's description contains<br/>(this is prompt text, ~46k chars)"]
        D1["CORE script API .d.ts<br/>cohesion, agent, shared types"]
        D2["Index of loadable modules<br/>cues, network, connectors, storage ..."]
        D3["Installed services<br/>(extensions granted `service`)"]
        D4["Connected services<br/>(your connector accounts + resource kinds)"]
    end

    F1 & B1 --> DESC
    F2 & B2 -. "returns a module's declarations on demand,<br/>and the services' declarations" .-> DESC
```
