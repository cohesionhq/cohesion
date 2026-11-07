# Actors

An actor is an agent on a client's device that Cohesion hands actor sessions to,
such as Claude Code in one directory. The client does the work in the
actor's own system; Cohesion keeps each actor session's transcript and status, shows
a working actor session as an activity, and carries what the user says to the actor.
An agent or an extension can also start an actor session, reply in one or stop one
(the `actors` script global); to the client those are the same calls.
An agent that hands an actor session work follows it: each time the actor session stops on
its own (it reports `needsAttention`, `idle` or `failed`, or its client
goes away), the agent hears of it. Nothing about that reaches the client:
the agent follows the statuses the client reports.
This guide is for writing a client that offers actors. For how the daemon
handles them, see "Actors" in [architecture.md](architecture.md).

`cohesiond/examples/fake_actor.rs` is a complete actor client in one file,
which steps an actor session through every state:

```sh
cargo run -p cohesiond --example fake_actor -- /tmp/cohesion.sock "Fake laptop"
```

## The session

An actor client is an ordinary client with a session to the daemon:

- **On the same machine**, over the Unix socket. The first line the client
  sends is a `LocalIdentity`, `{"key": "...", "name": "Ada's MacBook
  Pro"}`, and the daemon answers one line, `{"outcome": "accepted"}` or
  `{"outcome": "refused", "reason": "..."}`. Keep the key with the client's
  configuration: the same key is the same client, under the name it was
  first listed as.
- **From another device**, over the WebSocket at `/connect`, with the
  certificate the client paired with.

Either way, every frame is one line of JSON, and every frame the daemon
sends carries a `seq` and a `type`. The first is `hello`, which names the
client as Cohesion lists it (`client_name`).

## Offering actors

The client sends a `hello` frame listing its actors, and sends it again
whenever they change; each `hello` replaces what the client offered
before. An actor no longer offered disappears from Cohesion, but its actor sessions
stay.

```json
{
  "type": "hello",
  "id": 1,
  "actors": [
    {
      "identifier": "cohesion",
      "product": "Claude Code",
      "scope": "/Users/ada/Developer/cohesion",
      "modes": [
        { "identifier": "default", "title": "Ask", "description": "Asks before every edit." },
        { "identifier": "acceptEdits", "title": "Auto", "description": null }
      ],
      "models": [
        {
          "identifier": "opus",
          "title": "Opus",
          "efforts": [
            { "identifier": "medium", "title": "Medium" },
            { "identifier": "high", "title": "High" }
          ]
        }
      ]
    }
  ]
}
```

- `identifier` is what the daemon names the actor by in calls, unique
  among the client's actors.
- `product` and the client's name make the actor's title: "Claude Code on
  Ada's MacBook Pro". `scope` says where it works, such as a directory.
- `modes` and `models` list how it can run an actor session and on what, the first of
  each the default. Leave a list empty when the actor has one way only, or
  picks its own model. A model's `efforts` are listed least first, and are
  empty when it takes no setting.

Every identifier must be non-empty and unique among its siblings. The daemon
answers with a `reply` carrying the `hello`'s `id`: `accepted`, or why the
offer was refused.

## Calls

The daemon asks an actor to do something in a `call` frame. The client
answers each with a `callResult` carrying the `call_id`, within 60 seconds:

```json
{ "type": "call", "seq": 4, "call_id": 7, "call": { "kind": "startActorSession", ... } }
{ "type": "callResult", "call_id": 7, "ok": { "kind": "accepted" } }
{ "type": "callResult", "call_id": 7, "error": "Claude Code isn't installed." }
```

Answer once the actor has taken the call, not once it has finished the
work: the work is reported in updates, as it goes. An error is shown to the
user as written, and leaves things as they were: no actor session, no message,
the mode unchanged.

