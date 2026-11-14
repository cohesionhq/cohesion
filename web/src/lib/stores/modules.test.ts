import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createModulesStore } from './modules.svelte';

describe('modules store', () => {
	it('has every module on in a workspace that changed none', async () => {
		const store = createModulesStore(createMock().send);
		await store.current.load(1);
		expect(store.current.value.map((m) => [m.module, m.enabled])).toEqual([['calendar', true]]);
		expect(store.isEnabled('calendar')).toBe(true);
	});

	it('has a module on before its setting loads', () => {
		const store = createModulesStore(createMock().send);
		expect(store.isEnabled('calendar')).toBe(true);
	});

	it('reads a module turned off in the selected workspace', async () => {
		// The seed turns Calendar off in workspace 3.
		const store = createModulesStore(createMock().send);
		await store.current.load(3);
		expect(store.isEnabled('calendar')).toBe(false);
	});

	it('turns a module off and on again in one workspace only', async () => {
		const store = createModulesStore(createMock().send);
		await store.current.load(1);
		await store.edited.load(1);

		await store.set(1, 'calendar', false);
		expect(store.isEnabled('calendar')).toBe(false);
		expect(store.edited.value[0].enabled).toBe(false);

		await store.edited.load(2);
		expect(store.edited.value[0].enabled).toBe(true);

		await store.set(1, 'calendar', true);
		expect(store.isEnabled('calendar')).toBe(true);
	});
});
