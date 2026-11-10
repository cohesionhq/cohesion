# Product direction

What Cohesion is for: who it serves, what each area and surface does, and
which surface gets a feature. The rest of this repo's docs say how the code is
built and the rules it follows.

When a product question isn't answered here (who a feature is for, whether a
surface gets it, what a default should be), ask the maintainers in the issue or
pull request rather than guess. When your change moves product direction (a new
capability, a changed default, a surface gaining or losing a feature), update
this page in the same pull request and say so in its description.

## Goal, areas and surfaces

- **Goal** is the top of the product. A self-hosted, open-source
  platform for power users first, developers second and home users third. It
  sets Cohesion apart by owning the user's data, agents and actors that work
  on it with the user approving changes, and external data arriving through
  connectors and extensions. It is never the service itself (not the email
  client), never agent-only, and not built for human-to-human collaboration.
  Its principles: never lose core data (workspaces, tasks, documents) and
  tell users what gets cleaned up; nothing is done until it is polished;
  clients are good platform citizens; anyone can extend it. Its policies:
  a daemon feature the user controls at run time gets a control in the web
  UI (the CLI only where bootstrapping needs it); the iPhone app mirrors
  the knobs that are useful on the go; Cohesion and its apps are free and
  open source, nothing is gated behind a paid tier, and a feature that needs
  Cohesion Cloud is never baked into the binary. Cohesion Cloud
  (hellocohesion.cloud) hosts the same `cohesiond` for people who'd rather
  not run it.
- **Area: Tasks.** Three statuses (incomplete, completed, cancelled); a task
  sits in one folder or the inbox, with tags across folders; priority, size
  and start and due days are optional, and an unsized task never matches a
  size filter and sorts last. At the start of the day (6:00), each
  incomplete task starting that day fires `task:start` and each due that
  day `task:due`, for cues and extensions. Connector tasks are read-only; extension tasks
  may sync both ways. An agent's conversation and its forks change tasks
  directly; a job only with `cohesion:write`, otherwise it proposes.
- **Area: Databases.** A workspace's tables of typed values (text, whole
  number, decimal, currency, checkbox, date), whose columns the user adds,
  retypes and deletes at any time; a column can be a formula, a JavaScript
  function of its row. Filtering and sorting happen on the daemon, in SQL; the
  web UI keeps a table's filters and sort on the page, with no saved views. A
  row may have a document of its own, which stays out of the document list.
  Web only for now: the iPhone app has no databases.
- **Area: Agents.** One global model tier for agent conversations; jobs run
  at low, medium or high effort, which is the cost control. An agent works
  as its conversation, forks of it, jobs and cues. The conversation and its
  forks (a turn off the conversation, including an asked agent's fork when
  one agent asks another) have full rights and change the workspace
  directly. Jobs are read-only unless granted `network` or
  `cohesion:write`, and otherwise propose; proposals never expire, they
  wait for the user's answer. A cue is the JS that decides whether a job
  fires: it reads and notifies, and cannot propose. A job or cue never
  wakes a fork; the mount a fork needs isn't there for them. Hops are
  counted and logged, never limited: the global spend limits (USD per
  hour, day and month) are the only brake on a loop, and once one is
  reached jobs and `ask_agent` forks make no model calls. A broken cue says so every time it fails,
  loudly: an error in the agent's feed (no notification), with no model
  call; so does a spend limit holding an agent back. Memories are the agent's to curate, 20 per scope
  by default, and the user can review and delete them. Teams are a roster plus shared
  memory. Retention: agent messages (the model threads behind each chat) go after 14 days
  idle (the chat feed stays), and script results and background agent turns
  follow them; actor sessions go after 7, taking their images with them;
  security activity is a fixed 30 days; Rewind versions (7) and deleted
  items (30) are set under Retention too. Settings › Storage shows and sets
  these and Memory capacity (the memories an agent keeps per scope), with the
  database's size and what each area takes up of it (agent chats, actor
  sessions, documents and tasks, feeds and articles, resources, memories,
  Rewind versions, everything else and free space), drawn as the iPhone's
  storage screen draws it: one bar split by area, then a row each with its
  size and share. `cohesion export` and `import` move a server's data to
  another running server, replacing all of its data; that server keeps its
  key and paired clients, so only the devices of the old server pair again.
  It's CLI only. Grants,
  pinning and cue creation are web only.
