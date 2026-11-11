# Changelog

What changes in each release of Cohesion, newest first. A release is
`vXXXYYYYY`, the version `cohesion status` shows and the container image's tag
(see "Publish an image" in [CONTRIBUTING.md](CONTRIBUTING.md)). Add an entry
under "Unreleased" with any change users would notice.

## Unreleased

- An agent keeps an agenda: the steps it means to take next, in order,
  which it sets out, finishes and clears with the tools `agenda_append`,
  `agenda_finish` and `agenda_reset`. Every thread of its conversation,
  forks and cue runs included, shares it and hears when another changes
  it. The web UI shows the agenda in the column beside the conversation,
  and each change as a line in the transcript. `ListAgentAgenda` lists it,
  and agent messages gain the kind `agenda`; the iPhone app and other
  clients show that kind as a plain line until they add a view for it.

- Contacts can be linked and embedded like tasks and documents, as
  `cohesion://contact/<ws>/<id>`: a link alone on its line shows the
  contact as a card, `@` and ⌘K find contacts by name or notes, and a
  contact's panel has Copy link and Backlinks. Contacts are in workspace
  search (`searchWorkspace` answers `entity_kind` `contact`), `resolveLink`
  answers an embed of kind `contact`, and `LinkKind` gains `contact`.
  When the daemon is upgraded, it indexes existing contacts. The iPhone app
  and other clients that switch on these kinds need the new case.


- `cohesion mcp` is a local MCP server, over stdin and stdout, for an MCP
  client such as Claude Code. Its one tool, `code`, runs a script in
  cohesiond and returns its result, and the script API's declarations are
  the resource `cohesion://cohesion.d.ts`.
- An agent's Monitoring list, in the column beside its conversation, has
  Stop following on hover or focus beside Resolve. The agent then no longer
  wakes when that actor session stops, until it tells the session something
  again. Clients send `unfollowActorSession` with the session and the agent;
  a session another agent follows is left as it is.
- A task's description links as a document does: a task whose description
  links to an item, as a card or in a sentence, shows under that item's
  Backlinks, and moves off it when the link goes or the task is deleted.
  When the daemon is upgraded, it fills the list from existing tasks.
- A document folder can be a table: once it has a column, each document in
  it is a row, titled by the document's title, and each column holds one
  type of value (text, whole number, decimal, currency, checkbox, date or
  choice).
  Rows filter and sort on any column, the title included, on the daemon.
  Deleting a folder's last column makes it a plain folder again, and the
  Inbox never has columns. A row is an ordinary document: it shows in the
  document list and search, keeps versions and goes to Rewind when deleted.
  Moving a document out of a table, or into another, deletes its values.
  The wire gains `listDocumentFolderProperties`,
  `createDocumentFolderProperty`, `renameDocumentFolderProperty`,
  `setDocumentFolderPropertyType`, `moveDocumentFolderProperty`,
  `deleteDocumentFolderProperty`, `listTableRows`, `getTableRow` and
  `setDocumentValue`, `is_table` on `DocumentFolder`, the
  `documentTable(folder)` invalidation and
  `hello.vocabulary.property_data_types`; rows are created, renamed, deleted
  and moved with the document commands. The iPhone app needn't show tables:
  it lists their rows as plain documents.
- A table's column can be a formula: a JavaScript function of the row, such
  as `(row) => row.Price * row.Quantity`, whose result fills the column in
  every row and is computed again when the row's values or the columns
  change, or, with its update mode set to continuously, every hour as well,
  so a formula that reads the date (days until a deadline) stays current.
  The row holds the values of the columns that aren't formulas, not
  the document's title or text. Its results filter and sort like any value.
  A row the formula fails on stays empty, and the formula shows why.
  Formulas run with nothing but the row, under a time and memory limit. The
  wire gains `setDocumentFolderPropertyFormula`, a `formula` on
  `createDocumentFolderProperty` (a `FormulaDefinition{source, update_mode}`
  or `null`) and on `DocumentFolderProperty` (`{source, update_mode, error}`
  or `null`), `FormulaUpdateMode` (`whenChanged`, `continuously`) and
  `hello.vocabulary.formula_update_modes`, and `setDocumentValue` refuses a
  formula's column. A pass that computes the values already stored doesn't
  invalidate `documentTable`.
