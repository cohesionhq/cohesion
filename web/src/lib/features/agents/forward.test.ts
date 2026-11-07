import { describe, expect, it } from 'vitest';
import { forwardTarget, forwardedKind, forwardedLabel, forwardedLink } from './forward.svelte';

const task = { kind: 'task', id: 42, workspace_id: 3, title: 'Fix login' } as const;
const entry = { kind: 'journal', id: 57, workspace_id: 3, title: '2026-10-20' } as const;

describe('forwardedLabel', () => {
	it('names a task by its title and id', () => {
		expect(forwardedLabel(task)).toBe('Task “Fix login” #42');
		expect(forwardedKind(task)).toBe('Task #42');
	});

	it('names a journal entry by its day in words', () => {
		expect(forwardedLabel(entry)).toMatch(/^Journal entry “.*20.*”$/);
		expect(forwardedLabel(entry)).not.toContain('2026-10-20');
	});
});

describe('forwardTarget', () => {
	it('names a journal entry as the document it is, as its link does', () => {
		expect(forwardTarget(entry)).toEqual({ kind: 'document', workspace_id: 3, id: 57 });
		expect(forwardedLink(entry)).toEqual({ kind: 'document', ws: 3, id: 57 });
		expect(forwardTarget(task)).toEqual({ kind: 'task', workspace_id: 3, id: 42 });
	});
});