- **Area: Actors.** An actor is an agent on one of the user's devices,
  offered by a client that is online or offline. Statuses: working, needs
  attention, idle (shown as Done) and failed; a message restarts an idle or
  failed actor session. Modes, models and efforts pass through. Users, agents and
  extensions drive actors with no caps; plan usage is shown, never
  enforced; the latest agent to touch an actor session follows it. Requests wait
  until answered. The daemon pushes no needs-you alert; the iPhone app
  raises one through a Live Activity. Idle or failed, read actor sessions go after 7 days
  by default (Settings › Storage).
- **Area: Clients and devices.** The web UI and the WebUI harnesses (for
  Mac, iPad and Linux, in `cohesionhq/webshell`) are the same UI and the full
  surface; the iPhone app is the native twin; the CLI, actor
  clients and extensions are clients too. A paired client can do anything;
  revoking evicts its sessions and drops its push registrations. Clients
  keep no data and need the daemon, which is meant to be reachable publicly;
  data never goes through the cloud, which only relays pushes. Last write
  wins. How the daemon and its clients change together is under
  "Compatibility" below.
- **Surface: WebUI.** Every feature exists here first, and it is also the
  WebUI harnesses' UI. One hue (indigo) on neutral greys, amber for what
  needs the user; words come from the daemon; no tooltips; Menu, Picker,
  Select, Dialog and SidePanel each have one job. The rules live in
  `web/README.md` and the `web-ui-change` skill.
- **Surface: CLI.** A terminal client for small jobs, minimal by design.
  `cohesion api` reaches every command as JSON; a command gets a friendly
  noun or verb only if a person would use it from a terminal. Conventions
  are in "Add a CLI command" in `CONTRIBUTING.md`.
- **Surface: the iPhone app.** Cohesion for iPhone and iPad, a native app
  (in `cohesionhq/apple`), matching the web on every feature that makes sense on
  mobile, up to a 13" iPad with keyboard and trackpad, and adding what only
  a native app can (push, Live Activities, widgets, the share extension).
  Its own `CLAUDE.md` holds its rules.
- **Parity: WebUI and the iPhone app.** Each web feature that makes sense
  on mobile reaches the iPhone app too. A change to a web feature notes in
  its report or pull request whether the iPhone app needs the same change.

## Compatibility

Cohesion is in active development: the major version is `001`, and the
policy below holds until it changes.

- **Stored data is kept.** A daemon's database moves forward by a migration on
  start, and an update never asks you to start again. Core data
  (workspaces, tasks, documents) is never lost; what gets cleaned up is shown
  under Settings › Storage.
- **The wire moves with the daemon.** The web UI ships inside `cohesiond`, and
  the clients in the `cohesionhq` organization (the iPhone app, the WebUI
  harnesses, the actor clients and `cohesion-ts`) are updated alongside it. A
  protocol change may be breaking, and nothing is built or kept for an older
  client. Each breaking change is listed under its release in `CHANGELOG.md`
  with the clients it breaks. A client you write yourself should expect to
  follow the same changes.
- **The protocol number stays 1** until development settles; the web UI asks to
  be reloaded only when its number differs from the daemon's. When the project
  starts keeping older clients working, this section and the number change
  together.

## Where the rules for each live

- Daemon design, trust model, layering: `docs/design-principles.md`,
  `docs/security-model.md`, `docs/conventions.md`, `docs/architecture.md`.
- Recipes for models, commands, CLI commands and script entities:
  `CONTRIBUTING.md`.
- Web UI: `web/README.md`, the `web-ui-change` skill,
  `web/src/lib/stores/README.md`.
- Hosts of the web UI and the push path: "The web UI" and "Native clients and
  other repositories" in `docs/architecture.md`.
