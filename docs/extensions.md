# Extensions

An extension is a single JavaScript or TypeScript file that `cohesiond` loads
once and keeps running. It declares a manifest, with the permissions it needs
and the hooks it wants, then defines hook functions. The daemon calls a hook
when the matching event happens: when the daemon launches, when a task is created, when a task's
status, folder, priority, size, description or dates change, or when a journal
entry has settled after an edit. It can also call an extension's functions
on a timer, an extension can act as a [registry](#registries) that
offers other extensions to install, it can answer
[requests from clients](#service-requests), it can sign the user in to an outside
service and keep their [accounts](#accounts) there, and it can act as a
[connector](#connectors) that imports items from those accounts as resources.

Extensions use the same script API as `cohesion run` and the LLM code tool
(see `cohesion script new` for the `.d.ts`). The differences are which globals
they get and how long their sandbox lives.

## A minimal extension

```js
function manifest() {
  return {
    identifier: "com.example.done-stamp",
    name: "Done stamp",
    version: 1,
    description: "Stamps a task's completion time in its metadata.",
    author: "Example Co.",
    permissions: ["cohesion:write"],
    hooks: ["task:status"],
  };
}

function onTaskStatus(task) {
  if (task.status === TaskStatus.completed && task.metadata("done_stamp.at") === null) {
    task.setMetadata("done_stamp.at", new Date().toISOString());
  }
}
```

The extension stamps metadata, so it holds `cohesion:write`. Without it, the
`setMetadata` call throws a read-only `Error` and the hook fails.

Install or update it:

```sh
cohesion extension install done-stamp.js
```

It installs with `cohesion:write` and `task:status` both **off**. Turn them
on under Extensions → Installed in the web UI, or with the API (see
[Permissions](#permissions)); until then the hook never runs.

## The manifest

`manifest()` is a global function that takes no arguments and returns an
`ExtensionManifest` (declared in `cohesion.d.ts`, so a TypeScript extension
can write `function manifest(): ExtensionManifest`):

| Field | Type | Meaning |
|---|---|---|
| `identifier` | string | Stable, unique key for the extension. Installing a file with an identifier that already exists **replaces** that extension. Use a reverse-DNS name. |
| `name` | string | Display name. |
| `version` | number | A whole number, raised with each release. A registry listing a higher one offers it as an update, and for an `oauth` extension it is also its connector definition's version. |
| `description` | string | Optional. What the extension does, in a sentence or two. Shown where extensions are listed. |
| `author` | string | Optional. Who wrote the extension. |
| `icon` | string | Optional. Shown beside the extension, and beside the accounts it serves. See [Icon](#icon). |
| `permissions` | string[] | What the extension may do. See [Permissions](#permissions). An unknown string rejects the install. |
| `hooks` | string[] | Which events the extension wants to be called for. See [Hooks](#hooks). An unknown string rejects the install. |
| `service` | object | Optional. The functions a service extension serves at `serviceRequest`, with their TypeScript types; needs `service` or `service:isolated`. See [Service requests](#service-requests). |

At install time the daemon evaluates the whole file in an **isolated
sandbox** that has no globals except entity classes and enums. Then it calls
`manifest()`. Code at the top level of the file therefore runs twice: once
during the install (without `cohesion`, `network` or `log`), and again when
the extension's real sandbox starts. Keep top-level code free of side
effects, and do your setup in `onLaunch`.

The install fails with a validation error if the file throws, if
`manifest` is missing, or if the returned object doesn't match the shape
above.

### Icon

`icon` is a `data:image/…` URL embedded in the script, or an `https://` URL.
Aim for about 100×100 pixels; a PNG or a small SVG is plenty. The icon can be
at most 64 KB. Anything else, a longer string or another kind of URL, is
dropped without an error, and the UI shows the name's initial instead. A
reinstall replaces it.

## Permissions

| Permission | Grants |
|---|---|
| `cohesion:read` | The `cohesion` global, read-only (workspaces, documents, tasks, search, metadata). |
| `cohesion:write` | The same `cohesion` global with writes (`createTask`, `rename`, `delete`, `setMetadata`, task setters such as `status`, `title`, `priority`, `size`, `description`, `startDate` and `dueDate`, `journal()`). Also makes the entity passed to a hook writable. |
| `network` | The `network` global (`network.fetch`). |
| `timer` | The `timer` global (`timer.every`). See [Timers](#timers). |
| `registry` | No global. Lets the daemon call the extension's `registryResults(query)` when the user searches for extensions, and `registryLookup(identifiers)` when it checks for updates. See [Registries](#registries). |
| `oauth` | The `accounts` global (`accounts.all()`). Lets the daemon call the extension's sign-in functions (`serviceDescription`, `authenticationURL`, and so on) so the user can add accounts at a service. See [Accounts](#accounts). |
| `connector` | No global. Lets the daemon call the extension's import functions (`getOrganizationUnits`, `listResources` and `getResource`) to import an account's items as resources. Needs `oauth`: a manifest listing `connector` without it fails to install. See [Connectors](#connectors). |
| `service` | No global. Makes the extension a service: lets clients (the CLI, the web UI, the apps) call the extension's `serviceRequest(name, args)` with the `callExtension` command and receive what it returns, and, once its manifest declares a `service` block, scripts, agents and extensions holding `services` call it with `services.use(identifier)`. See [Service requests](#service-requests). |
| `service:isolated` | No global. Like `service`, but for clients only: agents, scripts and other extensions can never `services.use` it. A manifest lists `service` or `service:isolated`, never both; listing both fails to install. See [Isolated services](#isolated-services). |
| `services` | The `services` global, through which the extension calls other installed services' declared functions with `services.use(identifier)`. See [Calling a service from a script](#calling-a-service-from-a-script). |
| `usage` | The `usage` global (`usage.total()`, `usage.byModel()`): how many tokens Cohesion's models used and what they cost. See [`usage`](#usage-needs-usage). |
| `intelligence` | The `intelligence` global: create and delete endpoints and add and remove their models. These are settings, not data, so `cohesion:write` doesn't allow them and this permission doesn't allow changing data. See [`intelligence`](#intelligence-needs-intelligence). |
| `notifications` | The `notifications` global (`notifications.send(title, message)`): sends the user a notification; it needs no `cohesion:write`. See [`notifications`](#notifications-needs-notifications). |
| `actors` | The `actors` global: reads the actors on the user's devices and their sessions, and starts, replies in and stops sessions; it needs no `cohesion:write`. See [`actors`](#actors-needs-actors). |

Every extension also gets `log` and `storage` (see
[`storage`](#storage-no-permission)), along with the entity classes (`Workspace`,
`Document`, `Task`, `SearchResult`, `Resource`, `Account`, `Agent`) and the enums (`TaskStatus`,
`NetworkMethod`, and so on).

**Everything starts off.** Installing an extension, from any source, records
the permissions and hooks its manifest lists, all switched off. The user
turns each one on, under Extensions → Installed in the web UI or with the
`setExtensionPermission` and `setExtensionHook` API commands
(`listExtensionPermissions` and `listExtensionHooks` show the current state;
`setExtensionPermission` takes `{ extension_id, permission, enabled }` and
`setExtensionHook` takes `{ extension_id, hook, enabled }`):

```sh
cohesion api '{"type":"setExtensionPermission","args":{"extension_id":3,"permission":"network","enabled":true}}'
```

A change applies **immediately**: the daemon discards the extension's running
sandbox, and the next call starts a fresh one with the globals its grants now
allow. Since timers die with the sandbox, the daemon then calls `onLaunch`
again if the `launch` hook is on; enabling `launch` calls it straight away.
Several changes in quick succession call `onLaunch` once, not once each,
and it doesn't run at all if `launch` is off by the time its turn comes.

> **Note:** without `cohesion:write`, an extension has read-only access. Any
> call that would change data throws an `Error` reading "This script has
> read-only access to Cohesion; the `cohesion:write` permission is required to
> change data." That includes the write methods on a hook's `Task` argument.
> `journal()` gets or creates the day's entry, so it counts as a write; use
> `getJournal()` to only read. Changing intelligence settings needs
> `intelligence` instead, and throws "This script can't change intelligence
> settings; the `intelligence` permission is required." without it.

## Hooks

A hook is a global function with a fixed name. The daemon calls it only if
the extension lists the matching hook in its manifest and has it enabled. If
an extension enables a hook but doesn't define the function, the call fails
with "script does not define a function named …" and the failure is logged.

| Function | Hook | Called | Argument |
|---|---|---|---|
| `onLaunch()` | `launch` | Once when the daemon starts, when the hook is switched on, and again whenever the extension's sandbox restarts while the hook is on: on an update, or when any of its permissions or hooks changes. Restarts made while an `onLaunch` is still waiting to run share that call. | none |
| `onDayStart()` | `day:start` | Every day at 6:00 in the daemon's local time. If the computer is asleep then, it runs once on waking; if the daemon isn't running, that day's call is skipped. | none |
| `onDayEnd()` | `day:end` | Every day at 23:00 in the daemon's local time, under the same rules as `onDayStart`. After a sleep that spans both times, `onDayEnd` runs before `onDayStart`. | none |
| `onTimer(timer)` | `timer` | At the start of every minute in the daemon's local time, under the same rules as `onDayStart`: after a sleep it runs once, for the latest minute. At 6:00 and 23:00 it runs after the day hook. | `{ time, hour, minute, since }`: when the minute began (milliseconds since the epoch), the local hour (0–23) and minute (0–59), and when the minute it last ran for began, which is more than a minute back after a sleep |
| `onExtensionUpdate(update)` | `extension:update` | When an update check (at daemon start, daily, or Check now) first finds a registry offering an installed extension at a higher version not yet announced: once per extension and offered version, across restarts. It fires again only when the registry moves on to a version higher still. | `{ extensionId, identifier, name, installedVersion, version, sourceURL }`: the installed extension's id, manifest identifier, name and version, the version offered, and the URL serving its script |
| `onTaskCreate(task)` | `task:create` | After any task is created, whether by the CLI, a script, the LLM, or another extension. | `Task` |
| `onTaskStatus(task)` | `task:status` | Once per task whose status actually changed, including each task a bulk `changeTasksStatus` changed. Setting a task to the status it already has, or naming an id that doesn't exist, fires nothing. | `Task` |
| `onTaskRename(task)` | `task:rename` | After a task's title actually changes. Setting the title it already has fires nothing. | `Task` |
| `onTaskMove(task)` | `task:move` | After any task is moved to a folder or to the inbox, by any caller. A move to the folder the task is already in fires too. | `Task` |
| `onTaskPriority(task)` | `task:priority` | After a task's priority actually changes. Setting the priority it already has fires nothing. | `Task` |
| `onTaskSize(task)` | `task:size` | After a task's size actually changes, including being cleared. Setting the size it already has fires nothing. | `Task` |
| `onTaskAssigned(task)` | `task:assigned` | After a task's assignee actually changes: assigned to the user, an agent or a contact, moved to someone else, or unassigned. Assigning the one it already has fires nothing, and so does deleting the agent or contact, which unassigns its tasks quietly. Assigning an agent (by anyone but that agent itself) also wakes a fork of its conversation to work on the task, with no cue. | `Task` |
| `onTaskDescription(task)` | `task:description` | After a task's description actually changes, including being cleared. Setting the same text fires nothing. | `Task` |
| `onTaskStartDate(task)` | `task:startdate` | After a task's start date actually changes, including being set or cleared. Rescheduling that leaves the start date alone fires nothing. | `Task` |
| `onTaskDueDate(task)` | `task:duedate` | After a task's due date actually changes, including being set or cleared. A reschedule that changes both dates fires `onTaskStartDate` then `onTaskDueDate`. | `Task` |
| `onTaskStart(task)` | `task:start` | Once for each incomplete task whose start date is today, right after `onDayStart` and under its rules: at 6:00, or on waking if the computer slept through it, and not at all if the daemon wasn't running then. A task given today's start date after 6:00, or one finished or cancelled by then, fires nothing. | `Task` |
| `onTaskDue(task)` | `task:due` | Like `onTaskStart`, for each incomplete task due today, after every `onTaskStart` of that morning. A task that starts and is due today fires both. | `Task` |
| `onTaskTagged(task)` | `task:tagged` | After a tag is put on a task or taken off it. Setting a tag the way it already is fires nothing. `task.tags()` lists the tags it has now. | `Task` |
| `onTaskDelete(task)` | `task:delete` | After a task is deleted, by any caller. Deleting a folder deletes its tasks without firing this; `onTaskFolderDelete` fires instead. The argument is the deleted task, as it was when deleted. | `Task` |
| `onTaskFolderCreate(folder)` | `taskfolder:create` | After any task folder is created. | `TaskFolder` |
| `onTaskFolderDelete(folder)` | `taskfolder:delete` | After a task folder is deleted, which deletes its tasks too. The argument is the deleted folder. | `TaskFolder` |
| `onWorkspaceCreate(workspace)` | `workspace:create` | After any workspace is created. | `Workspace` |
| `onWorkspaceDelete(workspace)` | `workspace:delete` | After a workspace is deleted. The argument is the deleted workspace. | `Workspace` |
| `onJournalSettled(journal)` | `journal:settled` | Two minutes after a journal entry was last saved (edited or appended to, by any caller), once per burst of edits: each save restarts the wait. Creating an empty journal by fetching it from a script isn't a save. A settled hook: see below. | `Document` |
| `onResourceImport(resource)` | `resource:import` | After a resource is imported into a workspace. | `Resource` |
| `onDocumentSettled(document)` | `document:settled` | Like `onJournalSettled`, for any document that isn't a journal: two minutes after it was last edited or appended to, once per burst of edits. Creating a document isn't a save. A settled hook: see below. | `Document` |
| `onResourceUpdate(resource)` | `resource:update` | After a refresh changes a resource's title or contents. An unchanged resource fires nothing. | `Resource` |
| `onResourceDelete(resource)` | `resource:delete` | After a resource is removed from its workspace. The argument is the resource as it was when removed; it is no longer stored, so only its fields can be read. | `Resource` |
| `onFeedArticle(article)` | `feed:article` | Once for each new article a feed refresh brings in: the hourly read, or one you ask for. Following a feed brings in its current items without firing this. | `Article` |
| `onFeedDelete(feed)` | `feed:delete` | After a feed is unfollowed. The argument is the feed as it was when unfollowed. | `Feed` |
| `onArticleRead(article)` | `article:read` | Each time an article goes from unread to read, one at a time or each article a bulk mark-read changes. Marking a read article read again, or marking one unread, fires nothing. | `Article` |
| `onAgentMessage(message)` | `agent:message` | After any message lands in an agent's conversation: the user's messages and forwards, the agent's replies, scripts it ran, modules it loaded, memories it kept or dropped, choices and failed turns, its forks' messages and failures, and the questions it asks other agents and they ask it (a question's answer coming in fires nothing). A choice picked fires nothing, nor do an agent's notes to itself and the lines saying why it woke, and an agent's cues never fire on its own conversation. A cue's fork's messages count as one level deeper than the event that woke it, so agents waking each other stop at the depth limit below. | `AgentMessage` |
| `onAgentTurnStart(agent)` | `agent:turn:start` | When an agent starts a turn answering the user: a message, a picked choice or a reaction. Forks aren't turns, and an agent's cues never fire on its own turns. | `Agent` |
| `onAgentTurnEnd(agent)` | `agent:turn:end` | When that turn ends, whether it replied, posted nothing or failed, after the messages it posted. | `Agent` |
| `onActivityStarted(activity)` | `activity:started` | When an activity starts: today, a stretch of an actor session's work. | `Activity` |
| `onActivityUpdated(activity)` | `activity:updated` | When an activity's progress, status or title changes while it goes on. Reporting the progress, status or title it already has fires nothing, and nothing fires once it has ended. | `Activity` |
| `onActivityEnded(activity)` | `activity:ended` | When an activity ends: it succeeded, failed, or was cancelled as the daemon shut down. Activities the daemon cancels on starting again after a crash fire nothing. | `Activity` |
| `onActorMoved(session)` | `actor:moved` | Each time an actor session's status actually changes: it works, needs the user, goes idle or fails, as its actor reports, or as Cohesion sets it (a message to an idle, failed or resolved session sets it working, stopping it or its client going away leaves it idle, and resolving it makes it resolved). A new session, which starts working, fires nothing, nor does leaving sessions idle as the daemon stops. | `ActorSession`, as it stood when its status changed |
| `onNotification(notification)` | `notification:send` | When a script, an extension or an agent (a fork included) calls `notifications.send`. Nothing stores it: this call, and the toast clients show, are all there is of it. Like any hook, it skips the extension that sent it, and an agent's cues never fire on its own. | `{ title, message }` |
| `onClientRevoke(client)` | `client:revoke` | Once for each client revoked: one at a time, by Revoke others…, or for not connecting as long as the security settings allow. A client is revoked once; pairing it again makes a new client. The argument is the client as it was revoked, plain data: its `id`, the `clientId` its [service requests](#servicerequest) carried, and its `name`. Use it to forget what the extension keeps for that client, such as a push registration. | `RevokedClient`: `{ id, name }` |
| `onAgentReaction(message)` | `agent:reaction` | When the user reacts to one of an agent's messages, or the agent to one of the user's, with the message as reacted (its `reaction` set). Taking a reaction back fires nothing, and an agent's cues never fire on its own conversation. | `AgentMessage` |

What callers can rely on:

- **After commit.** Hooks fire only after the change has committed. A command
  that fails fires nothing.
- **Asynchronous.** The client's request returns without waiting for hooks.
  The `Task` argument is loaded when the hook runs, not when the event
  happened. If the task changed in the meantime, the hook sees the newer
  state. If it was deleted in the meantime, the hook isn't called and an
  error is logged.
- **No previous value.** `onTaskStatus` and the other change hooks receive
  the task as it is now. To react to a particular transition, keep the
  previous value in metadata yourself.
- **Ordered and serial.** Events are handled one at a time, in the order they
  happened. `onJournalSettled` joins the queue when its wait ends, not at
  the save. For a single event, extensions run by `priority` (lower first),
  then in install order. A hook that runs slowly, such as a 20-second
  `network.fetch`, delays every hook queued behind it, for every extension.
- **Settled hooks wait for quiet.** A `:settled` hook, today
  `journal:settled` and `document:settled`, runs once a burst of changes has stopped rather than
  on each change: every change restarts its wait, so one burst calls it
  once, with the item as it ended up. A wait still running when the daemon
  stops isn't lost: the hook runs then, before the daemon exits. Settled
  hooks added later follow the same rules.
- **Isolated failures.** A hook that throws doesn't affect other extensions
  or the change that triggered it. The error is logged under the
  `extensions` module.
- **Return values are ignored.**

### Reading an agent message

Unlike the other arguments, an `AgentMessage` is plain data, not an entity:
`id`, `agentId`, `sentAt` (Unix seconds), `sender` (`"user"`, `"agent"` or
`"backgroundAgent"`, for an older cue run), `runDescription` (what woke a fork, else
`null`) and `content`. `content` is a union tagged by `kind`, so TypeScript narrows
it in a `switch`:

```ts
function onAgentMessage(message: AgentMessage) {
  const content = message.content;
  switch (content.kind) {
    case "text":          // content.text
      break;
    case "codeExecution": // content.title, content.code, content.outcome (by its status)
      break;
    case "error":         // content.error: why a turn or fork failed
      break;
    case "documentation": // content.module: the script API module the agent loaded
      break;
    case "choice":        // content.question, content.options: a question with lettered answers
      break;
    case "memory":        // content.memory, content.change: "remembered" or "forgotten"
      break;
    case "forward":       // content.note, content.item: the task, document or journal entry the user forwarded
      break;
    case "askedAgent":    // content.agent, content.question, content.reply: the agent asked another agent
    case "askedByAgent":  // ...or another agent asked it; `reply` is waiting, answered or unanswered
      break;
  }
}
```

### Hooks fire on the extension's own changes

When a hook makes the same kind of change it listens for, it triggers itself
again. For example, an `onTaskCreate` that creates a task, or an
`onTaskStatus` that sets a status, fires its own hook.

The daemon caps how deep this goes. Every event carries a depth:

- A change from a client (the CLI, a `cohesion run` script, the LLM), from
  `onLaunch`, or from a timer function is depth 1.
- A change made inside a hook is one deeper than the event that hook is
  handling.
- An event deeper than 3 is dropped. Its hooks don't run, for **any**
  extension, and the daemon logs `Hook loop terminated.` as a warning under
  the `extensions` module.

So an outside change can set off at most three levels of hooks. The change
the third-level hook made is still committed; only its hooks are skipped.
Because the cap is shared, a different extension that listens for that
change doesn't see it either.

The cap stops runaway loops; it isn't meant as control flow. A hook that
relies on it does the same work three times and hides other extensions'
hooks. Guard these cases yourself, for example with metadata:

```js
function onTaskCreate(task) {
  if (task.metadata("myext.generated") !== null) return; // created by us
  const followUp = cohesion.workspace(task.workspaceId).createTask("Follow up: " + task.title);
  followUp.setMetadata("myext.generated", "1");
}
```

This hook creates tasks and sets metadata, so it needs `cohesion:write`. The
`setMetadata` above runs after `createTask` has already committed. The
recursive `onTaskCreate` for `followUp` (depth 2) runs later, after the
current hook returns, and by then it sees the key.

## Lifetime and state

Each extension gets its own **persistent** sandbox. It is a QuickJS runtime
on a dedicated thread, created the first time one of the extension's hooks
fires. Global variables survive between hook calls, so a hook can cache data
in a global:

```js
let seen = 0;
function onTaskCreate(task) {
  seen += 1;
  log.info(`tasks created since launch: ${seen}`);
}
```

The sandbox is discarded when the daemon stops, when the extension is
reinstalled, and when one of its permissions or hooks is switched on or off. Data that must survive belongs in [`storage`](#storage-no-permission), or in metadata when it describes an item, not in globals.

Limits:

- **60 seconds per call.** The clock restarts on each hook call and on the
  initial load of the file.
- **20 MB of JS heap for the whole sandbox lifetime,** not per call. Unbounded
  caches eventually make the sandbox fail.
- **Synchronous only.** Every API call returns its result directly. There is
  no `await`, no `setTimeout` or `setInterval` (use `timer.every`, or
  [`cohesion.sleep`](#waiting-a-moment) for a short pause), no
  `console` (use `log.info`), and no modules (`import`/`export`).

### Timers

An extension with the `timer` permission can call one of its functions on a
fixed interval:

```js
function manifest() {
  return {
    identifier: "com.example.hourly-sync",
    name: "Hourly sync",
    version: 1,
    permissions: ["timer", "cohesion:read", "network"],
    hooks: ["launch"],
  };
}

function onLaunch() {
  timer.every({ every: 1, unit: TimerUnit.hour, call: "sync" });
}

function sync() {
  // runs in this extension's sandbox, an hour after onLaunch and then hourly
}
```

- `every` is a whole number of at least 1. `unit` is `"second"`, `"minute"`
  or `"hour"`. `call` is the name of a global function. It isn't checked
  when you register: a missing function fails, and is logged, on every tick.
- **The interval must be between 30 seconds and 24 hours,** inclusive.
  `{ every: 10, unit: TimerUnit.second }` and
  `{ every: 25, unit: TimerUnit.hour }` both throw a `RangeError`, as does
  an `every` below 1. Nothing is registered when it throws.
- **Register timers in `onLaunch`.** It's the only function that runs on
  every daemon start, reinstall and sandbox restart, so it's the only place a
  timer is reliably set up. `timer` doesn't grant or imply the `launch` hook; list both.
  You can call `timer.every` from any hook or timer function, for example to
  start polling only after the first `onTaskCreate`, but such a timer exists
  only from that call until the sandbox is next discarded.
- Don't call `timer.every` at the top level of the file. The install sandbox
  has no `timer` global, so the call throws and the install fails.
- The first call comes one interval after registration, not immediately.
  Call the function yourself in `onLaunch` if you want it to run at start.
- Calling `timer.every` again with the same `call` replaces that timer.
  Different names add timers. There's no way to cancel one.
- Timers aren't persisted. They're registered again on each daemon start,
  reinstall or sandbox restart, and the interval restarts each time. **A long timer may never
  fire if the daemon restarts more often than its interval.** For daily work,
  register a shorter timer and keep the last run time in metadata.
- A timer never overlaps itself. If a call is still running when the next
  tick is due, that tick is skipped, not caught up. Ticks share the
  extension's queue with its hooks, so a function slower than its interval
  delays that extension's hooks too.
- Changes a timer function makes start a fresh hook chain at depth 1 (see
  [Hooks fire on the extension's own changes](#hooks-fire-on-the-extensions-own-changes)).
- A timer call that throws is logged under the `script` module, with the
  function's name.

## Errors

A call that changes or loads data throws if the daemon rejects it. The
exception's type tells you why, and its message is the same one a CLI
client would see:

| Exception | Cause |
|---|---|
| `ReferenceError` | The item doesn't exist, or was deleted. |
| `SyntaxError` | Invalid input, such as a missing required value or a duplicate of a unique one. When one field is at fault, the message ends with ``(field `name`)``. |
| `InternalError` | A database failure. The script only sees a generic message; the details stay on the daemon. |
| `Error` | The change refers to an item that doesn't exist (for example, a folder that was deleted), or isn't permitted. A write without `cohesion:write` also throws an `Error`, with the read-only message above. |

`timer.every` throws `RangeError` for an interval out of range, and
`task.status = "…"` throws `TypeError` for an unknown status.

An exception you don't catch fails the hook or timer call and is logged.
Changes made before the throw stay committed.

## Globals

The generated `cohesion.d.ts` (`cohesion script new`) is the reference for
every method. This section covers the extension-specific parts; `accounts` is
covered under [Accounts](#the-accounts-global).

### `cohesion` (needs `cohesion:read` or `cohesion:write`)

Same as in `cohesion run` scripts: `cohesion.workspaces()`,
`cohesion.workspace(idOrTitle)`, and the methods on `Workspace`, `Document`
and `Task`. With `cohesion:read` the global is read-only; with
`cohesion:write` its write methods work too. `workspace.resources({ kind?, search? })`
lists the [resources](#connectors) imported into a workspace; they are read-only
under either permission. `cohesion.feeds()` lists the feeds followed;
`feed.articles()` lists what one brought in, each an `Article` whose `contents` is
the page's text as it was fetched, and `cohesion.searchCorpus(query)` finds articles
by their title, author or text. Articles are readable with `cohesion:read`, but
extensions can't fetch them: you and your agents can. `article.resnapshot()` throws
an `Error` reading "Fetching articles isn’t available to extensions; only you and
your agents can fetch one again." whatever the extension's permissions, `network`
included. With `cohesion:write` an extension can still `rename` an article or mark
it read. `workspace.contacts()` and `workspace.contact(id)` read the
people a workspace keeps (a `Contact`: `firstName`, `middleName`, `lastName`,
`notes`, `photo` (`{ mediaType, data }`, base64, or `null`), `createdAt`,
`modified`, and `methods`, each a `ContactMethod` with a
`kind` of `"email"` or `"phone"` and a `value`); with `cohesion:write`,
`workspace.createContact({ firstName, lastName, methods?, … })` adds one,
`contact.addMethod(kind, value)` adds a method, assigning a property of either
saves it, `method.delete()` removes a method and `contact.delete()`
soft-deletes the contact.
`task.versions()` and `document.versions()` read Rewind's history, newest first:
each row says what changed (`change`), who (`author`, `authorName`), when
(`modified`) and whether it is `current`, and `open()` loads its text. With
`cohesion:write`, `restore()` on what `open()` returns puts the item back to that
version, as a new version, so it can be undone. `workspace.deletedTasks()` and
`workspace.deletedDocuments()` list deleted items that can still come back,
through their own `versions()`. No script can change or remove a version.
`cohesion.agents()` / `cohesion.agent(id)` give an
agent's name, color, emoji, personality, verbosity and `teamId` (an `Agent`),
useful in `onAgentMessage`: `cohesion.agent(message.agentId).name`.
`cohesion.agent(id)` throws a `ReferenceError` for an id no agent has.
`cohesion.teams()` / `cohesion.team(id)` give the teams agents are organized
into (a `Team`, with `agents()`). With `cohesion:write`,
`cohesion.createTeam(title)`, `team.rename(title)`, `team.delete()` and
`agent.setTeam(teamId | null)` organize them. `cohesion.vocabulary()`
gives the words the apps show for each kind, keyed as `Hello` sends them:
`cohesion.vocabulary().activity_statuses.find(w => w.kind === activity.status)?.label`
is `"Needs you"` for a paused activity. A task's `size` is a number from 1 to 5 (or
`null`), so name it the same way: `cohesion.vocabulary().task_sizes.find(w => w.kind === task.size)?.label`. Each size
also carries a `color` (an agent color: teal, green, blue, purple, pink, none shared with priorities)
the apps draw its chip in.
`Workspace.tasks()` and `TaskFolder.tasks()` take `size` (one size or an array), and
`sort: TaskSort.size` lists smallest first with unsized tasks last.
A task's `assignee` is who it is assigned to: `{ kind: "user" }`,
`{ kind: "agent", agentId }` (look it up with `cohesion.agent(agentId)`),
`{ kind: "contact", contactId }` (one of the task's workspace's contacts; look
it up with `workspace.contact(contactId)`) or `null`. Assigning it saves
(`task.assignee = { kind: "agent", agentId: agent.id }`, `null` to unassign),
`createTask` takes `assignee` too, and an agent that doesn't exist or a contact
that isn't a live one of the workspace throws. Deleting an agent or a contact
leaves its tasks unassigned.
The entity passed to a hook
follows the same rule, whether or not the extension mounts `cohesion`: it is
writable only with `cohesion:write`.

#### Waiting a moment

`cohesion.sleep(seconds)` pauses the script, then returns. It works with
`cohesion:read` or `cohesion:write` and needs no other permission. Use it to
wait a few seconds and look again, for example before pushing a message the
user may already have read in the app:

```js
function onAgentMessage(message) {
  cohesion.sleep(5);
  // read the conversation again here and push only if it is still unread
}
```

- `seconds` is a number from 0 to 30, fractions included; 0 returns at once.
  A negative, `NaN` or infinite value, or one over 30, throws a `RangeError`.
- Sleeps count toward the call's [60 seconds](#lifetime-and-state): one that
  would pass them throws a `RangeError` before it starts.
- The sleep holds the extension's thread. The extension's other hooks, timer
  calls and service requests wait behind it, so keep pauses short and don't
  sleep in a loop to poll; use `timer.every` for that.
- Turning the extension off, reinstalling it or switching one of its
  permissions or hooks ends a sleep at once, throwing an `Error`.

### Metadata

`Workspace`, `Document` and `Task` carry string key/value pairs for scripts'
own bookkeeping, typically to map an item to an ID in an external service:

```js
task.setMetadata("todoist.id", "8812");   // saved immediately
task.metadata("todoist.id");              // "8812", or null if unset
task.setMetadata("todoist.id", null);     // removes the key

ws.tasks({ metadata: { "todoist.id": "8812" } });       // exact value
ws.tasks({ hasMetadata: "todoist.id" });                // any value
ws.tasks({ hasMetadata: ["todoist.id", "todoist.sync"] }); // all of these keys
ws.documents({ metadata: { "notion.id": "abc" } });
cohesion.workspaces({ hasMetadata: "todoist.project" });
```

- Keys and values are strings. Store other values with `JSON.stringify`.
- Keys are shared across every script and extension, so prefix them with
  your extension's name.
- `ws.tasks()` returns only incomplete tasks unless you pass `status`, even
  when filtering by metadata. Pass
  `status: ["incomplete", "completed", "cancelled"]` to search every task.
- Metadata stays on soft-deleted items and is removed when the item is
  permanently deleted.

### `network` (needs `network`)

```js
const response = network.fetch({
  url: "https://api.example.com/items",
  method: NetworkMethod.post,              // or "POST"
  headers: { Authorization: "Bearer …" },
  jsonBody: { title: task.title },         // optional; sent as JSON
});
if (response.code != 200) {
  log.info(`com.example: ${response.code} ${response.body}`);
  return;
}
const data = JSON.parse(response.body);
```

- `formBody: { grant_type: "refresh_token", … }` sends a URL-encoded form
  instead, as OAuth token endpoints take it. Sending both throws.
- `fetch` returns `{ code, headers, body }`: the HTTP status as a number,
  the response headers keyed by lowercase name (a repeated header's values
  joined with `", "`), and the body as a string: as text, or with
  `encoding: "base64"` its bytes base64-encoded, for an image or another
  binary body.
- It throws only on transport errors (DNS, connection, a 20-second timeout).
  **An HTTP error status such as 404 or 500 doesn't throw.** It's returned
  like any other response, so check `code`.
- The call blocks the extension's sandbox, and every queued hook, until the
  response arrives.
- Any host is reachable, including `localhost` and the local network.

### `timer` (needs `timer`)

`timer.every({ every, unit, call })`. See [Timers](#timers).

### `services` (needs `services`)

`services.use(identifier)` returns the functions another installed service
declares, called as `services.use("com.example.counter").read()`. See
[Calling a service from a script](#calling-a-service-from-a-script).

### `usage` (needs `usage`)

What every model call Cohesion made cost, across conversations, agents and
scripts:

```js
const month = usage.total({ days: { after: "2026-09-30" } });
month.calls;          // model calls
month.inputTokens;    // also outputTokens, cacheReadTokens, cacheWriteTokens
month.cost;           // in US dollars
usage.byModel().map(m => `${m.provider}/${m.model}: $${m.cost.toFixed(2)}`);
```

- `total()` sums every call; `byModel()` gives one entry per provider and
  model, costliest first.
- `days` takes a day, an array of days, or a `{ before, after }` range, each
  `"YYYY-MM-DD"` in the daemon's local time, like a task's dates. Left out,
  every day counts.
- A call is recorded when its provider reports what it cost or its model is
  in the catalog, which prices it.

### `notifications` (needs `notifications`)

Sends the user a notification. Cohesion keeps no copy: it raises
[`notification:send`](#hooks), for an extension that pushes notifications
to the user's devices (through ntfy, Pushover and the like, with
`network`), and the clients connected at the time show it as a toast.

```js
notifications.send("Rent is due", "The rent task is due today.");
```

```js
// A consumer: push every notification to an ntfy topic.
function onNotification({ title, message }) {
  network.fetch({
    url: "https://ntfy.sh",
    method: NetworkMethod.post,
    jsonBody: { topic: "my-topic", title, message },
  });
}
```

- `cohesion run` and agents' turns have the global
  too. An agent's messages may already reach the user's devices through an
  extension hooked to `agent:message`, so an agent sends a notification
  only for something that needs attention now.
- The hook skips the extension that sent the notification, so a consumer
  that also sends one doesn't wake itself.

### `actors` (needs `actors`)

The actors on the user's devices (such as Claude Code in a directory; see
[actors.md](actors.md)) and the sessions Cohesion hands them, as the user
works with them from the Actors page:

```js
const actor = actors.list().find((a) => a.product === "Claude Code");
const session = actors.dispatch(actor.id, { prompt: "Fix the flaky socket test." });
// Later, say from onActorMoved:
actors.messages(session.id); // the transcript, oldest first
actors.send(session.id, "Also update the docs."); // or an ActorResponse to a request
actors.stop(session.id);
session.resolve(); // the user is done with it: it leaves actors.sessions() and the lists
```

- `dispatch`, `send` and `stop` reach the actor's device, which must be
  online, and wait until it takes the call (not until the work is done);
  each throws with why not, leaving nothing behind.
- `session.resolve()` says the user is done with a session, stopping it first
  if it is going on (telling its device when it is online). A session stays
  in `actors.sessions()` and the user's lists until it is resolved, and is
  deleted once it has sat resolved for the actor session retention; sending
  it more starts it again. Like `session.markRead()`, it needs a script that
  may write (`cohesion:write` for an extension).
- `actor.usageLimits()` is how much of its plan's limits the actor has
  used (`[{ label: "5-hour limit", used: 0.84, resets_at }]`), as its
  device last reported; actors drawing on one account share it, the latest
  any of them reported. Empty while the device is offline or reports none.
  It reads what the daemon holds and reaches no device.
- `actor:moved` hands each change of a session's status to a hook, so an
  extension follows a session without polling.
- Agents' turns have the global
  too; cues don't. An agent's conversation follows the sessions it hands
  work, and hears each time one stops on its own; an extension's script
  follows its sessions through `actor:moved`.

### `intelligence` (needs `intelligence`)

The endpoints Cohesion's models are reached through, and the models each
offers:

```js
const endpoint = intelligence.createEndpoint({
  kind: "custom", identifier: "ollama",
  baseURL: "http://localhost:11434/v1", dialect: Dialect.chatCompletions,
});
const model = endpoint.addModel({ identifier: "llama3", contextSize: 8192 });
endpoint.models();        // what it offers
model.remove();
endpoint.delete();
intelligence.endpoints(); // every endpoint
```

- `createEndpoint` takes `{ kind: "provider", provider, apiKey, name? }` for
  a built-in provider, which starts with the provider's default model, or
  `{ kind: "custom", identifier, baseURL, dialect, name?, apiKey? }`.
- An `oauth` extension can instead create an endpoint that signs in with one
  of its accounts, for a service whose key expires: see
  [Endpoints on an account](#endpoints-on-an-account).
- `addModel({ identifier, name?, contextSize?, prices? })` replaces a model
  with the same identifier. Details left out come from the models.dev
  catalog. `prices` (`{ input, output, cacheRead?, cacheWrite? }`, USD per
  million tokens) overrides the catalog's: all zero for a model a
  subscription pays for.
- Deleting an endpoint or removing a model leaves any tier that used it
  without a model.
- `intelligence.endpoint(name)` and `intelligence.model(tier?)` read, as in
  `cohesion run`.

### `storage` (no permission)

The extension's own key/value store. It survives restarts, reinstalls and
permission changes, and it is removed when the extension is uninstalled. No
other extension or agent can read or change it. It needs no permission:
writes work even when `cohesion` is read-only.

```js
storage.set("sync.cursor", { page: 3, since: "2026-10-01" }); // saved immediately
storage.get("sync.cursor");   // { page: 3, since: "2026-10-01" }, or null if unset
storage.keys();               // ["sync.cursor"], alphabetical
storage.remove("sync.cursor"); // or storage.set("sync.cursor", null)
```

- Values are anything `JSON.stringify` accepts and come back parsed. A value
  it can't store, such as a function, throws `TypeError`.
- Keys are private to the extension, so they need no prefix.
- Use metadata instead to tag an item (a task's ID in an external service),
  so the tag goes with the item and can be filtered on.

Agents have a store of their own too, the same `storage` global keyed on the
agent, which their cues and forks share.

### `log`

`log.info(message)` writes a line to the daemon log under the `script`
module. The line doesn't name the extension, so include your identifier in
the message if you need to tell extensions apart.

## Registries

A registry is an extension that lists other extensions for the user to find
and install, much like a package index. It holds the `registry` permission
and must define two globals, `registryResults(query)` and
`registryLookup(identifiers)`. `registryResults` answers a search:

- `query` is what the user typed in Extensions → Browse, or `""` when the view
  opens; return what you'd feature then.
- Return an array of `RegistryResult` (declared in `cohesion.d.ts`):
  `{ manifest, sourceURL }`. `manifest` is an `ExtensionManifest` describing
  the extension, and `sourceURL` is an `http` or `https` URL serving its
  script.
- The daemon calls only registries the user has granted `registry`, each in
  its own sandbox, side by side. A registry that throws, is missing the
  function, or returns the wrong shape shows its error in its own section;
  the others still show their results.

`registryLookup` answers the update check:

- `identifiers` is an array of every installed extension's identifier.
- Return a `RegistryResult` for each identifier you list, in its current
  version, and leave out the ones you don't.
- The daemon asks every registry at start, then once a day, and when the
  user presses Check now under Extensions → Installed. Any result
  whose `manifest.version` is higher than the installed one is offered as
  an update (a lower one is never offered); with several registries, the first in priority order wins. A
  registry that fails is logged and skipped. Installing an update goes
  through the same review as a fresh install, replacing the installed
  extension and keeping its grants. The first check to find a version
  raises the [`extension:update`](#hooks) hook for it.

Browse leaves out extensions that are already installed; updates appear under
Installed instead.

A registry's `manifest` is only a claim. When the user picks a result, the
daemon downloads the script from `sourceURL` (`previewExtension`, 20-second
timeout) and reads the manifest from that code, and the user reviews the
real manifest and the code before installing. Whatever the registry says,
the installed extension starts with every permission and hook off.

A registry that serves a JSON index needs `network` too:

```js
function manifest() {
  return {
    identifier: "com.example.registry",
    name: "Example registry",
    version: 1,
    description: "Extensions published by Example Co.",
    author: "Example Co.",
    permissions: ["registry", "network"],
    hooks: [],
  };
}

// index.json: [{ "manifest": { ... }, "sourceURL": "https://..." }, ...]
function registryResults(query) {
  const response = network.fetch({
    url: "https://extensions.example.com/index.json",
    method: NetworkMethod.get,
  });
  if (response.code != 200) throw new Error(`index answered ${response.code}`);
  const q = query.toLowerCase();
  return JSON.parse(response.body).filter(({ manifest }) =>
    (manifest.name + " " + (manifest.description ?? "")).toLowerCase().includes(q),
  );
}

function registryLookup(identifiers) {
  const response = network.fetch({
    url: "https://extensions.example.com/index.json",
    method: NetworkMethod.get,
  });
  if (response.code != 200) throw new Error(`index answered ${response.code}`);
  return JSON.parse(response.body).filter(({ manifest }) =>
    identifiers.includes(manifest.identifier),
  );
}
```

From the API, `searchExtensionRegistries` takes `{ query }` and answers one
section per registry, `{ extension_id, name, results, error }`;
`previewExtension` takes `{ source_url }` and answers
`{ script, manifest, installed }`, where `installed` is the extension the
script would replace. `previewExtensionScript` takes `{ script }` and
answers the same for a script you already have, which is how the web UI
reviews a file before installing it. Install the reviewed `script` with
`registerExtension`. `listExtensionUpdates` answers the updates the last check
found, `[{ extension_id, registry_extension_id, result }]`, and
`checkExtensionUpdates` runs a check first.

## Service requests

An extension holding `service` is a service: other code sends it requests by
name and receives what it answers. It defines one global,
`serviceRequest(name, args, caller)`, and usually declares the requests it serves in
a `service` block in its manifest. Requests reach it from:

- **Clients** (the CLI, the web UI, the apps), with the `callExtension`
  command.
- **Scripts and agents**, through the `services` global:
  `services.use("<identifier>").<function>(args)`, where the identifier is
  the service extension's manifest `identifier`. A caller with read-only
  access (a background agent run, an agent's cue code, or an extension
  without `cohesion:write`) can call only the functions declared `readOnly`.
- **Other extensions** holding the `services` permission, through the same
  global.

The last two reach only services that declare a `service` block, and never a
[`service:isolated`](#isolated-services) one.

### `serviceRequest`

- `name` is a string naming the request: one of the functions the `service`
  block declares, or, without a block, any name the extension and its
  clients agree on (at most 128 characters, never empty).
- `args` is the JSON the caller sent, as the JS value it parses to, or
  `null` when it sent none. At most 64 KB as JSON.
- `caller` says who asked: `{ clientId }` when a client calls with
  `callExtension`, its id the one [`onClientRevoke`](#hooks) names when the
  client is revoked, and `null` when a script, an agent or an extension
  calls through `services`. Daemons before `client:revoke` pass no
  `caller`, so a service that also runs on them reads it as `caller?.clientId`.
- Return anything JSON can hold; the caller receives it. Returning
  `undefined` (or nothing) answers `null`. The function runs synchronously,
  so return the value itself, not a promise.
- Throw for a request you don't know or arguments you reject. A client sees
  the exception's message as a validation error, as it does when
  `serviceRequest` is missing; a script calling through `services` sees it
  thrown. The daemon logs each request's `name`, never its `args`.

Requests run in the extension's own sandbox, with whatever other globals
its grants mount, so state kept at the top level is shared with its hooks
and timers. Without `service` enabled, a call fails with "This extension
doesn't have the `service` permission enabled."

### The `service` block

The block describes the service and declares each function it serves, with
its argument and result as TypeScript types. The daemon builds what
`services.use` returns and its declarations from it, so scripts, agents
and editors know what the service offers without reading its source:

```js
function manifest() {
  return {
    identifier: "com.example.counter",
    name: "Counter",
    version: 1,
    permissions: ["service"],
    hooks: [],
    service: {
      description: "A shared counter.",
      functions: {
        increment: {
          description: "Adds `by` (default 1) and answers the new count.",
          args: "{ by?: number }",
          returns: "{ count: number }",
        },
        read: {
          description: "The current count.",
          returns: "{ count: number }",
          readOnly: true,
        },
      },
    },
  };
}

let count = 0;

function serviceRequest(name, args) {
  switch (name) {
    case "increment":
      count += args?.by ?? 1;
      return { count };
    case "read":
      return { count };
  }
}
```

| Field | Type | Meaning |
|---|---|---|
| `description` | string | Optional. What the service offers, in a sentence or two. Agents see it in their index of services. |
| `functions` | object | The functions, by name; each name is a JavaScript identifier of at most 64 characters and not a reserved word. At most 128. |
| `functions.<name>.description` | string | Optional. What the function does and what its arguments mean. |
| `functions.<name>.args` | string | Optional. The TypeScript type of the function's one argument, such as `"{ by?: number }"`. Leave it out for a function that takes none. |
| `functions.<name>.returns` | string | Optional. The TypeScript type of what it returns. Leave it out for `unknown`. |
| `functions.<name>.readOnly` | boolean | Optional, `false` by default. Whether the function only reads, changing nothing. A caller with read-only access can call only these; the others throw for it. |

The block needs `service` or `service:isolated` in `permissions`; otherwise
the install fails, as it does for a function name that isn't an
identifier, or text over its limit (1024 characters for a description,
4096 for a type). Whitespace in the text,
line breaks included, is collapsed to single spaces.

The types are TypeScript source, not checked by the daemon: it pastes them
into the declarations as written, so keep each one a single, self-contained
type. One that could reach outside its place in the signature (unbalanced
brackets, an unclosed string, a `/`, or a `,` or `;` outside any bracket) is
declared `unknown` instead. Nothing checks at runtime that `args` or the
result match their types either; validate `args` in `serviceRequest`.

Once a service declares the block, it serves **only** the functions it
declares: `callExtension` with any other name fails without reaching
`serviceRequest`, and `services.use` returns only the declared functions. A
service without the block still works through `callExtension`, but
`services.use` throws for it.

### Calling a service from a script

`services.use(identifier)` takes a service extension's manifest `identifier`
and returns a frozen object with one function per function its `service`
block declares. A function passes its argument as `args` and returns what
`serviceRequest` returns:

```js
const counter = services.use("com.example.counter");
counter.increment({ by: 2 }); // { count: 2 }
counter.read();               // { count: 2 }
```

`use` throws for an identifier that isn't installed, a service without
`service` enabled, a [`service:isolated`](#isolated-services) one, or one
without a `service` block. It looks the service up each time it is called,
so installs and permission changes show up without restarting anything.
There is no way to list the services from a script.

Every declared function is there in every context, but a caller with
read-only access (a background agent run, or an extension without
`cohesion:write`) can call only those declared `readOnly`; calling any other
throws. That limit doesn't carry over to the service: it runs with its own
grants, so a read-only function that writes anyway can. A call also can't
outlast its caller: one still running when the caller's own time runs out
throws in the caller.

The `.d.ts` that `cohesion script new` and `cohesion script typegen` write
(and the web UI's Developer tab shows) declares `services.use` over a
`ServiceMap` interface, and ends with an `interface ServiceMap` block adding
one entry per installed service, keyed by its identifier, so
`services.use("com.example.counter")` is typed. Each function not declared
`readOnly` says "Not available in read-only contexts." in its comment.

- **Agents.** An agent's code tool lists each service on one line, as
  `services.use("com.example.counter"): Counter — A shared counter.`, and
  its `documentation` tool hands out the full declarations as the
  `services` module. A turn a cue started sees the same declarations;
  calling a function not marked read-only throws there, so it asks the
  user first, and the turn their answer starts can call it.
- **Extensions.** An extension holding `services` gets the global, and calls
  other services the same way. The call runs in the other extension's
  sandbox, so a service can't be called back while it is already handling a
  request: a call to an extension already in the chain of calls (the
  extension itself, or A calling B calling A) throws instead.

### Isolated services

An extension listing `service:isolated` instead of `service` is a service
for clients only, such as the companion of an app. `callExtension` reaches
it as it reaches any service, but `services.use` throws for it and it never
appears in the declarations or in an agent's index, so scripts, agents and
other extensions can't call it. It may still declare a `service` block, to
type its functions and refuse undeclared names. A manifest lists
`service` or `service:isolated`, never both.

### From clients

`callExtension` takes `{ identifier, name, args }` and answers
`{ result }`. `identifier` is the manifest's `identifier` (1 to 256
characters), which stays the same across reinstalls and machines, unlike
the row id; an unknown one fails with a `notFound` error. `args` may be
left out.

```sh
cohesion api '{"type":"callExtension","args":{"identifier":"com.example.counter","name":"increment","args":{"by":2}}}'
cohesion extension call com.example.counter increment '{"by":2}'
```

`cohesion extension call` takes the extension's identifier and prints
`result` as JSON.

## Accounts

An extension holding `oauth` signs the user in to an outside service, such
as a notes app, and keeps the resulting accounts. It defines five global
functions with fixed names, which the daemon calls to add an account, and
may define two more, which it calls once an account is added and once one
is removed. It
gets the [`accounts`](#the-accounts-global) global to reach those accounts
itself.

Accounts are the user's sign-ins. They belong to the daemon, not to a
workspace: one per extension and remote account, added under Connectors →
Accounts. The stored credential is never sent to clients, and among scripts
only the extension that added the account reads it, through `accounts`.
Uninstalling the extension deletes its accounts.

### The sign-in functions

The interfaces are declared in `cohesion.d.ts`.

| Function | Returns | Called |
|---|---|---|
| `serviceDescription()` | `ServiceDescription` | When the extension is installed, when the user lists services to add an account at, and when an account is added. |
| `authenticationURL(pkce: PkceChallenge)` | `string` | When the user starts adding an account. The sign-in page to open. See [PKCE](#pkce). |
| `completeAuthentication(code: string, verifier?: string)` | `Credential` | When the user returns from the sign-in page with a code. See [PKCE](#pkce). |
| `completeRefresh(credential: Credential)` | `Credential` | Whenever a stored credential is about to expire, before it is used. See [Credentials](#credentials). |
| `getAccountDescription(credential: Credential)` | `AccountDescription` | When an account is added, to name it. |
| `accountAdded(account: AccountDescription)` | nothing | Optional. After an account is added, or signed in to again, and stored, so `accounts.all()` lists it. See [Endpoints on an account](#endpoints-on-an-account). |
| `accountRemoved(account: AccountDescription)` | nothing | Optional. After the user removes an account. See [Removing an account](#removing-an-account). |

```ts
interface ServiceDescription {
  title: string;        // "Notion"
  unit: string;         // what the service calls an account: "Workspace"
  kinds?: ResourceKindDescription[];  // a connector's kinds of item; see Resource kinds
  metadataKeys?: MetadataKey[];       // the metadata its items carry; see Metadata
  signInHint?: string;  // what to do on the sign-in page, when it shows no code
}
interface PkceChallenge {
  challenge: string;    // base64url SHA-256 of the verifier
  method: string;       // always "S256"
}
interface Credential {
  authorization: string;
  refresh?: string;
  expiresAt?: number;   // Unix seconds
}
interface AccountDescription {
  identifier: string;   // stable at the service; the same one again replaces the credential
  description: string;  // for people: a name or email
}
```

The description, with the extension's identifier, manifest `version` and
[icon](#icon), is the extension's **connector definition**, which every
account of it shares. Installing the extension stores it, so a manifest
listing `oauth` needs a `serviceDescription()` that runs without globals;
installing again with a higher manifest `version` rewrites it for every
account, and a lower or equal one leaves it. A client offering a connector under
the same identifier serves the same connector.

### PKCE

Every sign-in comes with a PKCE (RFC 7636) pair the daemon makes, so a
service that requires PKCE needs no state of the extension's own.
`authenticationURL(pkce)` puts the challenge in the sign-in page's URL, and
`completeAuthentication(code, verifier)` sends the verifier with the code:

```js
function authenticationURL(pkce) {
  return "https://example.com/oauth/authorize?client_id=…&response_type=code"
    + `&code_challenge=${pkce.challenge}&code_challenge_method=${pkce.method}`;
}
function completeAuthentication(code, verifier) {
  const response = network.fetch({
    url: "https://example.com/oauth/token", method: NetworkMethod.post,
    jsonBody: { grant_type: "authorization_code", client_id: "…", code,
                code_verifier: verifier },
  });
  // …return a Credential from response.body
}
```

The daemon keeps the verifier in memory from `authenticationURL` until the
next `completeAuthentication` for the extension, which takes it: starting a
second sign-in at the extension replaces the first's, and a daemon restart in
between loses it, leaving `verifier` `undefined`. Clients see none of this;
they send only the code. A service without PKCE can ignore both.

### Credentials

`completeAuthentication` and `completeRefresh` return a `Credential`. The
daemon stores it with the account, passes it to every
[import function](#the-import-functions) that reaches the service, and returns it
from `account.credential()`.

Before each of those uses, if `expiresAt` is within 60 seconds of now, the
daemon calls `completeRefresh(credential)` first and stores the result in
place of the old credential. One account is refreshed once at a time: a use
that finds a refresh under way waits for it and gets the stored result, so
`completeRefresh` never sees a refresh token twice. A credential without
`expiresAt` never expires.
If `completeRefresh` fails, the use that needed it fails with that error, and
the daemon flags the account (`credential_failed_at`, the time of the first
failure). Connectors → Accounts then marks it "Sign-in failed" and offers
**Sign in again** on its row (also in its menu). Only a failed
`completeRefresh` flags an account: an import function that fails for another
reason doesn't, and nor does an account whose credential never expires. The
flag clears when a refresh later succeeds (an expired credential is retried
at each use) or the user signs in again.

Signing in again runs the same `authenticationURL` and
`completeAuthentication` as adding an account, and because the account's
`identifier` is the same, its credential is replaced and what it imported
stays. Signing in as a different remote account adds that account instead,
and the page says so; the flagged one stays flagged.

### Removing an account

When the user removes an account (Connectors → Accounts, or
`deleteConnectorAccount`), the daemon deletes it and then calls
`accountRemoved(account)` with the account's `identifier` and
`description`, so the extension can undo what it set up for the account,
such as an endpoint it created with [`intelligence`](#intelligence-needs-intelligence).
The account is already gone, so `accounts.all()` no longer lists it, and
what `accountRemoved` returns or throws doesn't stop the removal: a throw
is logged. An extension that doesn't define it, or no longer holds `oauth`,
isn't called. Uninstalling the extension doesn't call it.

```js
function accountRemoved(account) {
  const name = storage.get(`endpoint.${account.identifier}`);
  if (name) intelligence.endpoint(name)?.delete();
  storage.remove(`endpoint.${account.identifier}`);
}
```

### Endpoints on an account

An extension holding `oauth` and `intelligence` can serve models whose key
is an account's credential, such as a ChatGPT plan's hour-long access
token: `account.createEndpoint(...)` creates an intelligence endpoint that
signs in with the account. Every request it sends carries the account's
credential, refreshed first by the extension's `completeRefresh` when it is
about to expire, as `account.credential()` is, so a conversation that
outlasts a credential gets its renewal.

```js
function accountAdded(added) {
  const account = accounts.all().find((a) => a.identifier === added.identifier);
  // Signing in again keeps the account, and with it its endpoint.
  if (intelligence.endpoints().some((e) => e.accountId === account.id)) return;
  const endpoint = account.createEndpoint({
    identifier: "openai", baseURL: "https://api.openai.com/v1",
    dialect: Dialect.statelessResponses, name: "ChatGPT",
  });
  endpoint.addModel({ identifier: "gpt-6.1-sol", prices: { input: 0, output: 0 } });
}
```

- The endpoint takes `{ identifier, baseURL, dialect, name?, usageURL? }`,
  as a custom one does, and starts with no models.
- Removing the account deletes it, and so does uninstalling the extension.
  Users remove the account from the endpoint's row in Settings ›
  Intelligence, too.
- `Dialect.statelessResponses` is the Responses API with nothing stored on
  the server: each turn sends the whole conversation, with the system
  prompt as `instructions`, and streams its reply. ChatGPT plans serve only
  that.
- A sign-in that doesn't end by showing a code, such as one redirecting to
  a loopback address no server answers, says what to paste instead with
  `serviceDescription()`'s `signInHint`; the extension reads whatever the
  user pasted as `completeAuthentication`'s `code`.

### The `accounts` global

```js
for (const account of accounts.all()) {
  const { authorization } = account.credential();
  const response = network.fetch({
    url: "https://api.example.com/me",
    method: NetworkMethod.get,
    headers: { Authorization: "Bearer " + authorization },
  });
  log.info(`${account.description}: ${response.code}`);
}
```

- `accounts.all()` returns an `Account` for each account the user
  added for **this** extension, oldest first. Other extensions' accounts
  never appear.
- An `Account` has the account's `id`, `connectorId`, `identifier`,
  `description` and `createdAt`.
- `account.credential()` returns the stored `Credential`, refreshed first
  when it is about to expire, as above. The refresh calls the extension's
  own `completeRefresh` in the same sandbox, and stores the result even
  without `cohesion:write`. A missing `completeRefresh` throws an `Error`, as
  does calling `credential()` from inside `completeRefresh`; one that throws
  fails the call with its exception.
- Reaching the service needs `network` too.

## Connectors

A connector is an extension that imports items from the accounts it signed
in to as resources. It holds `connector` and `oauth` (a manifest listing
`connector` without `oauth` fails to install), and defines three more global
functions, which the daemon calls to list what an account has and to fetch
items.

**Resources** are items imported from an account into one workspace, under
Connectors → Import. A resource is a read-only copy. Scripts read it with
`workspace.resources({ kind?, search? })` or as a `SearchResult`; they can't
change it, and the daemon keeps it fresh (see [Refresh](#refresh)). Resources
are deleted for good, not soft-deleted, and can be imported again. Removing
an account deletes its resources. Uninstalling the extension orphans its
accounts: they lose their credential and keep their resources, which the
user can move to another account of the same connector, such as one
added after reinstalling, under Connectors → Accounts.

### The import functions

The import calls need both permissions enabled: without `oauth` the daemon
can't refresh the account's credential, and the call fails saying so.

| Function | Returns | Called |
|---|---|---|
| `getOrganizationUnits(credential: Credential)` | `OrganizationUnit[]` | When the user browses an account. |
| `listResources(filters: ConnectorResourceFilters, credential: Credential)` | `ConnectorResourcePreview[]` | When the user searches an account for items to import, and first on every refresh, with no filters. |
| `getResource(identifier: string, credential: Credential)` | `ConnectorResource` | To import an item, and on every refresh of it. |

```ts
interface OrganizationUnit { id: string; display: string }
interface ConnectorResourceFilters { organizationUnitId?: string; search?: string }
interface ConnectorResourcePreview {
  identifier: string;
  lastUpdated: number | null;  // Unix seconds; null when unknown
  title: string;
  kindIdentifier: string;
  metadata?: { [key in MetadataKey]?: string };
}
interface ConnectorResource {
  identifier: string;
  lastUpdated: number;  // Unix seconds
  title: string;
  contents: string;
  contentKind: "markdown" | "text" | "json";
  kindIdentifier: string;
  metadata?: { [key in MetadataKey]?: string };
}
```

- `lastUpdated` is in Unix **seconds**, like `Document.modified`, not
  milliseconds. A preview's is `null` when the service has no modified
  date or the listing can't know it cheaply (a refresh then fetches the
  item); clients show no "updated" label for it. A fetched item's is a
  number, and `0` there means the same: clients show no label for it
  rather than a date in 1970. (A daemon reads a preview's `0` as `null`,
  for extensions written before `null` existed.)
- `kindIdentifier` is a short key for the item's kind, one of the
  definition's `kinds`, such as `"page"`, on a preview and on the item
  alike. The resource's `kind` is that key prefixed with the connector's
  identifier, as `"com.example.notion.page"`, and it is what
  `workspace.resources({ kind })` matches. An item of a kind the definition
  doesn't declare can't be imported, and a list holding one fails.
- `contentKind` says whether `contents` is Markdown, plain text or JSON text
  to `JSON.parse`. Search finds a Markdown or text resource by its title and
  contents, and a JSON resource by its title only.

### Metadata

An item may carry metadata from a fixed vocabulary, `MetadataKey`:
`dateCreated`, `dateModified`, `dateDue`, `dateStart`, `dateEnd`,
`author`, `url`, `location`, `folder`, `category`, `keywords` (separated by
commas) and `status`. Values are text; dates are ISO 8601 in UTC with a
`Z` suffix (`"2026-10-24T09:30:00Z"`), so they sort as text. List the keys
a connector fills in `metadataKeys`. A fetch replaces what the resource
carried; scripts read it with `resource.get(MetadataKey.author)`, which
answers `null` for a key the item doesn't have.

### Resource kinds

`serviceDescription()` lists the kinds of item `getResource` returns in
`kinds`, so scripts, the web UI and the assistant know what a resource of
each kind is and holds:

```ts
interface ResourceKindDescription {
  identifier: string;  // the kindIdentifier getResource returns: "page"
  title: string;       // for people: "Page"
  plural: string;      // for several, titling their list: "Pages"
  contents: string;    // what contents holds, in a sentence or two
}
```

```js
kinds: [
  { identifier: "page", title: "Page", plural: "Pages",
    contents: "Markdown of the page's blocks." },
  { identifier: "database", title: "Database", plural: "Databases",
    contents: "JSON { rows: [{ item: string, amount: number }] }." },
],
```

They are part of the connector definition, so changing them takes a
higher manifest `version`; a version that leaves a kind out removes it,
and every resource of it. Each kind is a page of its own in the web UI,
listed under its `plural` in a workspace that shows it. Every assistant and agent session lists the connectors
the user has accounts at in its code tool's description, under "Connected
services", with each declared kind as its full `kind` and `contents`, then
the accounts. Agents
can also browse an account's live service and import from it through the
`connectors` global in `cohesion.d.ts` (a module agents load on demand), as a last resort after the imported
resources. Keep `contents` short and concrete: it is prompt text.

### Refresh

The daemon refreshes every imported resource one minute after it starts, then
every 30 minutes. For each account it calls `listResources` with no
filters, then `getResource` for each resource whose `lastUpdated` moved or
that the list left out, and writes only the resources whose `lastUpdated`,
contents or metadata changed; each one it writes raises
`resource:update`. Importing a resource raises `resource:import`. A failure for one
resource or account is logged and doesn't stop the pass.

`refreshResources { workspace_id }` runs the same pass now, for one workspace,
and answers `{ checked, updated, failed }`. The web UI offers it as Refresh
now.

### Errors and limits

These apply to the sign-in and import functions alike.

- A function that throws shows its message to the user.
- A function that returns the wrong shape fails with "*Extension*:
  *function* didn't return the expected shape" and the reason.
- A missing function fails with "script does not define a function named …".
- Each call has the usual 60-second limit. The calls run on the extension's
  own worker, so they queue behind its hooks and timers, and one slow call
  delays the rest.

### API

The web UI drives accounts and connectors through these commands (see
`cohesion api`). Needing `oauth`: `listConnectorServices` describes every
service an extension granted `oauth` reaches; `beginConnectorAuthentication`
takes `{ extension_id }` and answers the sign-in `{ url }`;
`completeConnectorAuthentication` takes `{ extension_id, code }` and adds
the account (or replaces the credential of one already added).
`listConnectorAccounts` lists accounts, without credentials, each with its
`connector` definition, `credential_failed_at` (when refreshing its
credential last failed, if it needs signing in again) and `importable`, whether its items can be browsed
(its extension is granted `connector`);
`deleteConnectorAccount` takes `{ account_id }`; `migrateResources` takes
`{ from_account_id, to_account_id }`, two accounts of one connector. Needing
`connector` (and `oauth`): `listConnectorOrganizationUnits` takes
`{ account_id }`; `searchConnectorResources` takes
`{ account_id, workspace_id, organization_unit_id?, search? }` and answers the
items the account offers, each marked `imported` or not; `importResource` takes
`{ account_id, workspace_id, identifier }`; `refreshResources` takes
`{ workspace_id }`. `listResources` takes
`{ workspace_id, kind?, search?, sort, descending }`, `sort` being `"title"`,
`"lastUpdated"` or a metadata key;
`getResource` and `deleteResource` take `{ workspace_id, resource_id }`.

### A minimal connector

This one serves two canned resources. A real connector also holds `network`, to
call the service's API with the credential.

```js
const ICON = "data:image/svg+xml," + encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="20" fill="#5b6cff"/><text x="50" y="64" font-size="48" text-anchor="middle" fill="#fff" font-family="sans-serif">F</text></svg>');
function manifest() {
  return {
    identifier: "com.example.fake-connector",
    name: "Fake connector",
    version: 1,
    description: "Serves two canned resources.",
    author: "Example",
    icon: ICON,
    permissions: ["oauth", "connector"],
    hooks: ["resource:import", "resource:update"],
  };
}
const ITEMS = {
  p1: { title: "Roadmap", kind: "page", unit: "u1", text: "# Roadmap\n\nShip connectors." },
  d1: { title: "Budget", kind: "database", unit: "u2", text: "{\"rows\":3}" },
};
function serviceDescription() {
  return {
    title: "Fake service",
    unit: "Workspace",
    kinds: [
      { identifier: "page", title: "Page", plural: "Pages", contents: "Markdown of the page." },
      { identifier: "database", title: "Database", plural: "Databases",
        contents: "JSON { rows: number }." },
    ],
    metadataKeys: ["author"],
  };
}
function authenticationURL() { return "https://example.com/authorize"; }
function completeAuthentication(code) {
  return { authorization: "token-" + code, refresh: "r1", expiresAt: code === "expired" ? 1 : 4102444800 };
}
function completeRefresh(c) { return { authorization: c.authorization + "+", refresh: c.refresh, expiresAt: 4102444800 }; }
function getAccountDescription(c) { return { identifier: "acct-1", description: "Example Co. (" + c.authorization + ")" }; }
function getOrganizationUnits() { return [{ id: "u1", display: "Engineering" }, { id: "u2", display: "Finance" }]; }
function listResources(filters) {
  return Object.entries(ITEMS)
    .filter(([, i]) => (!filters.organizationUnitId || i.unit === filters.organizationUnitId)
                    && (!filters.search || i.title.toLowerCase().includes(filters.search.toLowerCase())))
    .map(([id, i]) => ({ identifier: id, lastUpdated: 1700000000, title: i.title,
                         kindIdentifier: i.kind }));
}
function getResource(id) {
  const i = ITEMS[id];
  return { identifier: id, lastUpdated: 1700000000, title: i.title, contents: i.text,
           contentKind: i.kind === "page" ? "markdown" : "json", kindIdentifier: i.kind,
           metadata: { author: "Ada" } };
}
function onResourceImport(resource) { log.info("imported " + resource.title + " as " + resource.kind); }
function onResourceUpdate(resource) { log.info("updated " + resource.title); }
```

## Updating and removing

Running `cohesion extension install` again with the same `identifier`
replaces the script, name, version, description, author and icon. Permissions and
hooks the user already granted stay as they are; ones the new manifest adds
start off, and ones it no longer lists are removed. The running sandbox is
discarded, and `onLaunch` runs again if the `launch` hook is on.

Uninstalling (Extensions → Installed, or
`cohesion api '{"type":"uninstallExtension","args":{"extension_id":1}}'`)
stops the sandbox and its timers and deletes the extension with its
permission and hook grants. Data it already wrote stays.
