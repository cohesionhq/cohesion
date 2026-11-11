# Cohesion web UI

A SvelteKit single-page app (Svelte 5, TypeScript) built to static files in
`build/`. `cohesion open` serves that directory and relays the daemon's JSON
protocol at the same origin (`GET /connect` WebSocket, `GET /client`). The
CLI opens a one-time launch link, which redirects to the app with the token
in the URL fragment; the link works once, for 30 seconds, so the token never
appears on a command line. The CLI saves the token and
serves on port 7343 by default, so the address and a browser's stored token
survive restarts; `cohesion open --reset-token` replaces it.

```sh
npm ci
```

## Dev loops

**A. Live reload against a real daemon.** From the repository root,
`scripts/web-dev.sh` makes a random token, starts the CLI proxy in dev mode on
port 4000 and Vite, which proxies the protocol paths to it
(http://localhost:5173):

```sh
scripts/web-dev.sh
```

The proxy keeps its token in `--dev`: it reads `COHESION_DEV_TOKEN` from the
environment, and Vite's proxy (`vite.config.ts`) adds the same value to what
it forwards (an `Authorization` header, and `?token=` on the WebSocket), so
the browser never holds it. To run the two by hand, export one token for
both:

```sh
export COHESION_DEV_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
cohesion open --dev --port 4000 --no-browser
npm run dev
```

`.env.development` sets `VITE_DEV_PROXY=1` so the "open with
`cohesion open`" notice stays hidden. To run the proxy on another port, set
`VITE_PROXY_PORT` to match (in the environment or `.env.local`):

```sh
cohesion open --dev --port 4100 --no-browser
VITE_PROXY_PORT=4100 npm run dev
```

**B. The built app, as users get it.**

```sh
npm run build          # writes build/
cohesion open
```

## Mock mode

```sh
VITE_MOCK=1 npm run dev
```

Swaps the transport for `src/lib/protocol/mock.ts`, an in-memory daemon with
seed data modelled on a real daemon's, with people's names made up (three workspaces, journals,
tasks and tags, documents and folders, two of them tables, contacts, feeds and their articles, agents and their threads, actors,
settings).
No daemon or CLI needed. The tests use the same mock. It pushes the
invalidations of tables (`documentTable`, and `documentFolders` as a folder
becomes a table or stops being one) a moment after the write, as the daemon
does, and no others. It has no JavaScript
engine, so a table's formula runs only if it is written in the small part of
the language `src/lib/protocol/mockFormula.ts` reads (arithmetic, comparisons,
`??`, `?:` and a few methods over `row`); the formula editor's examples are.

## Checks

```sh
npm run check          # svelte-check (types)
npm run lint           # prettier --check + eslint
npm test               # vitest
npm run build
npm run format         # prettier --write
npm run generate       # rewrite generated.ts and vocabulary.json in src/lib/protocol/ from cohesion-core
```

## Verifying a change

Verification is manual: there are no component or browser tests, by
choice; helpers and stores get unit tests. A small change (copy, spacing,
one control, a fix in one component) is verified by the checks and a read
of the diff, and a reviewer looks at it by hand. A large one (a new page,
screen, panel or dialog, or a redesigned layout) is also checked in a real
browser against mock mode, the states and widths that apply, dark mode,
and the rules below, with at most six screenshots in the whole task; the
last of them go in the pull request for the reviewer. The steps
are in the `web-ui-change` skill.

## Layout

- `src/lib/protocol/`: wire types, the WebSocket `Connection` that carries
  every request (`sendRaw` for the API console), the session token, and the
  mock. `api.ts` picks the transport. `generated.ts` holds every
  model, request and reply, and the `Requests`/`Replies` maps `send` is
  typed by; `npm run generate` writes it from cohesion-core (it needs
  cargo); commit it with the Rust change. Never edit it by hand: change the
  Rust types and regenerate. `types.ts` re-exports it and adds the
  client-only frames and helpers. What the daemon calls each permission,
  hook, priority and agent setting comes in `hello` and lives in
  `vocabulary.svelte.ts`; never word a kind on the page. `npm run generate`
  also writes the same words to `vocabulary.json`, which only mock mode and
  tests read.
- `src/lib/features/<area>/`: a feature's components and helpers (agents,
  tasks, documents, actors, settings…).
