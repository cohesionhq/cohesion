import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { sidebarState } from './sidebarState.svelte';

describe('sidebarState', () => {
	let store: Record<string, string>;
	beforeEach(() => {
		store = {};
		vi.stubGlobal('localStorage', {
			getItem: (key: string) => store[key] ?? null,
			setItem: (key: string, value: string) => (store[key] = value)
		});
		sidebarState.set(false);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('toggles and remembers the choice', () => {
		sidebarState.toggle();
		expect(sidebarState.collapsed).toBe(true);
		sidebarState.collapsed = false;
		sidebarState.restore();
		expect(sidebarState.collapsed).toBe(true);
		sidebarState.toggle();
		sidebarState.restore();
		expect(sidebarState.collapsed).toBe(false);
	});

	it('starts expanded when storage is blocked', () => {
		vi.stubGlobal('localStorage', {
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			}
		});
		sidebarState.restore();
		expect(sidebarState.collapsed).toBe(false);
		sidebarState.toggle();
		expect(sidebarState.collapsed).toBe(true);
	});
});
