// Typed text as a full-text query. The daemon's search takes SQLite FTS5
// syntax, where punctuation is an operator; as-you-type search wants every
// word to match the start of a word instead, whatever the text holds.

/** How long as-you-type search waits after the last keystroke before asking the daemon. */
export const SEARCH_DEBOUNCE_MS = 250;

/**
 * `"garden pl"` → `"garden"* "pl"*`: each run of letters and digits quoted
 * and prefix-matched, all of them required. Null when there are no words.
 */
export function prefixQuery(text: string): string | null {
	const words = text.match(/[\p{L}\p{N}]+/gu);
	return words === null ? null : words.map((word) => `"${word}"*`).join(' ');
}

/**
 * The first line of `contents` holding a word of `text` as typed (so a
 * prefix counts), else its first non-blank line: what a result shows to
 * say why it matched.
 */
export function matchingLine(contents: string, text: string): string {
	const words = (text.match(/[\p{L}\p{N}]+/gu) ?? []).map((word) => word.toLowerCase());
	const lines = contents
		.split('\n')
		.map((line) => line.trim())
		.filter((line) => line !== '');
	const hit = lines.find((line) => {
		const lower = line.toLowerCase();
		return words.some((word) => lower.includes(word));
	});
	return hit ?? lines[0] ?? '';
}
