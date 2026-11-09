# Security model

The trust model: who and what the daemon trusts, and the boundaries where it
validates. What the code does about each boundary (file modes, secrets, limits)
is in "Trust boundaries and secrets" in
[architecture.md](architecture.md#22-trust-boundaries-and-secrets); reporting a
problem is in [SECURITY.md](../SECURITY.md).

## Trust model

Validate at these boundaries and nowhere else:

- **Before pairing**: `/register` with the setup key, the Unix socket's
  handshake and file permissions, and the `cohesion open` loopback proxy.
  A stranger on the LAN or a local process gets nothing else.
- **A connection's address** is a claim, not a credential. It is the peer's own
  unless the peer is named by `--proxy-protocol` (the pass-through Router), whose
  PROXY v2 header is then required and believed; no other peer can claim one.
  The header is unauthenticated, so someone on the path can write false addresses
  into what clients show, and that must stay all they can do: nothing is ever
  admitted or refused by address ("Behind a pass-through router" in
  [architecture.md](architecture.md#behind-a-pass-through-router)).
- **Wire input** from a paired client is well-formed or refused, but a
  paired client is trusted: it may do anything the protocol offers, except
  open pairing when the security settings keep that to clients on this
  computer (a `FromClient` route, the one kind that sees its sender).
  A client's offers and call results are wire input.
- **Extensions** are trusted once installed and a grant is on; a grant
  decides which mounts a scope gets, and nothing inside a mount
  second-guesses the grant.
- **The LLM** is semi-trusted: it reads untrusted text (articles, feeds,
  imported resources). Everything an agent does is a turn of its
  conversation or a fork of it (one turn on a copy of the conversation's
  thread, or, for a cue, on a fresh thread of its own), and its rights follow from one thing, who started its chain
  (`Initiative`, on the trace, inherited by every fork the chain wakes and
  kept on an actor session's follow): `AgentRole`
  (`cohesion-js/src/engine/agent_role.rs`) alone turns that into mounts and
  a database. A capability a role lacks is a mount it doesn't have or a
  database that refuses the write, never a check inside a mount.
  - **Started by the user** (a message, a pick, and the questions and
    actor follow-ups that descend from them): everything the conversation
    may do.
  - **Started by an event** (a cue firing): reads, sends notifications,
    works with actors, and asks with `ask_choice` before any change; no
    `network`, no writes, no cues or memories set. The user's pick starts
    a chain they started. A chain never gains rights.
  - **A cue's function**, deciding whether its fork wakes, reads and sends
    notifications, nothing more.

  Forks are counted in hops and logged, never limited; the spend limits
  hold them. Hooks raising hooks stop at the depth limit (`MAX_DEPTH`, 3).
- **External services** (providers' APIs, models.dev, fetched pages and
  feeds) are untrusted: parse defensively, bound sizes and time.

Review against real threats to these boundaries, not hypothetical ones a
trusted party could mount.
