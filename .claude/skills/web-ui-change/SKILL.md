---
name: web-ui-change
description: Make a change to the cohesion web UI (SvelteKit in web/): where code goes, the checklist for a new feature or screen, which component to use, the Copy, styling and motion rules, verifying a large change in a real browser in mock mode with a few screenshots, and noting parity with the iPhone app. Use for any change under web/src, including new pages, components, stores and UI text.
---

# A web UI change

The web UI is the full surface: every feature exists here first, and the
Mac, iPad and iPhone WebUI harnesses show this same page (see "Surface:
WebUI" in `docs/product-direction.md`). Read `web/README.md` first: it
holds the layout, styling, components, shortcuts, motion and Copy rules,
and `web/src/lib/stores/README.md` holds the store pattern. This skill is
what to do with them.

## Where code goes

- `src/lib/features/<area>/`: a feature's components and helpers
  (agents, tasks, documents, actors, settings…).
- `src/lib/stores/`: rune stores, one per resource, refetching on the
  invalidations they `match`.
- `src/lib/ui/`: primitives. They import nothing from `stores/`, `shell/`
  or `features/`.
- `src/lib/shell/`: sidebar, ⌘K palette, shortcuts (`shortcuts.svelte.ts`,
  `keys.ts`, `ShortcutsDialog.svelte`), route builders (`routes.ts`), the
  linked-item panel.
- `src/lib/protocol/`: the connection, the generated types and the mock.
  Never edit `generated.ts` by hand; change the Rust and run
  `npm run generate` (`protocol-change`).
- `src/routes/`: pages. `/w/[ws]/…` are workspace pages.

Match a nearby feature that does something similar before inventing a
pattern.

## Checklist for a new feature or screen

Go through each line; most are one edit, and each is easy to forget.

- **Either half**: a page can show in either half of a split view, so it
  reads `params`, `url` and `state` and moves with `pane()`
  (`shell/pane.svelte.ts`), names itself with `PageTitle`, and takes the
  stores of what it has open from `pageStores()` (lint refuses `$app/state`
  and `goto`). Check it in a split half as well as alone ("Split view" in
  `web/README.md`).
- **Store**: a `Resource` whose `matches` names the invalidations that
  touch it; mutations update from the reply, never refetch; a component
  that loads what only it shows calls `release()` on unmount
  (`stores/README.md`).
- **Mock**: a handler and seed data in `src/lib/protocol/mock.ts` for every
  request the feature sends, so mock mode and the store tests work.
- **Words from the daemon**: kinds the daemon names (permissions, hooks,
  priorities, sizes, agent settings) come from `vocabulary.svelte.ts`,
  never written on the page.
- **Copy**: every string follows skill `copywriting` and "Copy" in
  `web/README.md`.
- **States**: a page or section that loads goes through `ResourceState`
  (loading, error with retry, not found); an empty list shows
  `EmptyState` ("No {things} yet" with the page's action, or "No
  matches").
- **A page**: a route builder in `shell/routes.ts`; the same name in the
  sidebar, its `PageHeader`, its `<svelte:head><title>`, the ⌘K palette
  (`Palette.svelte`, typed by hand), which is how it is reached from the
  keyboard: pages get no chords.
- **Any other shortcut**: its handler and a row in
  `ShortcutsDialog.svelte`, in the same commit ("Keyboard shortcuts" in
  `web/README.md`). Nothing checks this; the dialog is how people find it.
- **A new link kind** (`cohesion://…`): a panel of its own on
  `ui/ItemPanel.svelte` through `shell/linkedItem.svelte.ts`, and an
  `/open/…` route as the fallback.
- **Icon-only controls** carry an `aria-label`; no `title` anywhere.
- **Hover-only actions** (a row's ⋯ that fades in) also show with
  `touch:opacity-100`, or a touch screen never sees them.
- **Motion**: Svelte transitions take `motion.duration` (or `slideIn` and
  `slideOut`), never a literal, so reduced motion zeroes them.
- **Titlebar inset**: anything pinned to the top of the window keeps clear
  of `--titlebar-inset`, where the Mac WebUI harness drags the window.
- **Tests**: a new store or helper gets a vitest test beside it
  (`*.test.ts`, against the mock); components have none.
- **Parity**: say in the report whether the iPhone app (`apple`) needs the same change
  (see `finish-task`).

## Copy

Pick the wording yourself, taking the plainest option the rules allow and
reusing words the UI already uses for the same thing (`grep -r` the term).
List each new or changed string under "Choices made" in the report, so
the reviewer can reword it.

## Styling

- Tailwind with the tokens in `src/lib/tokens.css`; see "Styling" and
  "Look and feel" in `web/README.md`. A new colour, shadow or size is a
  token, not an arbitrary value.
- Pick components by "Components" in `web/README.md` (Menu, Picker,
  Select, Dialog, SidePanel).
- Agent avatars: only `ui/AgentAvatar.svelte` draws one, by the shares in
  "Avatars" in `web/README.md`.

## Verify it in a browser

There are no component or browser tests, by choice; how far you verify
in a browser depends on the size of the change. Every image you take
stays in the session's context for the rest of the run, so screenshots
are the expensive part: take them only where this section asks.

**A small change** (copy, spacing, a colour, one control, a bug fix in
one component): no browser check and no screenshots. The checks below
and a read of the diff are enough; if behaviour needs exercising, read
the page as text (the DOM or accessibility tree), not as images. Under
"Check by hand" in the report, say what the reviewer should look at.

**A large change** (a new page, screen, panel or dialog, or a redesign of
an existing layout) gets the browser check, with **at most 6 screenshots
in the whole task**, your own check and the reviewer's together:

1. Start mock mode: `cd web && VITE_MOCK=1 npm run dev` (or the `web-mock`
   configuration in `.claude/launch.json`), at http://localhost:5173. Add
   mock data when the feature needs some to show. Run it in the
   background and stop it when you're done.
2. Open the page in a browser tool and exercise the change: the happy
   path, the empty, loading and error states, the keyboard path, a narrow
   window (below 900px and 600px) and dark mode where they apply. Check
   behaviour by reading the page as text; take a screenshot only to judge
   the look.
3. Judge the look against `web/README.md` (Copy, "Look and feel",
   Motion) and fix what's wrong.
4. End with the screenshots the reviewer looks at: the finished result, one per
   state worth seeing, within the limit. A screenshot a browser tool
   returns, or a PNG you `Read`, is attached to the run (the actor session
   message it appears under) and shows in the task's transcript, which is
   how the reviewer sees it; make sure each one you mean the reviewer to see is
   shown that way, not only saved to disk. Under "Check by hand" in the
   report, list what the screenshots show and what's left to look at:
   the reviewer still checks final polish.

Without a browser tool, say so under Caveats: there are no screenshots,
and the look is left to the reviewer's check.

## Checks

`cd web && npm run check && npm run lint && npm test && npm run build`.
