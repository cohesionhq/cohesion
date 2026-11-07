// App-wide keyboard shortcuts: which key presses the layout acts on, and whether
// the shortcuts dialog is open. The layout listens; the palette can open the dialog.
// Pages are reached from the ⌘K palette, not by chords of their own.

class Shortcuts {
	/** The "Keyboard shortcuts" dialog. */
	open = $state(false);
}

export const shortcuts = new Shortcuts();

/**
 * Whether a key press opens Settings: ⌘, (Ctrl+,), as an app's preferences
 * open, matched by the physical key. Every other page is reached from ⌘K.
 */
export function opensSettings(
	event: Pick<KeyboardEvent, 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>
): boolean {
	return (
		(event.metaKey || event.ctrlKey) &&
		!(event.metaKey && event.ctrlKey) &&
		!event.altKey &&
		!event.shiftKey &&
		event.code === 'Comma'
	);
}

/**
 * Which way a key press steps through history: -1 back, 1 forward. ⌘[ and
 * ⌘] (matched by the physical key) and ⌘← and ⌘→, as Safari takes them, and
 * ⌥← and ⌥→ (Alt elsewhere), as other browsers do. The page acts on them
 * itself, so they work where nothing else would, such as the WebUI harnesses' web view.
 */
export function historyStepFor(
	event: Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>
): -1 | 1 | null {
	if (event.shiftKey || event.ctrlKey || event.metaKey === event.altKey) return null;
	if (event.key === 'ArrowLeft') return -1;
	if (event.key === 'ArrowRight') return 1;
	if (!event.metaKey) return null;
	if (event.code === 'BracketLeft') return -1;
	if (event.code === 'BracketRight') return 1;
	return null;
}

/**
 * Whether a key press asks for the shortcuts dialog: ⌘/ (Ctrl+/), which
 * works in a field too, as no browser claims it. A bare "?" does as well, but
 * only where nothing is being typed, so the layout checks that itself.
 */
export function showsShortcuts(
	event: Pick<KeyboardEvent, 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>
): boolean {
	return (
		(event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey && event.code === 'Slash'
	);
}

/** Which way a mouse's side button steps through history: its back button (3) or forward one (4). */
export function historyStepForButton(button: number): -1 | 1 | null {
	if (button === 3) return -1;
	if (button === 4) return 1;
	return null;
}

/** Whether a key press belongs to whatever has focus: a field, an editor, a dialog or a menu. */
export function isTyping(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	return (
		target.isContentEditable ||
		target.closest(
			'input, textarea, select, [contenteditable], [role="dialog"], [role="menu"], [role="listbox"]'
		) !== null
	);
}
