# Serving the WebUI

cohesiond is built with the web UI (`web/`) inside it and hands it to any
paired client that asks. A client that shows the UI, such as `cohesion open`
or a WebUI harness (an app that is nothing but this page in a web view), fetches the bundle, unpacks it, and serves it to
the page itself, so the UI always matches the daemon it talks to and no client
has to ship its own copy.

This page is for writing such a client. `cohesion open`
(`cohesion/src/web_proxy/`) is the reference implementation.

## How the daemon gets it

`cohesionkit/build.rs` walks `web/build/` (the output of `npm run build`),
compresses each file on its own as raw deflate, and embeds the results in the
daemon. So build the UI before building `cohesiond`; `scripts/install-server.sh`,
`scripts/install.sh` and the `Dockerfile` do. A daemon built with no `web/build/` still runs: its `Hello`
carries no hash and `WebBundle` answers an error.

## Knowing which version it has

Every session's first frame is the daemon's `hello`, and it carries the
bundle's hash:

```json
{
  "seq": 0,
  "type": "hello",
  "protocol": 1,
  "server": "cohesiond/0.1.0",
  "client_name": "iPad",
  "web_bundle_hash": "3f9a…",
  "vocabulary": { "permissions": […], "hooks": […], … }
}
```

Any change to the UI's files gives a new hash, and rebuilding unchanged files
keeps the old one. A client that keeps an unpacked copy, keyed by its hash, can compare the
two on connect and skip the download while they match. `web_bundle_hash` is
`null` when the daemon was built without the UI. `vocabulary` is what the
daemon calls each of its kinds (see `docs/architecture.md`).

## Fetching it

Send `webBundle` like any other request. It takes no arguments and works over
every transport: the WebSocket, the Unix socket and HTTPS `/command`.

```json
{ "id": 1, "type": "webBundle" }
```

The reply's payload is a `WebBundle`:

```json
{
  "hash": "3f9a…",
  "files": [
    { "path": "index.html", "data": "…" },
    { "path": "_app/immutable/entry/start.B1x2.js", "data": "…" }
  ]
}
```

- `hash` is the same value as in `hello`.
- `path` is relative to the build's root, with `/` separators.
- `data` is the file's bytes compressed as raw deflate (RFC 1951, with no
  zlib or gzip header), then base64-encoded.

The types are `WebBundle` and `WebFile` in
`cohesion-core/src/protocol/server_requests.rs`, and appear in the web UI's
`generated.ts`. The whole reply is about 600 KB.

## Unpacking it

Decode each file's base64, then inflate it as raw deflate. Each platform has
this built in or a standard library for it:

```rust
// flate2
let mut bytes = Vec::new();
DeflateDecoder::new(file.deflated()?.as_slice()).read_to_end(&mut bytes)?;
```

```swift
// Foundation: despite its name, `.zlib` is raw deflate.
struct WebBundle: Decodable { let hash: String; let files: [WebFile] }
struct WebFile: Decodable { let path: String; let data: Data } // base64 by default

let bytes = try (file.data as NSData).decompressed(using: .zlib) as Data
```

```js
// Browsers: DecompressionStream.
const deflated = Uint8Array.from(atob(file.data), (c) => c.charCodeAt(0));
const bytes = await new Response(
  new Blob([deflated]).stream().pipeThrough(new DecompressionStream('deflate-raw'))
).arrayBuffer();
```

Don't use a zlib or gzip decoder: the data has neither format's header.

## Serving it

The UI is a SvelteKit single-page app, served from the root of its origin:

- Serve each file at `/<path>`, with a `Content-Type` that matches its
  extension.
- Answer any path the bundle doesn't have with `index.html`, so the UI's own
  routes load on refresh.
- Files under `_app/immutable/` have content hashes in their names, so they
  may be cached forever (`Cache-Control: public, max-age=31536000, immutable`).
  Everything else should be revalidated (`no-cache`).

The page also expects its origin to give it a way to the daemon, which
`cohesion open` provides over the CLI's own connection:

- `GET /client` answers `{ "workspace": <id or null>, "target": "<where the
  daemon is>" }`.
- `/connect` is a WebSocket that relays text frames to and from a daemon
  session, unchanged.
- Both check a token the page presents, as `Authorization: Bearer <token>` on
  `/client` and `?token=<token>` on `/connect`. The page reads it from
  `#token=<token>` in the URL it is first opened at and keeps it in
  `localStorage`.

A WebUI harness can serve the files through a
custom URL scheme handler, but such a handler can't answer a WebSocket, so
`/connect` still needs a loopback server or another way into the page.

## In the WebUI harnesses

The WebUI harnesses in [webshell](https://github.com/cohesionhq/webshell)
share CohesionWebKit, a Swift package that pairs with a server, keeps the bundle and serves it from a
custom URL scheme, with no loopback server at all. It carries `/connect`
into the page by replacing `WebSocket` for that one URL; its README
describes how.

## What a host provides

A host is whatever shows the page: `cohesion open` in a browser, or a
WebUI harness's web view through CohesionWebKit. Each provides the same things, and a
feature that needs something more from its host is a change to every host
(below):

- **The bundle**, served as above, the copy matching the daemon's
  `web_bundle_hash`.
- **The token**, handed over once as `#token=<token>` in the URL the page
  is opened at; the page keeps it and presents it on `/client` and
  `/connect`. Pairing is the host's business, never the page's.
- **`/client` and `/connect`**, answered over the host's own paired
  session.
- **`--titlebar-inset`** on `<html>`: how much of the window's top the
  host covers (its window controls, a status bar), 0 in a browser. The
  page keeps that strip clear of anything clickable, since the Mac WebUI
  harness drags the window there.
- **Nothing for back and forward.** The page steps through its own
  history on ⌘[ ⌘], ⌘← ⌘→, ⌥← ⌥→ and a mouse's side buttons
  (`shell/shortcuts.svelte.ts`), because a web view has no browser chrome;
  CohesionWebKit turns its swipe gestures off rather than handle them.
- **Reloading on update.** When the daemon's protocol differs from the
  page's, the page stops and asks to be reloaded
  (`connection.svelte.ts`); a reload fetches the new bundle by its hash.
  The iPhone app shows no web page, so this never reaches it.

Push notifications and Live Activities don't go through the page or its
host at all; see "Push notifications" in `docs/architecture.md`.

## The page's side is a stable API

Every client above stands in for the same few things the page expects of
its origin, so they are an API: a change to them breaks the WebUI harnesses'
bridge as surely as it breaks `cohesion open`. Keep these stable, and
change the proxy and CohesionWebKit together when one must change:

- `web/src/lib/protocol/session.ts`: the token read from `#token=<token>`
  and kept in `localStorage` under `cohesion.token`, `Authorization: Bearer`
  on `/client`, and `/connect?token=<token>` on the page's own host, built
  from `location.host`.
- `GET /client`'s status: 401 means the token was refused, anything else
  that the daemon is away.
- `--titlebar-inset`, which the WebUI harnesses set on `<html>` after each load to
  the height the page must keep clear at its top, and which the page's
  layout reads.
- What `connection.svelte.ts` uses of a `WebSocket`: the constructor with a
  URL, `onmessage` with text `data`, `onclose`, `send` and `close`. The
  bridge's stand-in implements those (and `onopen`, `onerror` and
  `readyState`); a page that starts relying on more of `WebSocket`, such as
  binary frames or subprotocols, needs the bridge to grow first.
