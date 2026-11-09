---
name: copywriting
description: Cohesion's voice and words for anything a user reads, including web UI strings, buttons, dialogs, empty states, errors, toasts, CLI messages, user-facing docs, the README, landing pages, emails and posts. Use when writing any of these, and when asked to review, audit or fix copy. Doesn't cover commit messages.
---

# Copywriting

Write for end users, in plain language modeled on Apple's product copy. Say what happens, in words the reader already knows.

**Writing:** search for how Cohesion already names the thing (`grep -r` the term in `web/src` and `docs/`), apply these rules, then run the checklist at the end.

**Reviewing:** read the file, diff or surface you were given. Report each violation with its location, the current string, the rule it breaks and a rewrite. Change the strings only if asked.

## Cohesion's words

- **One word per concept**, the one the models and docs use: workspace, journal (an entry), document, task, folder (never "task list"), inbox, an agent's extended context and native context (the two stretches of its thread, capitalized as names in the chat's rules), agent, team, cue, fork, choice, extension, permission, hook, registry, connector, account, resource, endpoint, model, default, client. A page has one name in the sidebar, its header, its `<title>`, the ⌘K palette and the shortcuts dialog.
- **Words for what the daemon names** (permissions, hooks, priorities, sizes, agent settings) come from its vocabulary (`vocabulary.svelte.ts` on the web), never written on the page.
- **No internals in the web UI** outside Settings › Developer and About: no "daemon", `cohesiond`, protocol versions, raw enum or hook names, `null` or database ids (Copy ID on a task is the one deliberate exception). The server is "Cohesion": "Can’t reach Cohesion. Retrying…". The server's own messages show as they come.
- **Removal verbs**: Delete is for what you made, which leaves Cohesion. Remove takes out something that lives elsewhere (an imported resource, an account, a cue, a feed) or a detail on an item (a photo, an email address). Extensions are uninstalled and clients revoked.

## Voice

Name the thing, say what will happen, then say what the person can do about it. Clarity beats compression. Every sentence should make sense without the screen around it.

| Don't write | Write |
|---|---|
| It leaves this workspace, and a refresh won’t bring it back. Import it again to restore it. | This resource will be removed from the workspace. To get it back, import it again. |
| Its agents stay, on no team, and its memories are forgotten. | The team’s agents won’t be deleted, but its memories will. |
| Its tasks stay as they are. | Tasks in this folder won’t be deleted. |
| Its journal, documents, tasks and resources go with it. | All journal entries, documents, tasks and resources in this workspace will also be deleted. |
| It stops running and loses the access you gave it. | This extension will stop running and lose the access you gave it. |

- **Name the subject.** Don't open with a bare "It" or "Its". Say which thing you mean.
- **Say what the person gets or what will happen**, not how the data moves. Use the future tense for consequences.
- **No tacked-on clauses.** If the part after a comma is an afterthought or could stand alone, make it a sentence or cut it. "Each agent’s own, which only it sees." and "Its agents stay, on no team" both fail. "To get it back, import it again." is fine.
- **Don't describe what the screen already shows.** A clause that repeats what the data makes obvious gets cut, and sort order never appears in copy: "Every model call and what it cost, newest first." loses "newest first".
- **Cut any word that doesn't change the meaning.** Labels take no possessives: "Tasks" and "Name", not "My tasks" or "Your name". "Your" is fine in a sentence: "Cohesion keeps your data on this computer."
- **Address the reader as "you".**
- **Use only words the user knows from Cohesion.** No implementation terms, even accurate ones: "Enter a provider identifier" and "Backend" both fail on an end-user screen. Settings › Developer, the CLI and developer docs may use a technical term when it's the most accurate word.

## Never

- Exclamation marks
- "Please" or "successfully"
- Filler such as "simply", "just", "easily", "seamless" or "powerful"
- "Oops" or other personality in errors
- "We", "our" or "my"
- The rhetorical "not X, but Y" ("It’s not a task list, it’s a system."). A plain contrast of two facts is fine: "The team’s agents won’t be deleted, but its memories will."

## Capitalization

