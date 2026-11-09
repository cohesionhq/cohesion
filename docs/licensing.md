# Licensing

Cohesion is copyright Jackson Zamorano and is licensed under the GNU Affero
General Public License, version 3 only (`AGPL-3.0-only`). The full text is in
[`LICENSE`](../LICENSE). Every crate in the workspace and the web UI's
`package.json` declare it.

## What the licence asks

You may run, study, change and share Cohesion. If you share a copy, changed or
not, share its source under the same licence. If you change Cohesion and let
other people use your changed copy over a network, such as a hosted `cohesiond`
or its web UI, offer those people the source of your changed copy.

Running an unchanged copy for yourself, your family or your team asks nothing of
you beyond keeping the licence and notices with it.

## Programs that talk to Cohesion

Clients, actors and extensions talk to `cohesiond` over its protocol or run as
scripts inside it, and apart from the Linux app described below, nothing in this
repository is compiled into them. Their own repositories each choose their own
licence. This covers the iPhone app
([apple](https://github.com/cohesionhq/apple)), the Mac app
([webshell](https://github.com/cohesionhq/webshell)), which loads the web UI
from the daemon it's paired with, and the actor clients.

Cohesion Cloud, the hosted service at [hellocohesion.cloud](https://hellocohesion.cloud),
runs `cohesiond` from this repository. Its own service code lives elsewhere and
isn't part of Cohesion.

## Licences by repository

Each repository in the `cohesionhq` organisation carries its own `LICENSE`.

| Repository | Licence | Why |
| --- | --- | --- |
| `cohesion` | `AGPL-3.0-only` | The daemon, the CLI, the core crates and the embedded web UI. |
| `webshell` | `Apache-2.0`, except `gtk/` | The Mac shell and everything outside `gtk/`. Copyleft can't ship through the App Store. |
| `webshell/gtk/` | `AGPL-3.0-only` | The GTK app links `cohesion-core` (a path dependency in `gtk/Cargo.toml`), so it is a combined work with this repository. |
| `apple` | `Apache-2.0` | The iPhone app ships through the App Store, for the same reason. |
| `ts`, `actor-kit` | `MIT` | Libraries for writing actors and clients. |
| `relay`, `extensions`, the actor repos | `AGPL-3.0-only` | Same as this repository. |
| `cloud` | Proprietary, all rights reserved | The paid hosted product. It isn't open source. |

The permissive licences apply only to code those repositories hold. They don't
relax this repository's: a program that links code from `cohesion` is bound by
`AGPL-3.0-only` whatever its own repository says.

## Third-party software

[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) lists every crate compiled
into `cohesiond` and `cohesion`, with its licence text, and the code vendored in
`cohesion-js/vendor/`. The web UI's build lists the packages it bundles in
`third-party-licenses.md`, beside `index.html`, so the daemon serves it with the
web UI. Run `node scripts/third-party-notices.mjs` after changing `Cargo.lock`;
CI fails when the file is out of date.

## Contributions

By opening a pull request, you agree that your contribution is licensed under
`AGPL-3.0-only`, the same as the rest of Cohesion.

## Trademarks

The licence covers the code, not the name. You're welcome to say that your
project works with Cohesion, and a copy you've changed can keep the name while
you use it yourself. If you publish or host a changed copy for others, give it
your own name and logo, so nobody mistakes it for Cohesion. The logo is in
[`docs/logo.svg`](logo.svg). The apps' icon is the logo on a tile, in
[`docs/app-icon.svg`](app-icon.svg).
