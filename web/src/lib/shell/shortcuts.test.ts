import { describe, expect, it } from 'vitest';
import { pressesKey } from './keys';
import {
	historyStepFor,
	historyStepForButton,
	asksForNew,
	asksForNewFolder,
	focusesFolderHeading,
	opensSettings,
	togglesEditing,
	showsShortcuts,
	togglesSidebar,
	workspaceIndexFor
} from './shortcuts.svelte';

const press = (code: string, key: string, mods: Partial<KeyboardEvent> = {}) => ({
	code,
	key,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods
});

describe('opensSettings', () => {
	it('opens Settings on ⌘, or Ctrl+, alone', () => {
		expect(opensSettings(press('Comma', ',', { metaKey: true }))).toBe(true);
		expect(opensSettings(press('Comma', ',', { ctrlKey: true }))).toBe(true);
		expect(opensSettings(press('Comma', '≤', { altKey: true }))).toBe(false);
		expect(opensSettings(press('Comma', '<', { metaKey: true, shiftKey: true }))).toBe(false);
		expect(opensSettings(press('Comma', ','))).toBe(false);
	});

	it('leaves the old page chords alone', () => {
		expect(opensSettings(press('KeyJ', 'j', { metaKey: true }))).toBe(false);
		expect(opensSettings(press('Digit1', '1', { metaKey: true }))).toBe(false);
	});
});

describe('historyStepFor', () => {
	it('steps back and forward on ⌘[ ⌘], ⌘← ⌘→ and ⌥← ⌥→', () => {
		expect(historyStepFor(press('BracketLeft', '[', { metaKey: true }))).toBe(-1);
		expect(historyStepFor(press('BracketRight', ']', { metaKey: true }))).toBe(1);
		// A layout that types something else on the key still matches it.
		expect(historyStepFor(press('BracketLeft', 'ü', { metaKey: true }))).toBe(-1);
		expect(historyStepFor(press('ArrowLeft', 'ArrowLeft', { metaKey: true }))).toBe(-1);
		expect(historyStepFor(press('ArrowLeft', 'ArrowLeft', { altKey: true }))).toBe(-1);
		expect(historyStepFor(press('ArrowRight', 'ArrowRight', { altKey: true }))).toBe(1);
	});

	it('ignores a bare key, ⌥[, Ctrl, Shift and two modifiers at once', () => {
		expect(historyStepFor(press('ArrowLeft', 'ArrowLeft'))).toBeNull();
		expect(historyStepFor(press('BracketLeft', '“', { altKey: true }))).toBeNull();
		expect(historyStepFor(press('BracketLeft', '[', { ctrlKey: true }))).toBeNull();
		expect(
			historyStepFor(press('ArrowLeft', 'ArrowLeft', { metaKey: true, shiftKey: true }))
		).toBeNull();
		expect(
			historyStepFor(press('ArrowLeft', 'ArrowLeft', { metaKey: true, altKey: true }))
		).toBeNull();
	});

	it('steps on the mouse’s back and forward buttons only', () => {
		expect(historyStepForButton(3)).toBe(-1);
		expect(historyStepForButton(4)).toBe(1);
		expect(historyStepForButton(0)).toBeNull();
		expect(historyStepForButton(1)).toBeNull();
	});
});

describe('showsShortcuts', () => {
	it('takes ⇧⌘/ or Ctrl+Shift+/, and nothing else on that key', () => {
		expect(showsShortcuts(press('Slash', '?', { shiftKey: true, metaKey: true }))).toBe(true);
		expect(showsShortcuts(press('Slash', '?', { shiftKey: true, ctrlKey: true }))).toBe(true);
		expect(showsShortcuts(press('Slash', '/', { metaKey: true }))).toBe(false);
		expect(showsShortcuts(press('Slash', '?', { shiftKey: true }))).toBe(false);
		expect(
			showsShortcuts(press('Slash', '?', { shiftKey: true, altKey: true, metaKey: true }))
		).toBe(false);
	});
});

describe('togglesSidebar', () => {
	it('takes ⌘/ or Ctrl+/, and nothing else on that key', () => {
		expect(togglesSidebar(press('Slash', '/', { metaKey: true }))).toBe(true);
		expect(togglesSidebar(press('Slash', '/', { ctrlKey: true }))).toBe(true);
		expect(togglesSidebar(press('Slash', '/'))).toBe(false);
		expect(togglesSidebar(press('Slash', '?', { shiftKey: true, metaKey: true }))).toBe(false);
		expect(togglesSidebar(press('Slash', '/', { altKey: true, metaKey: true }))).toBe(false);
	});
});

