---
name: web-ui-change
description: Make a change to the cohesion web UI (SvelteKit in web/): working in mock mode, the checklist for a new feature or screen, choosing copy, verifying a large change in a real browser with a few screenshots, and noting parity with the iPhone app. Use for any change under web/src, including new pages, components, stores and UI text.
---

# A web UI change

Read `web/README.md` first (layout, split view, styling, components,
shortcuts, motion) and `web/src/lib/stores/README.md` for stores.
Match a nearby feature that does something similar before inventing a
pattern.

## Work in mock mode

Run the UI against the mock daemon, not a real one:

```sh
cd web && VITE_MOCK=1 npm run dev     # http://localhost:5173
```

(or `web-mock` in `.claude/launch.json`). It needs no daemon or CLI, so
nothing you do there touches anyone's data. Run it in the background and
stop it when you're done. Anything the feature sends needs a handler
(`src/lib/protocol/mock/handlers/`, in its area's file) and seed data
(`mock/seed.ts`), or mock mode and the store tests break. Don't run `scripts/web-dev.sh` yourself: it runs against the
developer's own `dev/` state and blocks until killed, so hand it to the
person in your report.

## Checklist for a new feature or screen

Each item is one edit that's easy to forget; the rule behind it is in
`web/README.md` unless noted.

- **Either half**: the page works in a split half as well as alone
  ("Split view"). Check it in both.
- **Store**: `matches` names its invalidations, mutations update from the
  reply, and page-only loads `release()` on unmount (`stores/README.md`).
- **Mock**: a handler and seed data for every request the feature sends.
- **Words from the daemon** come from `vocabulary.svelte.ts` ("Layout").
- **States**: `ResourceState` for loading, error and not found;
  `EmptyState` for an empty list ("Components").
- **A page**: a route builder in `shell/routes.ts`, the same name in the
  sidebar, `PageHeader`, `<title>` and the ⌘K palette ("Keyboard
  shortcuts").
- **Any other shortcut**: its handler and its row in
  `ShortcutsDialog.svelte`, in the same commit.
- **A new link kind** (`cohesion://…`): an entry in `ITEM_KINDS`
  (`lib/format/itemLinks.ts`), which the grammar, the `/open/…` matchers
  and the stores read; its icon in `shell/embedIcons.ts`; and a panel on
  `ui/ItemPanel.svelte`.
- **Look and feel**: `aria-label` on icon-only controls, hover-only
  actions shown on touch, `motion.duration` on transitions, nothing under
  `--titlebar-inset`, tokens rather than new values ("Look and feel",
  "Styling").
- **Tests**: a vitest test beside a new store or helper.
- **Parity**: say in the report whether the iPhone app (`apple`) needs the
  same change.

## Copy

Pick the wording yourself by skill `copywriting`, taking the plainest option and reusing words the UI
already uses for the same thing (`grep -r` the term). Mention each new or
changed string in the report, so the reviewer can reword it.

## Verify it in a browser

Every image you take stays in context for the rest of the run, so
screenshots are the expensive part.

**A small change** (copy, spacing, a colour, one control, a bug fix in
one component): no browser and no screenshots. The checks and a read of
the diff are enough; if behaviour needs exercising, read the page as text
in mock mode (the DOM or accessibility tree). In the report, say what the
reviewer should look at.

**A large change** (a new page, screen, panel or dialog, or a redesigned
layout): at most 6 screenshots in the whole task.

1. Open the page in mock mode, adding mock data the feature needs to
   show.
2. Exercise the change by reading the page as text: the happy path; the
   empty, loading and error states; the keyboard path; below 900px and
   600px; dark mode.
3. Screenshot only to judge the look against `web/README.md`, and fix
   what's wrong.
4. End with the screenshots the reviewer should see, one per state worth
   seeing. Make sure each is attached to the run (returned by the browser
   tool or `Read` from a PNG), not only saved to disk. In the report, say
   what they show and what's left to look at.

Without a browser tool, say so in the report: no screenshots, and the
look is left to the reviewer.
