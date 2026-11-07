# Self-hosting

`cohesiond` is the server that holds your data. This page covers installing it,
its options, pairing clients and running it in a container. To use it once it's
running, see [Command line](cli.md).

## Install

Run a script from a clone of this repo. Each one works from any directory.

| Script | Installs |
|---|---|
| `scripts/install.sh` | `cohesiond` and `cohesion` |
| `scripts/install-server.sh` | `cohesiond` |
| `scripts/install-cli.sh` | `cohesion` |

The scripts use `cargo install`. The first two also build the web UI first,
because `cohesiond` embeds it and hands it to clients. To run both programs from
an image instead, see [Run in a container](#run-in-a-container).

To build without installing:

```sh
cargo build --workspace
```

## Run cohesiond

```sh
cohesiond --config ./scratch/daemon --socket /tmp/cohesion.sock
```

- `--config <DIR>` holds `cohesion.db`, the server key and `config.json`.
  Without this flag, `cohesiond` uses your platform's per-user config directory.
- `--socket <PATH>` is the Unix socket local clients connect to. The default is
  in the runtime directory on Linux, and `<config>/cohesiond.sock` elsewhere.
- `--host <HOST>` is the IP address or DNS name in the setup code remote clients
  use, such as `cohesion.example.com` for a server behind a forwarded port. The
  default is this computer's outbound address.
- `--port <PORT>` (env `COHESIOND_PORT`, default `3000`) is the HTTPS port
  `cohesiond` listens on.
- `--public-port <PORT>` (env `COHESIOND_PUBLIC_PORT`) is the port remote clients
  connect to, when a router forwards a different one to `--port`. The default is
  `--port`. The setup code carries this port, so clients need no other setting.
- `--log-level <LEVEL>` (env `COHESIOND_LOG`, default `info`) is the least severe
  level printed: `debug`, `info`, `warn` or `error`. SQL statements are logged at
  `debug`.
- `--exit-with-parent` exits when stdin closes, for apps that run `cohesiond` as a
  child and pass it a pipe.
- `--proxy-protocol <ADDRESS>` names a pass-through router in front of
  `cohesiond`, which writes a PROXY v2 header with each client's real address.
  Repeat the flag for several routers. Connections from a named address must open
  with the header, and no other peer can claim an address. This is off by default.
  See "Behind a pass-through router" in [Architecture](architecture.md).

### config.json

Every flag but `--config` can also be set in `config.json` in the config
directory. `cohesiond` reads this file at startup and never writes to it. A flag
or its env var wins over the file, and the file wins over the default. Every key
is optional, and an unknown key stops `cohesiond` from starting.

| Key                | Flag                 | Value                                                      |
| ------------------ | -------------------- | ---------------------------------------------------------- |
| `host`             | `--host`             | An IP address or DNS name, such as `"cohesion.example.com"` |
| `port`             | `--port`             | A port number                                              |
| `public_port`      | `--public-port`      | A port number                                              |
| `socket`           | `--socket`           | A path, relative to the config directory unless absolute   |
| `log_level`        | `--log-level`        | `"debug"`, `"info"`, `"warn"` or `"error"`                 |
| `exit_with_parent` | `--exit-with-parent` | `true` or `false`                                          |
| `proxy_protocol`   | `--proxy-protocol`   | A list of IP addresses, such as `["10.0.0.2"]`             |

```json
{
  "host": "cohesion.example.com",
  "port": 3000,
  "public_port": 8443
}
```

## Pair clients

When `cohesiond` starts, it refreshes the model catalog. If no client is
registered yet, it also prints a setup code to stdout. A setup code works once,
for 10 minutes. To make another, run `cohesion pair` or choose Add client… in
Settings › Clients.

Settings › Security can limit pairing to clients on the computer running
`cohesiond`, and it lists security activity.

To connect the command line, see [Connect the CLI](cli.md#connect-the-cli).

## Run in a container

The `Dockerfile` builds one image holding `cohesiond` and the `cohesion` CLI.
Build it from the repo root. The build copies the working tree, uncommitted edits
included.

```sh
docker build -t cohesion .
```

Published images are in `ghcr.io/cohesionhq/cohesion`, each tagged with its
version, such as `v00100042` ([CHANGELOG.md](../CHANGELOG.md) lists what changed).
To publish one, see "Publish an image" in [Contributing](../CONTRIBUTING.md).

```sh
docker pull ghcr.io/cohesionhq/cohesion:<tag>
```

Rootless Podman runs the `docker` commands below as written, with a named volume.

Run the image with a volume for its data and the port published. Arguments after
the image name go to `cohesiond`. `--host` is the address clients reach this
computer at, which the container can't work out for itself.

```sh
docker run -d --name cohesion -p 3000:3000 -v cohesion-data:/data \
  cohesion --host 192.168.1.20
```

If you publish a different host port, give that port too: `-p 8443:3000` needs
`--public-port 8443`. The container's time zone decides when a new day starts and
what "today" means to an agent. The zone is UTC unless you set one, such as
`-e TZ=America/Chicago`.

The CLI in the container needs no setup. The first time it runs, it connects
through the Unix socket, as `setup --local` does, and lists itself as "Container
CLI".

```sh
docker exec cohesion cohesion status
docker exec cohesion cohesion workspace
```

Add `-it` for anything that asks a question. The image has no text editor, so
`cohesion journal` needs another way in: use `journal add`, or pipe the text with
`docker exec -i`.

To pair a client outside the container, ask for a setup code and use it there:

```sh
docker exec cohesion cohesion pair
cohesion setup <setup-code>            # on the other computer
```

`docker logs cohesion` also shows a setup code from a first start, while no client
is registered yet.

### Data

`/data` holds `cohesiond/` (the database, server key and `config.json`, which
works as under [config.json](#configjson)) and `cohesion/` (the CLI's identity).
Keep the volume, and a new container is the same server to its clients. The image
runs as uid 1000, so a bind mount in place of the volume must be writable by that
user.

### Stopping

`docker stop` gives `cohesiond` 10 seconds, but `cohesiond` waits up to 5 minutes
for work in flight. `docker stop -t 300` lets that work finish. A stop cut short
is cleaned up at the next start.

### The web UI

The image doesn't publish the web UI. `cohesiond` still carries the web UI and
hands it to paired clients, but nothing in the container serves it. The only port
is the mutual-TLS one, which turns away a browser. To see the web UI, run
`cohesion open` on a paired computer.
