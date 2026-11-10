# Changelog

What changes in each release of Cohesion, newest first. A release is
`vXXXYYYYY`, the version `cohesion status` shows and the container image's tag
(see "Publish an image" in [CONTRIBUTING.md](CONTRIBUTING.md)). Add an entry
under "Unreleased" with any change users would notice.

## Unreleased

- The daemon keeps databases: tables in a workspace whose columns each hold
  one type of value (text, whole number, decimal, currency, checkbox or
  date), whose rows may each have a document of their own, and whose rows
  can be filtered and sorted on any column. A row's document stays out of
  the document list and search. The wire gains `listDatabases`,
  `getDatabase`, `createDatabase`, `renameDatabase`, `deleteDatabase`,
  `listDatabaseProperties`, `createDatabaseProperty`,
  `renameDatabaseProperty`, `setDatabasePropertyType`,
  `moveDatabaseProperty`, `deleteDatabaseProperty`, `listDatabaseRows`,
  `getDatabaseRow`, `createDatabaseRow`, `deleteDatabaseRow`,
  `setDatabaseValue` and `createDatabaseRowDocument`, the `databases(ws)`
  and `database(id)` invalidations, `hello.vocabulary.database_data_types`,
  and a `databaseRow` document type, which a client meets only by opening a
  row's document. The iPhone app needn't show databases.
- A database column can be a formula: a JavaScript function of the row,
  such as `(row) => row.Price * row.Quantity`, whose result fills the
  column in every row and is computed again when the row or the columns
  change. Its results filter and sort like any value. A row the formula
  fails on stays empty, and the formula shows why. Formulas run with
  nothing but the row, under a time and memory limit. The wire gains
  `setDatabasePropertyFormula`, a `formula` on `createDatabaseProperty`
  and on `DatabaseProperty` (`{source, error}` or `null`), and
  `setDatabaseValue` refuses a formula's column.
- A database and a row of one can be linked to, as
  `cohesion://database/<ws>/<id>` and
  `cohesion://database-row/<ws>/<database>/<id>`, and `resolveLink`,
  `resolveEmbeds` and `backlinksOf` answer for them: `EmbedKind` and
  `LinkKind` gain `database` and `databaseRow`, and `EmbedItem` gains
  `database` (its properties, its first 5 rows and its `row_count`) and
  `databaseRow` (the row's cells, its database's title and properties). A
  row's title is its first text column's value, or "Untitled". A link to a
  row's document resolves as the row, and what a row's document links to
  lists the row among its backlinks. `hello.vocabulary.embed_kinds` gains
  their words. Clients that switch on an embed's `kind` or `item.kind`
  need the two new cases; the iPhone app can show them by title, excerpt
  and kind word until it draws them. The web UI doesn't show them as cards
  yet.
- The web UI shows databases. Databases, in each workspace's sidebar and
  ⌘K, lists them, and a database opens as a table: add a column of any type,
  rename it, change its type, move it or delete it; add and delete rows; edit
  a cell in place; and sort and filter on any column, which Cohesion does on
  the server. A formula's column is read-only and shows why it failed. A row
  opens its document over the page.
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
