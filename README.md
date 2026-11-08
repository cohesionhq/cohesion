<img src="docs/logo.svg" alt="" width="96" height="96">

# Cohesion

Cohesion keeps your tasks, documents, journal and articles in one place, on a
computer you control. Agents can read and change them with you, and you can
open them from a browser, an app or the terminal.

## What you can do

- **Plan with tasks.** Put each task in a folder or the inbox, tag it, and give
  it a priority, a size, and start and due days.
- **Write in documents and a daily journal.** Rewind keeps earlier copies of
  what you write, so you can restore them.
- **Read your feeds.** Follow RSS, Atom and JSON feeds, then read the articles
  they bring in without leaving Cohesion.
- **Bring in work from other services.** Connect an account and import what
  you need as resources, which stay up to date alongside your own work.
- **Find anything.** Search covers your documents, tasks and resources.
- **Keep areas apart.** Each workspace has its own journal, documents, tasks and
  resources.

## Work with agents

An agent is a persistent assistant with one ongoing conversation. Message an
agent the way you'd message a person. An agent can read your data, make changes
you ask for and remember what it learns.

An agent can also set cues, which wake it when something happens, such as a new
task. When a cue wakes an agent, the agent can read your data and send you
notifications. To change anything, the agent asks you first.

Agents run on your own account with an AI provider: Anthropic, OpenAI, xAI,
Mistral, Fireworks, DeepInfra, Together AI, OpenRouter or any other compatible
endpoint. Settings › Agents sets limits on what they can spend per hour, day and
month.

## Hand off to actors