describe('pressesKey', () => {
	it('matches a bare key by name or as typed, and nothing held with it', () => {
		expect(pressesKey(press('Digit1', '1'), '1')).toBe(true);
		expect(pressesKey(press('KeyO', 'O'), 'o')).toBe(true);
		expect(pressesKey(press('Backspace', 'Backspace'), 'backspace')).toBe(true);
		expect(pressesKey(press('Digit2', '2'), '1')).toBe(false);
		expect(pressesKey(press('Digit1', '1', { metaKey: true }), '1')).toBe(false);
		expect(pressesKey(press('Digit1', '1', { altKey: true }), '1')).toBe(false);
	});
});

describe('asksForNew and asksForNewFolder', () => {
	it('takes ⌘N or Ctrl+N for a new item', () => {
		expect(asksForNew(press('KeyN', 'n', { metaKey: true }))).toBe(true);
		expect(asksForNew(press('KeyN', 'n', { ctrlKey: true }))).toBe(true);
		expect(asksForNew(press('KeyN', 'n'))).toBe(false);
		expect(asksForNew(press('KeyN', 'N', { metaKey: true, shiftKey: true }))).toBe(false);
		expect(asksForNew(press('KeyN', 'n', { metaKey: true, altKey: true }))).toBe(false);
	});
	it('takes the shifted chord for a new folder', () => {
		expect(asksForNewFolder(press('KeyN', 'N', { metaKey: true, shiftKey: true }))).toBe(true);
		expect(asksForNewFolder(press('KeyN', 'N', { ctrlKey: true, shiftKey: true }))).toBe(true);
		expect(asksForNewFolder(press('KeyN', 'n', { metaKey: true }))).toBe(false);
	});
});

describe('focusesFolderHeading', () => {
	it('takes ⌘D or Ctrl+D alone', () => {
		expect(focusesFolderHeading(press('KeyD', 'd', { metaKey: true }))).toBe(true);
		expect(focusesFolderHeading(press('KeyD', 'd', { ctrlKey: true }))).toBe(true);
		expect(focusesFolderHeading(press('KeyD', 'd'))).toBe(false);
		expect(focusesFolderHeading(press('KeyD', 'D', { metaKey: true, shiftKey: true }))).toBe(false);
		expect(focusesFolderHeading(press('KeyD', 'd', { metaKey: true, altKey: true }))).toBe(false);
	});
});

describe('togglesEditing', () => {
	it('takes ⌘E or Ctrl+E alone', () => {
		expect(togglesEditing(press('KeyE', 'e', { metaKey: true }))).toBe(true);
		expect(togglesEditing(press('KeyE', 'e', { ctrlKey: true }))).toBe(true);
		expect(togglesEditing(press('KeyE', 'e'))).toBe(false);
		expect(togglesEditing(press('KeyE', 'E', { metaKey: true, shiftKey: true }))).toBe(false);
		expect(togglesEditing(press('KeyE', 'e', { metaKey: true, altKey: true }))).toBe(false);
	});
});

describe('workspaceIndexFor', () => {
	it('maps ⌘1 to ⌘9 or Ctrl+1 to Ctrl+9 to positions 0 to 8', () => {
		expect(workspaceIndexFor(press('Digit1', '1', { metaKey: true }))).toBe(0);
		expect(workspaceIndexFor(press('Digit9', '9', { ctrlKey: true }))).toBe(8);
	});

	it('ignores other keys and extra modifiers', () => {
		expect(workspaceIndexFor(press('Digit0', '0', { metaKey: true }))).toBeNull();
		expect(workspaceIndexFor(press('Digit1', '1'))).toBeNull();
		expect(workspaceIndexFor(press('Digit1', '1', { metaKey: true, shiftKey: true }))).toBeNull();
		expect(workspaceIndexFor(press('Digit1', '1', { metaKey: true, altKey: true }))).toBeNull();
		expect(workspaceIndexFor(press('KeyK', 'k', { metaKey: true }))).toBeNull();
	});
});