- `src/lib/stores/`: rune stores; see its README for the pattern.
- `src/lib/ui/`: primitives, which import nothing from `stores/`, `shell/`
  or `features/`, but for `Markdown.svelte`, which opens links and draws
  cards from `shell/`, and `ItemPanel.svelte`, which shows what links to its
  item with `shell/Backlinks.svelte`. Only `Editor.svelte` imports CodeMirror; it draws a
  link alone on its line as the card its caller hands it (`embed`) and looks
  items up after `@` or `[[` with its caller's search (`mentions`). Only
  `Markdown.svelte` renders HTML (with `embeds`, a link alone on its line is
  a card, split by `textPieces` as the daemon splits it; with `diagrams`, which agent replies, plans and documents turn on, a `mermaid` fenced block
  becomes an SVG through `diagram.ts`, which asks the daemon (`renderMermaid`) once its fence has closed, once per source and width, and keeps the reply;
  a block that won't draw stays code, as it does until the reply comes, and a drawn one opens in
  `ImageViewer.svelte` over the page, drawn again for the larger canvas and enlarged to fill it, from a click or its expand icon. The SVG is inlined so the `.diagram` tokens reach it, and a theme change or a resize needs no request); `colors.ts` is the one place an agent
  colour becomes a class.
- `src/lib/tokens.css`: the design tokens and the Tailwind v4 theme built
  from them. See [Styling](#styling).
- `src/lib/shell/`: the sidebar, the ⌘K palette, the workspace switcher,
  route builders and keyboard shortcuts (⌘ or ⌥ and a letter for a page, or a
  digit for a workspace, ⌘, for Settings, and ⌘[ ⌘] or ⌥← ⌥→ and a mouse's
  side buttons for back and forward, which the WebUI harnesses' web view lacks; ⌘/ or
  `?` lists them), and the split view (see [Split view](#split-view)).
- `src/routes/`: `/` picks a workspace; `/w/[ws]/…` are workspace pages
  (`ws` is the numeric id), and a workspace opens on its Dashboard
  (`routes.workspace`, the one place that says so; Today stays in the
  sidebar and ⌘K); `/agents`, `/actors` (and `/actors/[session]`, one
  actor session), `/threads/[thread]` (a read-only transcript), `/settings` and
  `/open/…` (where a `cohesion://task/3/42` link lands, or a document's, a
  resource's, an agent's, an actor session's or an article's; see "The web
  UI" in `docs/architecture.md`) are global.

## Split view

The sheet can hold two pages side by side, the sidebar and the actor sessions
column staying as they are. A page header's Split view button (top right)
opens a blank second half; ⌥-clicking a link, or ⌥↩ on a ⌘K result,
opens it there; each half's close button leaves the other filling the
sheet, and at a phone's width the split closes to the half last used. The
pieces, in `src/lib/shell/`:

- `pane.svelte.ts`: a `Pane` is one half. The main half is SvelteKit's page;
  the second (`SplitPane`) routes on its own (`splitRoutes.ts` finds a
  route, its layouts and its load as SvelteKit would) and keeps its own
  history, so ⌘[ ⌘] and the back link step the half in use. A window keeps
  its split across a reload (`sessionStorage`).
- `PaneHost.svelte` draws a half: its scroller, the header's controls
  (through `ui/paneChrome.ts`, so `PageHeader` and `ChatHeader` stay
  primitives), and, in the second half, links that open there.
- **A page reads where it is from `pane()`** (`params`, `url`, `state`,
  `goto`, `beforeLeave`), never from `$app/state` or `$app/navigation`, and
  names itself with `PageTitle`, which titles the window only from the main
  half. Lint refuses the router's own imports outside the shell.
- **A page's stores come from `pageStores()`**: each half has its own set
  of what a page has open (see `src/lib/stores/README.md`), so two task
  folders or two conversations side by side don't share one.
- Split, each half is a `pane` container, and `max-md:` and `max-sm:` also
  hold below 640px and 480px of half (`tokens.css`), so a page in a half lays
  out as it would in a narrower window.

## Going back

A way back is always `ui/BackLink.svelte`: the chevron alone before the title
in a page's or a conversation's header, or "‹ {label}" in a header with no title
(`PageHeader`'s `back`). A plain click steps the half it is in back
through history to the page you came from, named by that page's title; the
`href` and `label` a page gives are only for when nothing in history leads back
(a deep link, a reload, the first page of a second half), so they name the page
it sits under. `shell/back.ts` records where each entry came from, and
`SplitPane` does the same for its own list. `pane().goto(href, { replaceState:
true })` takes the entry's place and keeps its way back, so back skips the page
replaced; use it when the page is gone (deleted, not found) or only redirects,
and a push for anything else.

## Look and feel

The page looks professional and calm: neutral greys and one hue. Indigo
(`accent`) marks the action that settles a dialog or a form, focus and what is unread;
amber (`warn`) means something needs the user; agent colours are the only
other hues. Dark mode follows the system (`prefers-color-scheme`), with no
switch of its own. Words for what the daemon names (permissions, hooks,
priorities, sizes, settings) come from `hello`'s vocabulary, never from the
page. Nothing explains itself on hover: no tooltips, the label says it. A
native `title` gets in the way of a resting cursor, so nothing sets one (lint
refuses it). An icon-only control names itself with `aria-label`, and what
matters is on the page, not behind a hover: an action that fades in on
hover (a row's ⋯) also shows with `touch:opacity-100`.
The details are in [Styling](#styling), [Components](#components) and
[Copy](#copy).

## Styling

Components are styled with Tailwind utility classes. The theme in
`src/lib/tokens.css` replaces Tailwind's defaults with the app's tokens, so
only these exist:

- **Colours** switch with the colour scheme on their own, so no `dark:`
  classes are needed: `fg`, `fg-muted`, `fg-subtle`, `canvas`, `bg`,
  `bg-raised`, `bg-hover`, `current-item`, `border`, `border-strong`,
  `primary` (+ `-hover`, `-fg`), `accent` (+ `-hover`, `-fg`, `-soft`),
  `danger`, `ok`, `warn`, `selection`, `overlay`, `n0`–`n9`. `canvas` is
  the window the sidebar sits on; `bg` is the page's sheet. The chrome is
  neutral: the current item in a list of places (the sidebar, a page's
  folders, Settings' sections) takes `current-item`, a grey tint, never a hue.
  `accent` is the only hue, kept for the filled button that settles a
  dialog, a form or a card (`primary`), focus, what is unread,
  links, a chosen filter pill (`features/tags/FilterPill.svelte`: a check when
  it keeps what matches, an X and a strike-through when it hides that) and a
  medium-priority task's flag; a high-priority task's flag takes
  `agent-orange`, a deliberate exception, so it reads a step below urgent's red
  without being mistaken for amber; in an agent's conversation `primary` is the agent's color, so its
  main actions take it, and so does the Dashboard composer's for the agent it
  addresses. Status reads the same everywhere: amber (`warn`) for what needs the
  user, the same hue lifted in the dark; work going on is grey, never the accent,
  drawn by `ui/StatusIcon.svelte`. For example
  `text-fg-subtle`, `bg-bg-hover`, `border-border`, `bg-danger/10`.
- **Spacing** is a 4px step: `p-1` is 4px, `gap-2` 8px, `px-3` 12px.
- **Type**: `text-3xs` 10px (a dial's percentage only), `text-2xs` 11px, `text-xs` 13, `text-sm` 14 (body), `text-md`
  15, `text-lg` 20, `text-xl` 28. Every page's header is one bar
  (`ui/PageHeader.svelte`, as tall as a conversation's `ChatHeader`), running to the
  sheet's edges over a hairline, its title `text-md` semibold and its meta beside it
  `text-xs`, its tools at the sheet's right edge; a page that reads in a column (a
  document, an article, the journal) puts the column under the bar, and a document's or
  an article's own title, `text-xl`, at the column's head. A section's title is
  `text-md` semibold; a group label (Agents, Today) is `text-xs` medium in
  `fg-subtle`, in sentence case. `font-sans` is the UI; `font-serif` sets
  an article for reading; `font-mono` is code.
- **Radii**: `rounded-sm` 6px (controls, rows), `rounded-md` 10px (cards,
  messages), `rounded-lg` 24px (the composer's capsule, half a one-line
  field's height). **Focus ring**:
  `shadow-focus`.
- **Layout and widths**: the page is a sheet inset `--sheet-inset` (8px)
  from the window, with `--sheet-radius` corners, on the canvas beside the
  sidebar; on a phone it meets the screen's edges. From 1100px, on an
  agent's conversation or an actor session, a second column sits on the canvas
  right of the sheet (`shell/ConversationGutter.svelte`) and takes what the
  conversation's header would otherwise carry, in a summary row as tall as that header over flat groups, all ruled by
  hairlines and inset a little from its edges (`ui/GutterCard.svelte`, `ui/GutterGroup.svelte`): for an agent its context size and what it is doing, its
  agenda (the step it is on marked, finished ones struck through), its
  actor sessions not yet resolved, each with an eye-off icon to stop following it and the sidebar's archive
  icon to resolve it on hover or focus (always on touch), and its cues (`AgentGutter`), for a session where
  its actor, where it works and its model, then its usage as bars (`SessionGutter`); where it
  stands and Resolve stay in the header at every width. It speaks
  for the main half only; a second half's header keeps all of it, as does every
  header below 1100px. Pages fill the sheet,
  with no max-width column, so every header's tools end at the sheet's right edge. Each page's outer element takes
  `px-(--page-inset)` (28px, 20 below 1100px, 16 below 900). Lists sit flat
  on the sheet, ruled by hairlines, with no box of their own. Two
  exceptions read in a centred column, its title 24px under the bar: an article, at a book's line length
  (`max-w-240`); a document, a journal entry and Today, at prose's (`max-w-170`, about 75
  characters); and a conversation and the Dashboard (`max-w-190`), whose messages take at
  most 70% of it and whose question cards take all of it. A list's sections are headed by
  `list.heading` (a semibold label, its count, a hairline); a folder column's groups by a
  quiet `text-xs` label, its first group unlabelled. A conversation's header is `ui/ChatHeader.svelte`,
  a solid bar across the sheet, so nothing scrolled under it shows through, with its own even insets. `--gutter` is
  what overlays keep clear of the viewport's edges. `w-(--sidebar-width)` and
  similar for the other layout variables.
- **WebUI harnesses**: the Mac, iPad and Linux WebUI harnesses show the page edge to edge, under the
  Mac's window controls, the iPad's status bar or the Linux app's header bar, and set
  `--titlebar-inset` on `<html>` to how much of the top they cover (28px
  on a Mac, the safe area on an iPad, the header bar's height on Linux; 0 in a browser). The sidebar pads its
  top by it and the main pane sits below it. Keep anything clickable out of
  that strip: the Mac WebUI harness lays a window-drag area over it, and the
  Linux one its header bar, with the main menu and window controls at its end.
- **Avatars**: an agent's emoji or initial on a disc of its color, drawn
  only by `ui/AgentAvatar.svelte`. The glyph's font size is a fixed share
  of the disc's diameter: 0.6 for an emoji, 0.5 for an initial (semibold),
  with `leading-none` so it sits centred, and the disc clips it
  (`overflow-hidden`, `whitespace-nowrap`) so a glyph that draws wider
  than its font size never spills past the circle. Every client draws
  avatars by the same shares, so the web and the apps match. The
  exceptions are the sidebar's 20px disc (`xs`) and the message toast
  (`tight`) and a conversation's header (`md`, also `tight`), whose emoji takes 0.45: at 0.6 it crowds them, and the iOS
  chat's working indicator, which takes 0.45 too. The discs are
  20px (`xs`, the sidebar), 28 (`sm`, list rows), 32 (`md`, a
  conversation's header) and 44 (`lg`, cards).
  A new avatar size adds a disc diameter; it never picks its own font
  size. A person (a contact, or the user) is drawn by `ui/Photo.svelte`:
  their photo on a disc of the same diameters (20px beside a label, 28 in
  a list row, 96 on its own), or without one their name's initials at the
  initial's 0.5, or a person glyph when they have no name either.
  `ui/Avatar.svelte` draws whichever of the two a menu or picker item, or a
  task's assignee, stands for.
- **Motion**: CSS state changes (colour, hover, overlays, page fades) run
  on the `--duration` token, 120ms; CSS transitions take it by default.
  Svelte enter and leave transitions and `animate:` take `motion.duration`
  from `ui/motion.ts`, 200ms, longer because they move or resize content.
  A column sliding in beside the page takes `motion.slideIn` and
  `motion.slideOut` instead, 180ms in and 140ms out, as the side panel
  slides. Reduced motion zeroes them all. A page navigation cross-fades
  (`shell/fade.ts`): the old page and the new dissolve over the same 120ms
  on the same curve, so what they share holds still, and nothing moves or
  resizes. The column beside a conversation doesn't slide in a navigation:
  it opens or shuts at once and fades with the page, and between two
  conversations it stays open while its contents fade. Data that lands
  after the fade has stopped waiting fades in rather than popping in. A short changing value (the
  agent header's token count, a fork's step count) is drawn with
  `ui/RollingText.svelte`, which rolls only the characters that change.
  Text in progress shimmers, drawn with `ui/Shimmer.svelte`: an agent's name
  while it works, the line at a conversation's foot and an actor's running tool
  call. A band that dims the text's own color crosses it, or under reduced
  motion it stays still and dimmed. Grey, not the accent: work in progress needs
  no attention. The line at the foot (`ui/WorkingLine.svelte`) is the one place a
  screen says in words what an agent is doing: the daemon's `activity` trailing
  off ("Thinking…"), or "Working…" without one and for an actor session. New words
  fade in over the old on its one line, so nothing moves. While an agent works its
  header is its name alone; its role comes back when it stops. The icon rail and
  the top bar, which hide names, ring the agent's avatar in grey instead. An actor session at work is a
  grey disc with a turning ring, one of `ui/StatusIcon.svelte`'s set of discs. What repeats in step goes on the
  page clock in `ui/motion.ts`, one timer beating every 2.5s: `shimmerOnPageClock`
  starts a 0.9s crossing on every beat and `blinkOnPageClock` the following eye's
  double blink on every second one, so nothing keeps its own timing.
- **Breakpoints**: `max-sm:` below 600px (phone), `max-md:` below 900px
  (icon rail), `max-lg:` below 1100px; in half of a split view `max-sm:`
  and `max-md:` also hold below 480px and 640px of half. `touch:` targets
  devices without hover.

Tailwind's preflight reset is not loaded; the base rules in `tokens.css` are
the reset, in `@layer base`, so any utility overrides them. Form controls
inherit the surrounding font there, so they need no `font-[inherit]`.
An arbitrary value (`shadow-[…]`, `bg-(--…)`) only combines tokens, as in
`shadow-[inset_3px_0_0_var(--warn)]`; a new colour, shadow or radius is a
token in `tokens.css` first. The one exception is a glyph's own geometry,
a few pixels drawn as a mark (the sidebar's unread dot, a keycap's
corners).

Prettier sorts class lists. Write class names in full (`text-danger`, never
`text-${tone}`) and use Svelte's `class={[…]}` for conditional ones. Keep a
`<style>` block only for markup you don't write yourself, such as rendered
Markdown or CodeMirror's DOM.

## Components

Each kind of overlay or choice has one component; pick by what the user is
doing, not by how it looks.

- **Making something new** from a page (New document, New contact, Add
  account) is a `secondary` `Button` (outlined) led by a plus in the page header's actions.
  A page's buttons come in three kinds and no others: `primary`, filled in the accent,
  settles a dialog, a form or a card, and turns grey while it can't (only an action that can
  run carries the accent); `secondary`, outlined, is a page's own action and
  its header's menus (the order, Filters, Resolve), through `secondaryClass` where it isn't a
  `Button`; a toolbar's icon-only buttons are square and quiet. Resolve goes quiet while its
  session waits on the user, so what the session asks is the screen's action. A composer's
  Stop is a light grey disc, its Send the screen's `primary`.

- **Menu** (`ui/Menu.svelte`, `RowMenu` for a row's ⋯) is for actions: each
  item does something, and may name a bare key that picks it.
- **Picker** (`ui/Picker.svelte`) chooses from a list long enough to type
  into: a folder, tags, an agent. It filters as you type and can create.
- **Select** (`ui/Select.svelte`) is a form field over a short, fixed list:
  a setting's value, a provider. `Choices` is the same choice laid out in a
  panel.
- **Dialog** (`ui/Dialog.svelte`) is for a decision or a short form that
  blocks until answered; `ConfirmDialog` for a destructive action. Every
  dialog is centred in the window, at least `--dialog-inset` from its top
  (below the titlebar inset) and bottom; a taller form scrolls inside. A
  form opens on the fields it needs and folds the ones nobody has to fill
  under More options (`ui/Disclosure.svelte`), which starts closed even
  when one of them is set, and opens when one of them has an error.
- **Rewind** (`features/rewind/RewindButton.svelte`) is the one symbol and
  panel for an item's version history; an item with a history puts this button
  among its actions rather than drawing its own. The workspace's Rewind page,
  of what was deleted, has no sidebar row: it opens from the "…" menu beside
  the workspace picker, or ⌘K. Both draw their rows on one timeline
  (`features/rewind/Timeline.svelte`), grouped by day.
- **SidePanel** (`ui/SidePanel.svelte`) is for reading or writing
  something longer without leaving the page; `ItemPanel` on top of it opens
  every linked item (`cohesion://…`) but a folder, which opens the Documents page at it. An Expand button beside Close floats it
  wider and nearly full height at the middle of the window (Minimize returns it),
  with an item's details in a column beside its text, each scrolling on its own;
  it is gone below 900px and forgotten when the panel closes.
- **Popover** holds a small panel beside its trigger (the emoji picker, a
  usage breakdown);
  **HoverCard** only adds detail that is also reachable without hovering.
- **EmptyState** and **ResourceState** draw every empty, loading, error
  and not-found state, worded by "Empty states" in skill `copywriting`:
  centered, with its `message` as the title in `fg-muted` and an optional
  `detail` as the subtitle in `fg-subtle`. A `Picker` takes the same pair
  (`empty`, `emptyDetail`).

## Keyboard shortcuts

Every shortcut the page handles has a row in the ⌘/ dialog
(`shell/ShortcutsDialog.svelte`); that dialog is how people find them.

- **Pages have no chords.** Every page, workspace and settings section is
  reached from the ⌘K palette (`shell/Palette.svelte`), so a new page gets
  a palette entry, not a shortcut.
- **A shortcut** is two edits in the same commit: the handler where it
  works, and its row, by hand, in `ShortcutsDialog.svelte` under the group
  named for where it works. Nothing checks that they agree.

- **How keys are written**: on a Mac the symbols run together (⌘K, ⌘↩, ⇧↩,
  ⌥↑, ⌫); elsewhere words joined by `+` (Ctrl+K, Ctrl+Enter, Shift+Enter,
  Alt+↑, Backspace). A letter alone is lowercase, as typed; in a chord it is
  a capital, as on the keycap. Write them with `keyLabel`
  (`src/lib/shell/keys.ts`).
- **Modifiers**: the app runs in the WebUI harnesses' web view, so ⌘ (Ctrl
  elsewhere) is the base of every app-wide chord (⌘K search, ⌘, Settings, ⌘/
  shortcuts, ⌘↩ submit). ⌘N makes what the page lists (a task, or a document,
  which in a table is a row) and ⇧⌘N a folder, as a native app's File › New does; they are the
  one exception to the rule that follows, and are handled on the page that
  makes the item. A chord the system or an app already owns (⌘T, O,
  W, Q, H, M, R, P, S, C, V, X, A, Z) is left alone, and a field's own chords
  take ⌥ (Alt), as the new-task row's do.
- **Bare keys** work only where focus is not in a field. A menu item names
  its bare key (`key` on a `MenuItem` or `PickerItem`: digits for priority and
  size, a letter for the rest) and shows it in `<kbd>` at its trailing edge.
  It picks the item while the menu is open, and a picker with a filter field
  takes it only while the filter is empty.

## Copy

What the UI says, and the word for each concept, follows skill
`copywriting` (`.claude/skills/copywriting/SKILL.md`).
