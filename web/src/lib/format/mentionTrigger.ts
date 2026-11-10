// What opens the item picker in the editor: `@` or `[[`, typed at a word's
// start, with a character after it. Both open the same picker and write the
// same `[title](cohesion://…)`; `[[` is only another way to ask for it.

/** Where the picker was asked for, and what has been typed since. */
export interface MentionTrigger {
	/** The offset of the `@` or `[[` in the text it was found in. */
	at: number;
	/** What follows it, up to the caret: the search. */
	query: string;
}

// Each is the opener, then a first character that isn't a space (or the opener
// again), then up to 60 more, all at the end of the text. A `[[` search stops at
// a bracket, so a `[[Title]]` typed out whole closes the picker.
const OPENERS = [
	{ open: '@', typed: /(?:^|\s)@[^\s@][^@\n]{0,60}$/ },
	{ open: '[[', typed: /(?:^|\s)\[\[[^\s[\]][^[\]\n]{0,60}$/ }
];

/**
 * The picker `before` (a line's text up to the caret) asks for, or null. When
 * both an `@` and a `[[` are open, the one typed last.
 */
export function mentionTrigger(before: string): MentionTrigger | null {
	let found: MentionTrigger | null = null;
	for (const { open, typed } of OPENERS) {
		const match = typed.exec(before);
		if (!match) continue;
		const at = match.index + match[0].indexOf(open);
		if (!found || at > found.at) found = { at, query: before.slice(at + open.length) };
	}
	return found;
}