There are four calls, each naming the actor (`actor`) and the actor session
(`session_id`, Cohesion's id for it):

- **`startActorSession`**: start a new actor session with the user's `prompt` and the
  `images` they attached to it, in `mode`, on `model` at `effort`; each of
  the last three is `null` for the actor's default. The actor session is working
  from the moment it starts. `title` is the title it was started with,
  such as one an agent gave it, or `null` when Cohesion titled it by the
  prompt's first line; a client that generates titles, as Claude Code
  does, can send its own in a `title` update (see below), and may leave a
  title it was given as it is.
- **`sendActorSessionMessage`**: what the user said in the actor session, message
  `message_id`, as `content`, with the `images` they attached. It can be
  more to do, which starts an actor session that is done, failed or resolved again, or a
  response to one of the actor's requests (see below). A client that
  doesn't know the actor session, such as after a restart, should pick it up from
  here.

`images` is a list, empty when there are none, of up to 10 images, each
`{"media_type": "image/png", "data": "<base64>"}`: PNG, JPEG, GIF or WebP,
at most 5 MB once encoded, the same images an agent's models take. Hand
them to the actor with the text, such as Claude Code's image content
blocks, or save them where it can read them and name their paths.
- **`stopActorSession`**: stop what the actor session is doing. It is idle afterwards,
  and the user can still send it a message to start it again. Resolving
  an actor session that is going on sends this too.
- **`setActorSessionMode`**: run the actor session in `mode`, one of the actor's
  `modes`, from its next step on. It can come at any time: while the actor session
  works, while it waits on a request (switching to a mode that allows what
  the request asks, say), or while it is idle. It changes nothing else, so
  the status stays as it was.

## Updates

The client reports on an actor session in `actorSessionUpdate` frames, each with an `id`
of the client's own and the `session_id`. The daemon takes them in the order
sent and answers each with a `reply` carrying its `id`.

```json
{ "type": "actorSessionUpdate", "id": 12, "session_id": 3, "kind": "status", "status": "working" }
```

An update is one of six kinds:

- **`message`**: something the actor said, as `content`, added to the
  transcript, with any `images` it shows with it (see below). The reply
  carries the stored message (`actorSessionMessage`), whose `id` the user's
  response names when the message is a request, and an `end` names
  when it is a tool or thinking.
- **`status`**: where the actor session stands now (see Statuses).
- **`progress`**: what the actor is doing now, in a line, such as
  `"Running the tests"`, or `null` to clear it. It shows on the actor session's
  activity. Progress for an actor session that is done, failed or resolved sets it working.
- **`end`**: the tool or thinking in the `toolUse` or `thinking` message
  `message_id` has finished, a tool failed or not (see
  [Durations](#durations)).
- **`context`**: how full the actor session's context window is now (see
  [Usage](#usage)).
- **`title`**: what to call the actor session from now on, as `title`, such as one
  the actor generated from the prompt. Cohesion keeps its first line, cut
  short past 80 characters, and refuses a blank one; the actor session's activity,
  if it is working, takes the new title too. A client that never sends
  one keeps the title the actor session started with.

```json
{ "type": "actorSessionUpdate", "id": 13, "session_id": 3, "kind": "title", "title": "Fix the flaky socket test" }
```

An update about another client's actor session is refused.

A `message` can carry the images the actor shows the user, such as a
screenshot it took, a chart it drew or an image it read, in `images`: the
same list as a call's, up to 10 images of PNG, JPEG, GIF or WebP, each at
most 5 MB once encoded. Left out, it is none, so a client that never shows
images needn't send it. Cohesion shows them with the message, below what
it says. A message with an image it refuses is refused whole, nothing
stored, so send it again without the image:

```json
{
  "type": "actorSessionUpdate", "id": 13, "session_id": 3, "kind": "message",
  "content": { "kind": "toolUse", "tool": "Screenshot", "category": "other", "summary": "the login page" },
  "images": [{ "media_type": "image/png", "data": "iVBORw0KGgo..." }]
}
```

## Usage

Each actor session has a context window, and an actor may run on a plan with
limits (such as a five-hour and a weekly one). Cohesion shows an actor session's
context as a ring under its composer, which opens to the numbers and its
actor's plan limits; the Actors page lists the plan limits under "Usage",
one row an account the actors share and one for each actor whose limits
are its own, and scripts read them with `actor.usageLimits()`.

Clients read the two from different places: the context from the actor session
itself (`ActorSession.context`, with `used_tokens` and `total_tokens`, or
null), and the plan from the `listActorUsage` entry whose `actor_ids`
holds the actor session's `actor_id` (`plan`, and `pool` when the account is
shared). The web UI's `planOf` does that lookup.

An actor session's context is a `context` update, sent whenever it changes, such as
after each step; each replaces what the actor session reported before, and
Cohesion keeps it with the actor session, so an actor session at rest still shows its last.

```json
{
  "type": "actorSessionUpdate", "id": 14, "session_id": 3, "kind": "context",
  "context": { "used_tokens": 41800, "total_tokens": 200000 }
}
```

`used_tokens` is what the context window holds now, never negative, and
`total_tokens` the most it can hold, more than zero. An actor session its actor
never reports a context for shows none.

The plan's limits are the actor's, shared by all its actor sessions: send an
`actorUsage` frame whenever they change, such as after each turn; each
replaces what the actor reported before.

```json
{
  "type": "actorUsage",
  "id": 15,
  "actor": "cohesion",
  "plan": [
    { "label": "5-hour limit", "used": 0.72, "resets_at": 1793290000 },
    { "label": "Weekly limit", "used": 0.18, "resets_at": 1793630000 }
  ],
  "pool": { "id": "claude:jackson@example.com", "name": "jackson@example.com" }
}
```

- `actor` is one of the actors the client offers, by its identifier.
- `plan` lists the plan's limits in the order to show them, at most 8:
  each a `label` of at most 64 bytes, how much of it is `used` from 0 to 1,
  and when it resets (`resets_at`, Unix seconds), which may be left out.
  Leave the list out, or empty, when the actor runs on no plan, such as on
  an API key.
- `pool`, which may be left out, names the account the limits belong to
  when other actors draw on it too, such as several directories signed in
  to one account: an `id` of at most 256 bytes that names the account for
  good, prefixed with your product (`claude:` and the account's email,
  say), and a `name` of at most 128 bytes to show for it. Actors naming
  the same `id`, from any client, share one row and one set of limits,
  which the latest report among them sets. Left out, the actor's limits
  are its own.

The daemon answers with a `reply` carrying the `id`: `accepted`, or why
it was refused. A frame with an empty `plan` clears the actor's usage.
Cohesion keeps it only in memory, while the client's session lasts: it is
gone once the session closes or the client stops offering the actor, so
report it again after reconnecting. A client that sends neither works as
before, and shows no ring. A daemon older than these refuses them as bad
requests; ignore that reply.

## Statuses

An actor session is in one of four statuses, as its actor last reported, or
resolved, which only the user sets:

| Status           | Cohesion shows | Means                                                               |
| ---------------- | -------------- | ------------------------------------------------------------------- |
| `working`        | Working        | The actor is at it.                                                 |
| `needsAttention` | Needs you      | The actor is waiting on the user, such as for a response.          |
| `idle`           | Done           | The actor has finished, until the user says more.                   |
| `failed`         | Failed         | The actor couldn't go on; the user may say more to try again.       |
| `resolved`       | Resolved       | The user is done with it; never reported by a client.               |

No actor session ends for good. A client reports only the first four; `resolved`
is the user's, or an agent's for them (`ResolveActorSession`, or
`actor session.resolve()` in a script), and a resolved actor session that reports `idle` or
`failed` stays resolved, so a client needn't know of it. Report `idle` whenever the actor finishes its turn,
whether or not the actor session as a whole is finished; report `failed` only when
the actor itself failed, not when, say, tests it ran did.

Cohesion sets some statuses itself, so the client needn't report them:

- A new actor session, and one the user says something to, is `working`.
- Stopping an actor session makes it `idle`, unless it is resolved.
- Resolving an actor session that is working or needs attention sends
  `stopActorSession` first.
- A working actor session, or one that needs attention, becomes `idle` when the
  client's last session closes or the daemon stops: neither can hear from
  the actor any longer. When the client reconnects, it reports the status
  of each actor session still going on (see [Staying in sync](#staying-in-sync)).

While an actor session is working or needs attention, Cohesion shows it as an
activity: running while it works, paused while it needs the user. Going
idle ends the activity as succeeded, failing ends it as failed, and
stopping it ends it as cancelled. The next stretch of work opens a new
activity.

## The transcript

What the actor says (`message` updates) is tagged by `kind`:

- **`text`**: its own words, as Markdown: `{"kind": "text", "text": "..."}`.
- **`thinking`**: its reasoning on the way, as Markdown: `{"kind":
  "thinking", "text": "..."}`, sent as it starts thinking. Send a summary
  or the full chain, whichever the actor gives; clients show it quieter
  than its words, folded away, with how long it took. Cohesion adds
  `ended_at` once an `end` names it; leave it out. It is left out of
  transcript search and of what a following agent reads back. An actor
  that doesn't share its thinking never sends one.
- **`toolUse`**: a tool it ran, such as `{"kind": "toolUse", "tool":
  "Edit", "category": "edit", "summary": "src/main.rs"}`, sent as the tool
  starts: the tool's name as the actor gives it, what kind of tool it is
  (see [Tool categories](#tool-categories)), and what it did, in a line.
  Cohesion adds `ended_at` once an `end` names it; leave it out.
- **`question`**: questions for the user, each with options to pick from
  (Claude Code's `AskUserQuestion`):

  ```json
  {
    "kind": "question",
    "questions": [
      {
        "header": "Approach",
        "question": "How should I fix it?",
        "options": [
          { "label": "Patch it", "description": "The smallest change that works." },
          { "label": "Refactor", "description": null }
        ],
        "multi_select": false,
        "allows_other": true
      }
    ]
  }
  ```

  A question with no options is answered in words.
- **`permission`**: leave to do something, and the decisions the user may
  take, in the order to offer them (`allow`, `allowForSession`, `deny`,
  `abort`):

  ```json
  {
    "kind": "permission",
    "subject": { "kind": "command", "command": "cargo test", "cwd": "/src/cohesion" },
    "reason": "To check the fix.",
    "decisions": ["allow", "allowForSession", "deny", "abort"]
  }
  ```

  The subject is a `command` (with the directory it runs in), a
  `fileChange` (`changes`, each a `path`, a `change` of `add`, `update` or
  `delete`, and a unified `diff` when the actor has one), or any other
  `tool` (its `name` and `input` as JSON).
- **`plan`**: a plan, as Markdown, for the user to approve before the actor
  starts (Claude Code's `ExitPlanMode`): `{"kind": "plan", "plan": "..."}`.

The last three are requests. After making one, report `needsAttention`,
and `working` again once the response comes. The user's response arrives in
a `sendActorSessionMessage` whose `content` names the request's message by
`request_id`:

- `{"kind": "answers", "request_id": 41, "answers": [{"selected": ["Patch
  it"], "other": null}]}`: one answer per question, in order, picking only
  the question's options, and in the user's own words (`other`) only where
  it allows them.
- `{"kind": "permission", "request_id": 42, "decision": "deny", "message":
  "Not on this branch."}`: one of the decisions offered, with a note.
- `{"kind": "plan", "request_id": 43, "approved": false, "feedback":
  "Keep the old API."}`. An approval (`"approved": true`) has the actor
  implement the plan in the same actor session, reporting `working` again; nothing
  starts a second actor session for it, and the agents' `actors` docs tell them so.

Cohesion checks a response fits its request, and takes one response per
request, before handing it to the actor. Anything else the user says is
`{"kind": "text", "text": "..."}`. A message that arrives while the actor
waits on a request is the user's reply in place of a response: the actor
takes it as the user passing over every request still open (for Claude
Code, declining to pick an answer and saying this instead) and carries on
with it, and Cohesion shows those requests as `answered`. An actor must
never hold the message until the request is answered: the user has
already moved on.

## Tool categories

A `toolUse`'s `category` says what kind of tool it is, so Cohesion can show
it (the web UI picks an icon for each) without knowing any actor's tool
names. Only the client knows its actor's tools, so the client classifies
each one by its name; `tool` stays the name the user sees.

| Category  | Means                                        | Claude Code's tools                            |
| --------- | -------------------------------------------- | ---------------------------------------------- |
| `command` | Runs a shell command, or watches one it ran. | `Bash`, `BashOutput`, `KillShell`              |
| `read`    | Reads a file or a directory.                 | `Read`, `NotebookRead`, `LS`                   |
| `edit`    | Changes files.                               | `Edit`, `MultiEdit`, `Write`, `NotebookEdit`   |
| `search`  | Finds files, or text in them.                | `Grep`, `Glob`                                 |
| `web`     | Fetches a page or searches the web.          | `WebFetch`, `WebSearch`                        |
| `agent`   | Hands work to another agent.                 | `Agent` (`Actor session` before it)                     |
| `skill`   | Takes up a skill, instructions for the actor session. | `Skill`                                        |
| `other`   | Anything else.                               | `TodoWrite`, MCP tools, the rest               |

Pick the category for what the tool does, not what it is called, and send
`other` for a tool that fits none. Left out, the category is `other`, so a
client that doesn't classify its tools still works, and a transcript stored
before categories existed reads as `other` throughout. A category Cohesion
doesn't know is refused, like any malformed update.

## Durations

Cohesion times each tool and each stretch of thinking from its `toolUse`
or `thinking` message to the `end` naming it, both stamped with the
daemon's clock in Unix seconds: the message's `sent_at`, and `ended_at`
on its `content`, `null` until then. A client shows one with no
`ended_at` as running while the actor session works.

Send the `toolUse` as the tool starts, or the `thinking` as the actor
starts thinking, wait for its reply, and keep the stored message's `id`
(for a tool, beside the actor's own id for the call, Claude Code's
`tool_use_id`). When it finishes, a tool failed or not, send an `end`
naming that `id`:

```json
{ "type": "actorSessionUpdate", "id": 14, "session_id": 3, "kind": "end", "message_id": 44 }
```

The reply is `accepted`. An `end` adds nothing to the transcript, and
tools that run side by side end in any order. Ending one that has
already ended changes nothing and is accepted, so resending one is
harmless; naming a message that isn't a `toolUse` or `thinking` of the
actor session is refused. One the actor session was stopped in, or one whose message never
got a reply, needn't be ended. A client that never sends `end` still
works: Cohesion counts its tools and thinking up while the actor session works,
and takes each to have run until the next message once it stops. A
client with the thinking's text only once it is done (a summary, say)
may send the `thinking` then and end it at once.

## An actor session, start to finish

```
daemon  call startActorSession (actor session 3, "Fix the flaky test")   actor session is working
client  callResult accepted
client  update progress "Reading the test"
client  update message text "It races on the socket."
client  update message question …                             reply: message 41
client  update status needsAttention
daemon  call sendActorSessionMessage (answers to 41)
client  callResult accepted
client  update status working
client  update message toolUse Edit (edit) "src/session.rs"  reply: message 44
client  update end 44                                         reply: accepted
client  update message text "Fixed."
client  update status idle                                    activity succeeds
```

## Staying in sync

Cohesion and the actor each keep half of an actor session: Cohesion its transcript
and status, the actor its own session. Either side can restart while the
other runs, so a client should expect to lose its session at any time and
pick up where it left off.

### What Cohesion keeps, and what it forgets

- **It keeps** every actor session, its transcript, and every update it replied to.
  An update the daemon answered is stored, and is never lost by a restart.
- **It forgets** that anything is going on. When the daemon stops, or the
  client's last session closes, every working actor session and every one that
  needs attention becomes idle, and its activity ends cancelled. Cohesion
  learns otherwise only from the client.
- **It doesn't queue** anything for a client that is away. A call to an
  offline client fails at once, and the user sees why, so nothing waits to
  be replayed when the client comes back. A message the user sent is
  stored only once the client accepted it.
- **A stop still lands.** Stopping an actor session while its client is away makes
  it idle without telling the actor. If the actor kept working, its next
  `status` report makes the actor session working again.

### Reconnecting

1. **Reconnect when the session drops**, with a backoff: a daemon restart
   closes the socket or WebSocket, and the daemon is back within seconds.
   The one exception is an `evicted` frame: `replaced` means another
   session serves the client now, and `revoked` means the client was
   deleted and has to be paired again. Reconnecting after either would
   only take the other session's place, or be refused.
2. **Send `hello` first, and wait for its reply.** Updates belong to the
   client a session serves, so one sent before the `hello` is accepted is
   refused ("This session serves no client.").
3. **Report each actor session that is still going on**: a `status` of `working`,
   or `needsAttention` if the actor is still waiting on a request, then its
   `progress`. An actor session that finished while the client was away takes a
   `status` of `idle` or `failed`; one Cohesion already holds as idle needs
   nothing.
4. **Send what didn't get through.** Updates are answered in order, so the
   ones with a reply were stored; anything sent after the last reply may or
   may not have been. Resending a `status`, `progress`, `end` or `title` is harmless, but
   resending a `message` adds it twice. Before resending messages, read the
   transcript (below) and send only those it doesn't hold.

While the session is down, keep the actor working and queue its updates in
order; flush the queue after step 3. Updates for an actor session Cohesion no longer
has are refused as not found: drop them.

### What the client should keep

The client is what survives a restart of the actor, so keep enough on disk
to resume each actor session without Cohesion's help:

- **Cohesion's `session_id`, mapped to the actor's own session**, such as a
  Claude Code session id, so a later call for the actor session resumes that
  session (`claude --resume <id>`) rather than starting a new one. A call
  for an actor session the client doesn't know, such as `sendActorSessionMessage` after
  the client lost its state, should still be taken: start a session for
  it from the transcript.
- **The actor session's mode, model and effort**, which only `startActorSession` says,
  and the mode as each `setActorSessionMode` changes it.
- **Each open request's message id** (the `id` in the reply to its
  `message` update), mapped to whatever the actor is waiting on, so the
  user's response, which names that id as `request_id`, reaches the right
  question. If the actor can no longer take the response (its process
  died, say), hand it over as plain text, or report the actor session `idle`.
- **The last update Cohesion replied to**, per actor session, to know where to
  resume sending.

### Reading Cohesion's side

A paired client may send any request the protocol offers, so an actor
client can read what Cohesion holds instead of keeping a copy of it:

- `{"type": "getActorSession", "args": {"id": 3}}` reads one actor session, with its
  `status` and Cohesion's id for its actor (`actor_id`).
- `{"type": "listActorSessions", "args": {"actor_id": 4}}` lists that actor's
  actor sessions, latest status change first, which finds any the client lost
  track of.
- `{"type": "listActorSessionMessages", "args": {"session_id": 3}}` reads its
  transcript, oldest first, with whether each request was `answered`, and
  how many images came with each message, the user's or the actor's
  (`attachment_count`).
- `{"type": "listActorSessionMessageAttachments", "args": {"message_id": 9}}`
  reads one message's images, in the order sent, which the transcript
  leaves out.

The transcript is what an actor session resumed from scratch starts from: the user's
prompt is its first message, and an unanswered request is one the user can
still respond to.
