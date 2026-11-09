# Licensing

Cohesion is copyright Jackson Zamorano and is licensed under the GNU Affero
General Public License, version 3 only (`AGPL-3.0-only`). The full text is in
[`LICENSE`](../LICENSE). Every crate in the workspace but one, and the web UI's
`package.json`, declare it. The exception is `cohesion-rs`, which is `MIT`, with
its own [`LICENSE`](../cohesion-rs/LICENSE).

## What the licence asks

You may run, study, change and share Cohesion. If you share a copy, changed or
not, share its source under the same licence. If you change Cohesion and let
other people use your changed copy over a network, such as a hosted `cohesiond`
or its web UI, offer those people the source of your changed copy.

Running an unchanged copy for yourself, your family or your team asks nothing of
you beyond keeping the licence and notices with it.

## Why these licences

The core is copyleft. `cohesiond`, the CLI and the embedded web UI are
`AGPL-3.0-only`, so anyone who hosts a changed copy for other people has to
offer them its source. `extensions`, `relay` and the two `hellocohesion`
websites use the same licence.

Clients are permissive. They talk to `cohesiond` over its protocol, so they
don't need to be copyleft. The iOS, iPadOS and macOS app (`apple`) and both
WebUI harnesses in `webshell`, the Mac app and the Linux app in `gtk/`, are
`Apache-2.0`, partly because copyleft can't ship through the App Store.

Interoperability code is `MIT`: the libraries and actors other people build on
(`ts`, `actor-kit`, `actor-agentsdk`, `actor-opencode`, `actor-codex` and
`cohesion-proxy`), and `cohesion-rs` in this repository. That way anyone can
write an actor or client without taking on the core's terms.

`cohesion-rs` exists so a Rust client doesn't have to link the core. It holds
what a client needs to talk to a daemon and nothing else: keys, certificates,
the TLS key fingerprint, setup codes, the socket handshake, the private-file
helpers, and the client's view of the protocol's envelope. It depends on no
other crate in this repository. `cohesion-core` depends on it and re-exports
what it shares. The Linux app in `webshell/gtk/` links `cohesion-rs` rather
than `cohesion-core`, which is why it can be `Apache-2.0`. Anything whose
purpose is serving or storing the daemon's state, like the domain models, the
full command and reply types or the storage layer, stays `AGPL-3.0-only` in
`cohesion-core`. The client protocol keeps those parts as raw JSON instead.

`webshell/gtk/src/bridge.js` is a copy of the script in CohesionWebKit's
`SocketBridge`, and CohesionWebKit has no licence yet.

Cohesion Cloud, the hosted service at [hellocohesion.cloud](https://hellocohesion.cloud),
runs `cohesiond` from this repository. Its own service code in `cloud` is
proprietary, all rights reserved. It's the paid product and isn't open source.

## Licences by repository

Each repository in the `cohesionhq` organisation carries its own `LICENSE`.

| Repository | Licence |
| --- | --- |
| `cohesion`, except `cohesion-rs/` | `AGPL-3.0-only` |
| `extensions`, `relay` | `AGPL-3.0-only` |
| `hellocohesion.com`, `hellocohesion.cloud` | `AGPL-3.0-only` |
| `apple`, `webshell` | `Apache-2.0` |
| `cohesion/cohesion-rs/` | `MIT` |
| `ts`, `actor-kit`, `actor-agentsdk`, `actor-opencode`, `actor-codex`, `cohesion-proxy` | `MIT` |
| `cloud` | Proprietary, all rights reserved |
| `CohesionWebKit` | No licence yet |
| `cohesiond-macos` | No licence yet. It belongs under `AGPL-3.0-only` once it gets one. |

The permissive licences apply only to code those repositories hold. They don't
relax this repository's: a program that links code from `cohesion` other than
`cohesion-rs` is bound by `AGPL-3.0-only` whatever its own repository says.

## Third-party software

[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) lists every crate compiled
into `cohesiond` and `cohesion`, with its licence text, and the code vendored in
`cohesion-js/vendor/`. The web UI's build lists the packages it bundles in
`third-party-licenses.md`, beside `index.html`, so the daemon serves it with the
web UI. Run `node scripts/third-party-notices.mjs` after changing `Cargo.lock`;
CI fails when the file is out of date.

## Contributions

By opening a pull request, you agree that your contribution is licensed under
`AGPL-3.0-only`, the same as the rest of Cohesion, or under `MIT` where it
changes `cohesion-rs`.

## Trademarks

The licence covers the code, not the name. You're welcome to say that your
project works with Cohesion, and a copy you've changed can keep the name while
you use it yourself. If you publish or host a changed copy for others, give it
your own name and logo, so nobody mistakes it for Cohesion. The logo is in
[`docs/logo.svg`](logo.svg). The apps' icon is the logo on a tile, in
[`docs/app-icon.svg`](app-icon.svg).