- A table's column can be a choice: each value is one of the column's
  options, which have a name, unique ignoring case, and a color from the
  agents' palette or none. Renaming an option keeps every value that holds
  it, and deleting one clears them. A choice filters on an option (is,
  isn't) or emptiness, and sorts by its options' order. Changing a column to
  a choice makes an option of each different value, and back to text keeps
  each option's name. A choice is never a formula, and a formula reads it as
  its option's name. The wire gains `choice` in `PropertyDataType`, the
  `{"type": "choice", "value": <option id>}` `PropertyValue`, `PropertyOption`
  (`id`, `property_id`, `name`, `color`, `sort_order`), `options` on a
  choice's `DocumentFolderProperty` (its type is now the flattened
  `PropertyType`), an `options` list of `NewPropertyOption{name, color?}` on
  `createDocumentFolderProperty` (required, empty for other types), and
  `createPropertyOption`, `renamePropertyOption`, `setPropertyOptionColor`,
  `movePropertyOption` and `deletePropertyOption`, each replying with the
  property. This breaks `cohesionfs`, which sends
  `createDocumentFolderProperty` and reads values.
- A document folder, and so a table, can be linked to as
  `cohesion://document-folder/<ws>/<id>`, and `resolveLink`, `resolveEmbeds`
  and `backlinksOf` answer for it: `EmbedKind` and `LinkKind` gain
  `documentFolder`, and `EmbedItem` gains `documentFolder` (the folder, its
  columns, its first 5 rows and its `row_count`). A row is linked as the
  document it is. `hello.vocabulary.embed_kinds` gains its word, "Folder".
  Clients that switch on an embed's `kind` or `item.kind` need the new case;
  the iPhone app can show it by title, excerpt and kind word until it draws
  it.
- Breaking: databases are no longer an area of their own. When the daemon
  is upgraded, each database becomes a top-level document folder named for
  it, and each row a document in that folder: the row's own document, or a
  new empty one. A row's title is its value in the database's first text
  column set by hand, or its document's title, or "Untitled", and every
  column and value is kept. `listDatabases`, `getDatabase`,
  `createDatabase`, `renameDatabase`, `deleteDatabase`,
  `listDatabaseProperties`, `createDatabaseProperty`,
  `renameDatabaseProperty`, `setDatabasePropertyType`,
  `setDatabasePropertyFormula`, `moveDatabaseProperty`,
  `deleteDatabaseProperty`, `listDatabaseRows`, `getDatabaseRow`,
  `createDatabaseRow`, `deleteDatabaseRow`, `setDatabaseValue` and
  `createDatabaseRowDocument` are gone, as are the `Database`,
  `DatabaseRow` and `DatabaseRowEntry` models (`DatabaseProperty`,
  `DatabaseDataType`, `DatabaseFormula`, `DatabaseValue` and `DatabaseCell`
  are now `DocumentFolderProperty`, `PropertyDataType`, `PropertyFormula`,
  `PropertyValue` and `DocumentCell`), the `databases(ws)` and
  `database(id)` invalidations, the `databaseRow` document type,
  `hello.vocabulary.database_data_types`, and the `database` and
  `databaseRow` link and embed kinds. `cohesion://database/…` and
  `cohesion://database-row/…` links no longer name anything. The web UI
  moves with it (Databases leaves the sidebar and ⌘K); this breaks
  `cohesionfs`, and the CLI and the iPhone app never had databases.
- In the web UI, a folder in Documents becomes a table when you add a
  column to it, from the folder's ⋯ menu or the table's + button. The table
  shows each document's title, which you can rename in place, then a column
  for each value: add a column of any type, rename it, change its type, move
  it or delete it; edit a cell in place; and sort and filter on any column,
  the title included, which Cohesion does on the server. "New document" and
  the New line under the table add a row. Deleting the last column makes the
  folder a plain list again. Tables show in the folder tree with a table
  icon, and All leaves their rows out unless you search for them.
- A row opens over the page, with its values above its text, editable there
  as in the table, and Open goes to the document's own page, which shows the
  values under the title. A new column can be a formula, written in a code
  editor that explains what the row holds and starts from an example using
  the table's own columns, with an update mode, When changed or
  Continuously; a column's menu edits its formula and its update mode, turns
  a column set by hand into a formula, which deletes its values, and back. Drag a
  document onto a table in the folder tree to make it a row, or drag a row
  onto another folder or the Inbox to take it out; when the row has values
  set by hand, Cohesion asks first, since they'll be removed. Move to on a
  document's page asks the same way.
- In the web UI, a column can be a Choice: adding one takes its options,
  each with a color and a name, in the drawer; a cell shows its option as a
  colored pill and picks another from a menu; "Edit options…" on the
  column's menu renames, recolors, reorders, adds and deletes options, and
  asks before deleting one that documents hold. The folder's card shows the
  pills too.
- A link to a folder alone on its line shows as a card: a table with its
  first rows as a small table, a plain folder with its first documents'
  titles. Clicking it opens the folder. Copy link is on the folder's ⋯ menu,
  a row's ⋯ menu and the row's panel.
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