Sentence case everywhere in this repo: the web UI (including inside the WebUI harnesses), accessible labels, the CLI, docs and marketing headlines. Only proper names take capitals: Cohesion, Markdown, provider and API names. A page named in a sentence is lowercase: "Back to installed extensions". (The native iPhone app, in its own repo, uses title case for buttons, menus and titles.)

## Punctuation

- **A period ends every sentence**, however short: toasts, descriptions, errors and CLI messages. "Copied the task ID."
- **No period on labels:** headings, buttons, form labels, menu items, tabs, column headers, placeholders, CLI table headers and `Config  value` rows.
- **Contractions always:** can’t, couldn’t, isn’t, never "could not" or "is not".
- **Numerals for counts:** "3 tasks". Write "and", not "&".
- **No semicolons.** Use two sentences.
- **Curly quotes and apostrophes** (’ “ ”) in the web UI and marketing. Straight quotes are fine in a terminal and in Markdown source.
- **Em dashes** appear only in docs and marketing prose, at most about one per paragraph, closed up: "word—word". Never in UI strings or CLI messages.
- **An ellipsis** is one character (…).

## String patterns

**Buttons** say what they do: a verb, plus its object when it isn't obvious. "Add account", "Refresh catalog". Never "OK", "Yes" or "Submit". Cancel dismisses.

**Ellipses** end a menu item or text button that opens a dialog or picker before it acts ("Delete…", "New folder…"), and a busy label ("Saving…"). Never on a link, the button that confirms, or an accessible label.

**Confirmations** ask a question naming the thing, and the confirm button repeats the verb. Titles go in curly quotes (tasks, documents, folders, workspaces, resources, cues); names don't (agents, extensions, clients, endpoints, accounts): "Delete “Groceries”?", "Delete Ada?". The description says what else will happen. When nothing else goes with it, it says "This can’t be undone."

**Empty states** are a centered pair stacked vertically (`EmptyState` on the web): a title in the secondary text style with no period, and a subtitle in the tertiary style saying what to do. "No tags" over "Create tags in the filter menu." The subtitle is optional when the action is a button or the page's next step is obvious. Use "No {things} yet" with one action labeled like the page's own button, "No matches" for a search or filter, and "{Thing} not found" with "Back to {page}" for a dead link. In the CLI, the same pair is two lines: "No tags." then "Create one with `cohesion tag create <name>`."

An empty message inline in a list, with no pair, is a sentence and takes a period.

**Errors** say what didn't happen and, when it helps, what to do: "Couldn’t copy to the clipboard.", "Sign-in failed. Sign in again to keep importing."

**Toasts and CLI confirmations** use the past tense and name the thing: "Imported “Q3 plan”.", "Renamed Ada to Grace."

**Placeholders:** one that invites writing ends in an ellipsis ("Write about today…"). One that names the field doesn't ("Filter by title"). An optional field says "(optional)" on its label.

## CLI

Use the same voice as the web UI. Full sentences take a period and contractions: "Couldn’t open a browser. Open the URL above." Only bare label and value output goes without a period.

## Marketing

- **Lead with the outcome.** Say what the person can do, then how it works. "Your tasks, documents and journal in one place." comes before any mention of architecture.
- **Be concrete.** Use real numbers and real examples instead of adjectives.
- **Keep to one idea per section.** No feature dumps.
- **Skip hype.** No superlatives you can't prove, no "revolutionary", and no pressure or urgency tricks.
- **Use fragment headlines sparingly.** A headline that reads as a statement takes a period. Marketing headlines are the only place fragments may replace a sentence. Labels are still labels.

## Docs

- **User-facing docs and the README opening** (`README.md`, `docs/cli.md`, `docs/self-hosting.md`) use the plain voice, leading with what the reader can do.
- **Developer and architecture docs** keep the same mechanics and may use technical terms.

## Checklist

Before finishing, check each new or changed string:

1. Does it use Cohesion's existing word for the concept?
2. In the web UI, does it leave out internals (outside Settings › Developer and About)?
3. Does a sentence open with a bare "It" or "Its", or trail a tacked-on clause?
4. Would an end user know every word?
5. Can any word go without changing the meaning?
6. Does it end with a period only if it's a sentence?
7. Is it in sentence case?
8. Does anything from "Never" appear?