An actor is an agent on one of your devices, such as Claude Code in one
directory. You or an agent can start an actor session from Cohesion. Cohesion
shows the actor session's status and keeps its transcript, so you can follow the
work and reply from any client.
[actor-agentsdk](https://github.com/cohesionhq/actor-agentsdk) offers Claude
Code as actors, [actor-opencode](https://github.com/cohesionhq/actor-opencode)
offers OpenCode and [actor-codex](https://github.com/cohesionhq/actor-codex)
offers Codex. "Actors" in [Architecture](docs/architecture.md#e-actors) covers
writing your own.

## Add what's missing

Extensions are JavaScript or TypeScript programs that add to Cohesion. An
extension can connect another service, respond to events or run on a timer. A
new extension has no access until you give it some.
"Extensions" in [Architecture](docs/architecture.md#19-extensions) covers how
they work.

## Your data

Your data lives in one database on the computer that runs Cohesion. Apps and
other clients keep no copy, and only clients you've paired can connect.

Some text does leave that computer:

- An agent sends what it reads to the AI provider you chose for it.
- Notifications on the iPhone app pass through Cohesion Cloud, carrying only what
  the notification shows. They're off until you install the extension that sends
  them.
- An extension you give network access can send what it reads to other services.

Settings › Storage shows what Cohesion cleans up over time and when. You can
change those times there.

To move your data to another server, export it with the CLI paired with the
old server, then import it with one paired with the new server. Both servers
keep running.

```sh
cohesion export cohesion-export.db           # paired with the old server
cohesion -c ~/new-server import cohesion-export.db   # paired with the new one
```

Importing replaces everything on the new server and can't be undone. Only data
moves: the new server keeps its own key and the devices paired with it, so
devices paired with the old server pair again. See
[Move to another server](docs/self-hosting.md#move-to-another-server).

## Getting started

You'll need [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 22 or later,
and a C compiler (`gcc` or `clang`, which `build-essential` on Debian and Ubuntu
and the Xcode Command Line Tools on macOS provide). `rustup` installs the Rust
version the repository pins in `rust-toolchain.toml`. Cohesion builds and runs on
Linux and macOS. Choose a computer that stays on if you want Cohesion on your other
devices.

**1. Install Cohesion.**

```sh
git clone https://github.com/cohesionhq/cohesion
cohesion/scripts/install.sh
```

**2. Start the server and leave it running.** The server, `cohesiond`, prints a
setup code the first time it starts. You won't need that code on this computer.

```sh
cohesiond
```

**3. In another terminal, connect the `cohesion` command and create a workspace.**

```sh
cohesion setup --local
cohesion workspace create Home
```

**4. Add a task, a journal entry and a feed.** Replace `<feed-url>` with the address
of an RSS, Atom or JSON feed you read.

```sh
cohesion task add buy milk
cohesion journal add called the dentist
cohesion article feed add <feed-url>
```

**5. Add an agent.** Use your own API key and choose the model agents will use.
To use another provider, replace `anthropic` with `openai`, `xai`, `mistral`,
`fireworks`, `deepinfra`, `togetherai` or `openrouter`, then choose a model from
`cohesion ai models <endpoint>`.

```sh
cohesion ai endpoint create anthropic --api-key "$ANTHROPIC_API_KEY"
cohesion ai use Anthropic claude-sonnet-5-5
cohesion agent create Ada
```

**6. Open the web UI.** Your task, journal entry and agent are there, and everything
else you can do in Cohesion is too. The web UI stays available until you press
Ctrl-C.

```sh
cohesion open
```

### Add another device

Your other devices connect to `cohesiond` over HTTPS on port 3000, so that port has
to be reachable from them.

1. On the computer running `cohesiond`, get a setup code. A setup code works once,
   for 10 minutes. You can also choose Add client… in Settings › Clients.

   ```sh
   cohesion pair
   ```

2. Enter the code on the new device. In an app, enter the code when the app asks
   for it. On another computer, install the command line and connect it:

   ```sh
   git clone https://github.com/cohesionhq/cohesion
   cohesion/scripts/install-cli.sh
   cohesion setup <setup-code>
   cohesion open
   ```

The setup code carries this computer's address. If your devices reach it by another
address, such as a DNS name, start `cohesiond` with that address before you get the
code:

```sh
cohesiond --host cohesion.example.com
```

To run Cohesion in a container or behind a router that forwards a different port,
see [Self-hosting](docs/self-hosting.md).

## Clients

| Client | What it is |
|---|---|
| Web UI | Every Cohesion feature, in your browser. Run `cohesion open`. |
| [apple](https://github.com/cohesionhq/apple) | Cohesion for iPhone and iPad, a native app with a share extension and notifications. |
| [webshell](https://github.com/cohesionhq/webshell) | The web UI as an app for Mac, iPad and Linux. |
| [`cohesion`](docs/cli.md) | The command line, for small jobs and scripts. |

[Cohesion Cloud](https://hellocohesion.cloud) runs Cohesion for you, if you'd
rather not host it yourself.

## Learn more

- [Command line](docs/cli.md): every common command, AI setup and `cohesion api`.
- [Self-hosting](docs/self-hosting.md): installing, server options, pairing and
  containers.
- [Architecture](docs/architecture.md): how Cohesion is built, from the daemon
  and its protocol to agents, actors, clients and extensions, including writing
  an extension or a client that offers actors.
- [Contributing](CONTRIBUTING.md): how to change the code.
- [Changelog](CHANGELOG.md): what changed in each release.

## Contributing

Bug reports, ideas and pull requests are welcome. Read
[Contributing](CONTRIBUTING.md) first, and the [code of conduct](CODE_OF_CONDUCT.md).
To report a security problem, follow [Security](SECURITY.md) rather than opening an
issue.

## License

Cohesion is free software, licensed under the
[GNU Affero General Public License, version 3](LICENSE) only. If you run a changed
copy of Cohesion as a service for others, the licence asks you to offer them its
source. [Licensing](docs/licensing.md) says what that covers, and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) lists the software Cohesion
includes.

The licence covers the code. It doesn't let you use the Cohesion name or logo for
your own product: see [Trademarks](docs/licensing.md#trademarks).
