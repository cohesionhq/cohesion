# Command line

`cohesion` is the command-line client, for small jobs and scripts. This page
covers connecting it, the common commands, setting up AI endpoints, agents and
sending raw commands. `cohesion <command> --help` lists every option.

## Connect the CLI

On the computer running `cohesiond`, connect through the Unix socket. This needs
no setup code. The CLI makes a local key that identifies it, under `--name` or the
hostname followed by "(CLI)", such as `apollo (CLI)`.

```sh
cohesion setup --local                 # or give a path: --local /tmp/cohesion.sock
```

From another computer, use a setup code. `cohesiond` prints one on its first start,
and `cohesion pair` makes more.

```sh
cohesion setup <setup-code>
```

A client is known by its certificate or its local key. Its name (`--name`) is only
how it's listed, and Settings › Clients can change it.

Any client can also offer capabilities: the actors on its device, such as Claude
Code, that Cohesion hands actor sessions to. See "Actors" in
[Architecture](architecture.md#e-actors).

`-c <DIR>` keeps this client's identity and settings in another directory. Without
it, the CLI uses your per-user config directory.

## Common commands

```sh
cohesion status                      # which cohesiond and workspace this client uses
cohesion pair                        # a setup code for another client, good once for 10 minutes
cohesion switch                      # list workspaces, or `switch <id|title>` to select one
cohesion workspace                   # list workspaces, or `create`, `switch`, `rename`, `delete`
cohesion journal                     # open today's journal in $VISUAL or $EDITOR
cohesion journal get                 # print today's journal, empty if nothing was written
cohesion journal add called the dentist
echo "from a pipe" | cohesion journal    # piped text is appended
cohesion task                        # open tasks, or `--all`, `--folder <id|title>`
cohesion task add buy milk -f errands -p high
cohesion task priority 12 urgent     # none, low, medium, high, urgent
cohesion task show 12                # the task and its description
cohesion task describe 12            # edit the description in $EDITOR, or pass text, or pipe it
cohesion task add file taxes --due 2027-04-15 --start today
cohesion task schedule 12 --due tomorrow --start none
cohesion task folder                 # list folders, or `create`, `delete`
cohesion tag list                    # list tags, or `create`, `rename`, `delete`
cohesion article                     # what your feeds brought in, or `show <id|title>` to print a page's text
cohesion article feed add https://example.com/feed.xml  # every workspace shares feeds
cohesion search dentist              # documents, tasks, resources and contacts
cohesion run script.js               # run a script in cohesiond and print its result
cohesion script new script.js        # write a starter script and cohesion.d.ts
cohesion mcp                         # serve MCP on stdin and stdout, for an MCP client to start
cohesion extension install ext.js    # install an extension with its grants off, or `-o` to replace one
cohesion extension call <identifier> <name> '{"q":1}'   # call its `serviceRequest` and print the JSON
cohesion open                        # serve the web UI on 127.0.0.1:7343 and open it
cohesion export data.db              # copy all of cohesiond's data to a file
cohesion import data.db              # replace all of cohesiond's data with it, then restart it
```

`cohesion open` also takes `--port`, `--no-browser`, `--dev`, `--reset-token` and
`--exit-with-parent`.

A command named for a thing takes its default action without a verb. Most list, but
`journal` opens the editor. `ai`, `ai endpoint`, `ai models`, `agent` and `team` work
the same way.

`-w <id|title>` overrides the selected workspace for one command. Things you name
(workspaces, folders, agents, endpoints…) resolve by id or by name, ignoring case.
If several share a name, the command asks for the id. Deletes ask first unless given
`-y`. Friendly commands print text, and `cohesion api` prints JSON. The conventions,
and how to add a command, are in "Add a CLI command" in
[Contributing](../CONTRIBUTING.md).

## AI endpoints

An endpoint is an account with an AI provider, and agents run on its models. `ai` is
short for `intelligence`.

```sh
cohesion ai endpoint create anthropic --api-key "$ANTHROPIC_API_KEY"   # named Anthropic, with its default model
cohesion ai catalog anthropic haiku           # browse the models.dev catalog
cohesion ai models add Anthropic claude-haiku-5-5
cohesion ai models Anthropic                  # what the endpoint offers, with prices
cohesion ai use Anthropic claude-sonnet-5-5   # the `agent` tier: every agent conversation
cohesion ai use Anthropic claude-haiku-5-5 --for low   # cues marked low effort
cohesion ai                                   # endpoints, their models and the tiers at a glance
```

`endpoint create --help` lists the provider presets. `custom` takes `--identifier`,
`--dialect` and `--base-url` instead. An endpoint is named after its provider unless
`--name` says otherwise, so a second account on the same provider (`--name Work`)
gets its own name and its own models. Endpoints are named by name or id.

### Usage page and prompt caching

A preset's endpoint links to its provider's usage page unless `--usage-url` gives
another. Settings › Agents can change the link later.

An endpoint also says how many minutes its provider keeps a prompt cached: 30 for
OpenAI, and 5 for the others and for a custom endpoint. After that time, an agent's
idle conversation starts afresh. `--cache-minutes` gives another value, and
`setIntelligenceEndpointCacheMinutes` through `cohesion api` changes it.

Agents ask for a longer cache where the endpoint offers one: an hour for Anthropic,
12 hours for OpenAI and none for the others. `--long-cache-minutes` with
`--long-cache-write-multiple` (its write price as a multiple of the short cache's)
gives an endpoint one, such as a custom endpoint. `setIntelligenceEndpointLongCache`
changes or removes it.

### Models and tiers

Each endpoint offers the models added to it. `models add` imports a model from the
catalog, which supplies its context size and prices. For a model the catalog doesn't
list, it takes `--name`, `--context`, `--input` and `--output` (USD per million
tokens). `models remove <endpoint> <model>` takes a model away and unsets any tier
that used it.

There are 4 tiers. Every agent conversation uses `agent`, unless the agent has a
model of its own (`agent update --model`). Background runs use `low`,
`medium` or `high`, according to the effort the agent gave the cue that woke them.
Changing a default applies to existing agents from their next message or run.

## Agents

An agent is a persistent assistant with one ongoing conversation, on the `agent`
tier's model or one of its own. An agent can set cues. Each cue is a named hook that wakes a fork of
the agent's conversation, with an effort (`low`, `medium` or `high`) that picks the
model the fork uses.

You didn't start that fork, so the fork can read your data but can't change it, and
it has no network. The fork can message you, work with actors, note where things
stand and ask you before any change. Your answer then makes the change. Forks of one cue take
turns, so they never work over each other.

Assigning a task to an agent (`setTaskAssignee`, or another agent doing it) wakes a
fork of its conversation to work on the task. That fork has no cue and can do
everything the conversation may do. An agent assigning itself wakes nothing.

```sh
cohesion agent create Ada --color green
cohesion agent create                         # no name: the agent gets an unused one
cohesion agent                                # agents and their status
cohesion agent chat ada "watch my inbox folder for new tasks"
cohesion agent chat ada                       # chat: one message per line until Ctrl-D
cohesion agent messages ada -n 50             # the feed
cohesion agent pick 43 B                      # answer choice #43 with option B
cohesion agent cues ada                       # each cue's hook, effort, instructions and code
cohesion agent cues ada --remove "Triage"     # stop that cue waking the agent (name or id)
cohesion agent runs ada                       # forks, with tokens and cost
cohesion agent memories ada                   # what the agent remembers: its own, its team's and the global ones
cohesion agent forget m12                     # forget memory m12
cohesion team create Research                 # list teams with `team`, or `rename`, `delete`
```

Agents resolve by id or name, ignoring case. `agent update` takes `--name`, `--color`,
`--emoji`, `--team`, `--personality` (`balanced`, `friendly`, `professional`,
`candid`, `playful`) and `--verbosity` (`terse`, `brief`, `detailed`, `thorough`).
`--model <endpoint> <model>` gives the agent a model of its own in place of the
`agent` tier's, and `--default-model` puts it back on the tier's.
`agent delete` asks first unless given `-y`.

## MCP: `cohesion mcp`

`cohesion mcp` is a local MCP server for an MCP client, such as Claude Code, to start.
It speaks MCP over stdin and stdout and runs until the client closes stdin. Its one
tool, `code`, runs a script in cohesiond, as `cohesion run` does, and returns the
script's result. A script that throws comes back as an error result with the
message. The script API's TypeScript declarations, the `cohesion.d.ts` that
`cohesion script new` writes, are the resource `cohesion://cohesion.d.ts`.

```sh
claude mcp add cohesion -- cohesion mcp
```

The tool has the same access as this client, so the MCP client can read and change
everything in cohesiond. Pass `-c <DIR>` in the command the client starts to use
another client's configuration.

## Raw commands: `cohesion api`

`cohesion api [REQUEST]` sends one protocol `Command`, given as JSON, and prints the
reply as pretty JSON on stdout, errors included, so it pipes into `jq`. With no
`REQUEST`, or `-`, the CLI reads the JSON from stdin.

```sh
cohesion api '{"type":"createWorkspace","args":{"title":"Home"}}'
cohesion api '{"type":"listWorkspaces"}'
echo '{"type":"getJournal","args":{"workspace_id":1,"date":"2026-10-13"}}' | cohesion api
```

`type` is the command name in camelCase, and `args` holds the request's fields in
snake_case. An unknown `type` fails with a message listing the known ones.
