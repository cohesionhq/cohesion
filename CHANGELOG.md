# Changelog

What changes in each release of Cohesion, newest first. A release is
`vXXXYYYYY`, the version `cohesion status` shows and the container image's tag
(see "Publish an image" in [CONTRIBUTING.md](CONTRIBUTING.md)). Add an entry
under "Unreleased" with any change users would notice.

## Unreleased

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
