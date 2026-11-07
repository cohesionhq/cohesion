// Keyboard shortcuts as this platform writes them: a Mac's symbols run
// together (⌘K, ⇧↩), elsewhere words are joined by "+" (Ctrl+K, Shift+Enter).

export const isMac = typeof navigator !== 'undefined' && /Mac|iP(hone|ad)/.test(navigator.platform);

const MAC = {
	mod: '⌘',
	shift: '⇧',
	alt: '⌥',
	enter: '↩',
	backspace: '⌫',
	space: 'Space',
	up: '↑',
	down: '↓',
	left: '←',
	right: '→'
};

const OTHER: Record<keyof typeof MAC, string> = {
	mod: 'Ctrl',
	shift: 'Shift',
	alt: 'Alt',
	enter: 'Enter',
	backspace: 'Backspace',
	space: 'Space',
	up: '↑',
	down: '↓',
	left: '←',
	right: '→'
};

/** A named key, or a letter or symbol key written as typed (`'k'`, `'?'`). */
export type Key = keyof typeof MAC | (string & {});

/**
 * One chord, held together: `keyLabel('mod', 'k')` is "⌘K" on a Mac and
 * "Ctrl+K" elsewhere. A letter alone stays lowercase, as typed; after a
 * modifier it is written as a capital, as the keycap shows it.
 */
export function keyLabel(...keys: Key[]): string {
	const names = isMac ? MAC : OTHER;
	const chord = keys.length > 1;
	const labels = keys.map((key) =>
		key in names ? names[key as keyof typeof MAC] : chord ? key.toUpperCase() : key
	);
	return labels.join(isMac ? '' : '+');
}

/**
 * Whether a key press is exactly this one key, bare: no ⌘, Ctrl, ⌥ or ⇧
 * held. A named key is matched by its name (`'backspace'` is Backspace), a
 * letter or digit as typed.
 */
export function pressesKey(
	event: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>,
	key: Key
): boolean {
	if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return false;
	return event.key.toLowerCase() === key;
}
