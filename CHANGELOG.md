# Changelog

What changes in each release of Cohesion, newest first. A release is
`vXXXYYYYY`, the version `cohesion status` shows and the container image's tag
(see "Publish an image" in [CONTRIBUTING.md](CONTRIBUTING.md)). Add an entry
under "Unreleased" with any change users would notice.

## Unreleased

- An agent's Monitoring list, in the column beside its conversation, has
  Stop following on hover or focus beside Resolve. The agent then no longer
  wakes when that actor session stops, until it tells the session something
  again. Clients send `unfollowActorSession` with the session and the agent;
  a session another agent follows is left as it is.
- A task's description links as a document does: a task whose description
  links to an item, as a card or in a sentence, shows under that item's
  Backlinks, and moves off it when the link goes or the task is deleted.
  When the daemon is upgraded, it fills the list from existing tasks.
- An item's panel shows Backlinks: the documents and journal entries that
  link to the item, wherever the link is in their text. Click one to open it
  over the page. When nothing links to the item, the section doesn't show.
  Clients ask with `backlinksOf`, which answers with an `Embed` for each
  linking item, and refetch on the new `backlinks` invalidation of the item's
  kind and id. When the daemon is upgraded, it fills the list from existing
  documents.
- In a document, a journal entry, Today or a task's description, a link to
  an item alone on its line shows as a card of it: a task with its status and
  due day, a document, a resource, an agent, an actor session with where it
  stands, an article with its feed, each with the start of its text. Click it
  to open the item over the page. Type `@` to look up a task, document,
  resource, agent, session or article and link it; on an empty line it
  becomes a card. Articles come last in the list.
- Any `cohesion://` link resolves to the item it names in one shape,
  `Embed`: `resolveLink` returns its title, the start of its text, what kind
  it is (worded in `hello.vocabulary.embed_kinds`) and the item itself, or
  `notFound` once it is deleted. Links gain two kinds,
  `cohesion://resource/<ws>/<id>` and `cohesion://article/<id>`, which agents
  now write too. `resolveEmbeds` splits any Markdown into its text and the
  links that stand alone on their lines, each resolved, so a client can show
  them as cards without reading links itself. The iPhone app can show cards
  from it; until it does, it shows the new kinds as plain links.
- An actor session shows its subagents. In its transcript, the tool use that
  launched one is its line, with where it stands, which opens it as a thread
  of its own, read-only, with a way back up to the session; the column beside
  shows the session's family, the top session and every subagent nested by
  depth with the open one marked, and what a subagent asks can be answered in
  the session. Actor
  clients announce a subagent with a new `launch` report and report into it
  with `subagent` on `actorSessionUpdate`, and answers reach it with
  `subagent` on `sendActorSessionMessage`; the wire gains
  `listActorSubagents`, `listActorSessionFamily`,
  `listUnansweredSubagentRequests` and `subagent` on `ActorSession`. The actor clients and the iPhone app need the same to show
  them; a client that doesn't report subagents works as before.
- `cohesion setup` without `--name` names the client after the computer and
  the client, such as `apollo (CLI)`, instead of the hostname alone, so
  clients on one computer can be told apart in Settings › Clients and in actor
  titles. The container's own CLI is `Container (CLI)`. Existing clients keep
  their names.
- An actor session shows as one activity for its whole life. When it works
  again, its activity is reopened instead of a new one being added, so every
  client lists each session once. Upgrading merges a session's earlier
  activities into its first, which takes the newest one's title and status.
- A connector can draw its resources: its optional `renderResource(resource,
  kind)` returns a CohesionUI interface (headings, text, checkboxes, links and
  overviews of labelled values), which the web UI shows on the resource's page
  in place of its raw contents. `renderResource` is a new request; the iPhone
  app can draw the same blocks from it. A connector without it shows as
  before.
- Documents file into folders, which nest without limit; a document in no
  folder is in the Inbox. The Documents page shows the folders as a tree
  beside the list, under Inbox and All: drag a document onto a folder to file
  it, or a folder onto another to nest it, and a document's ⋯ menu has Move
  to. Deleting a folder deletes the folders inside it and moves their
  documents to the Inbox. Journals stay out of folders. Scripts reach them
  through `Workspace.documentFolders()`, `createDocumentFolder()`,
  `Document.folder()` and `moveTo()`, and the `DocumentFolder` class. The
  wire gains `listDocumentFolders`, `createDocumentFolder`,
  `renameDocumentFolder`, `moveDocumentFolder`, `deleteDocumentFolder`,
  `moveDocument`, a `folder_id` on `Document` and `createDocument`, and the
  `documentFolders(ws)` invalidation; the iPhone app can offer the same.
- An agent's ⋯ menu has Memories…, which shows every memory the agent can
  see: its own, its team's and the global ones, each with the day it was
  remembered. Forget removes one, with the same confirmation as Settings ›
  Memories. The iPhone app can offer the same from `listMemories` and
  `forgetMemory`.
- Cohesion for Linux: a WebUI harness in GTK 4 and libadwaita, in
  `cohesionhq/webshell` under `gtk/`. It pairs with a setup code, shows the
  web UI in WebKitGTK, and offers a newer web UI once the server serves one.
- Settings › Storage shows what fills the database under Usage, with Backup
  its own section after it: one bar split by area, then agent chats, actor sessions, documents and tasks, feeds and articles,
  resources, memories, Rewind versions, everything else and free space, each
  with its size and share. `getDatabaseSize` answers each area's bytes in a new
  `areas` field; the iPhone app can show the same breakdown from it.
- The technical documentation is rebuilt from the code as one reference,
  [Architecture](docs/architecture.md). It replaces the tour and the separate
  pages on actors, extensions and serving the web UI.
- Cohesion is open source, under the GNU Affero General Public License, version
  3 only. See [Licensing](docs/licensing.md).
- The repository starts a new history at `cohesionhq/cohesion`, with a code of
  conduct, a security policy, issue and pull request templates, and checks that
  run on every pull request.
- `THIRD_PARTY_NOTICES.md` lists the crates in `cohesiond` and `cohesion`, and
  the web UI's build lists the packages it bundles in `third-party-licenses.md`.
- Extensions install from a URL: under Extensions, Install from URL downloads
  the script and opens the same review as a file. A download stops at 5 MB.
- `cohesion export <file>` copies all of a server's data, images included, to
  a file, and `cohesion import <file>` replaces another server's data with it,
  then restarts that server. Both servers keep running. The importing server
  keeps its own key and paired clients, so devices paired with the old server
  pair again. The protocol gains `exportDatabase`, `readDatabaseExport`,
  `beginDatabaseImport`, `writeDatabaseImport` and `finishDatabaseImport`;
  no other client needs them.
- In an agent's conversation, each session under Monitoring has an archive
  button to resolve it, shown on hover or focus and always on touch. It asks
  first while the actor is still working.
- On the Tasks page, the filters in use sit before the Filters button, so the
  button stays put as filters are picked, and its panel opens leftwards from it.
  Popovers keep 16px from the window's edges.
- `rust-toolchain.toml` pins the Rust release Cohesion builds with.
- The web UI's second design pass: the Dashboard's greeting leads the page, and a
  task due today can be completed from it; documents read as rendered Markdown
  away from the line being edited, in a prose-width column; Documents rows show
  where their text starts; an actor session's header leads with its status; a
  disabled button turns grey; a working session's mark is grey, so the accent
  means unread.
