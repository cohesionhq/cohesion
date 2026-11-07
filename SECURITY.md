# Security

Cohesion holds your tasks, documents and journal, signs in to AI providers with
your keys, and is meant to be reachable from your other devices. A security
problem in it matters, and reports are welcome.

## Report a vulnerability

Report it privately, not in a public issue, through GitHub's private
vulnerability reporting: on the repository's **Security** tab, choose
**Report a vulnerability**, or open
<https://github.com/cohesionhq/cohesion/security/advisories/new>.

Include what you found, the version (`cohesion status` shows it, or the commit
you built), and the steps to reproduce it. You'll get a reply within 7 days.
Once a fix is out, the advisory is published and credits you, unless you'd
rather it didn't.

## Supported versions

Only the latest release and `master` get security fixes. Update to the newest
image or commit before reporting.

## What's in scope

The boundaries the daemon defends are described under "Trust model" in
[`CLAUDE.md`](CLAUDE.md) and in [`docs/architecture.md`](docs/architecture.md):

- Pairing and the setup code, the Unix socket and the `cohesion open` loopback
  proxy, which must give nothing to an unpaired stranger on the network or a
  local process.
- Wire input from a paired client, which must be well-formed or refused.
- Extensions, which reach only the mounts their grants allow.
- Agents, which may read untrusted text: a chain an event started must not gain
  the rights of one you started.
- Feeds, fetched pages and providers' replies, which are parsed as untrusted.

Out of scope: anything that needs a paired client, an installed extension with
the grant in question, or control of the computer running `cohesiond`, since
those are trusted. The same goes for addresses a `--proxy-protocol` router
claims, which are shown but never used to admit anyone.

Cohesion Cloud and the apps in other repositories take reports the same way, in
their own repositories.
