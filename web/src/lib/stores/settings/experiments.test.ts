import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createExperimentsStore } from './experiments.svelte';

function withExperiment() {
	const mock = createMock();
	mock.state.experiments.push({
		key: 'feedParser',
		title: 'New feed parser',
		description: 'Reads feeds with the new parser.',
		enabled: false
	});
	return { mock, store: createExperimentsStore(mock.send) };
}

describe('experiments store', () => {
	it('offers no experiments yet', async () => {
		const store = createExperimentsStore(createMock().send);
		await store.load();
		expect(store.list.value).toEqual([]);
		expect(store.isEnabled('feedParser')).toBe(false);
	});

	it('turns an experiment on and off', async () => {
		const { store } = withExperiment();
		await store.load();
		expect(store.isEnabled('feedParser')).toBe(false);

		await store.set('feedParser', true);
		expect(store.isEnabled('feedParser')).toBe(true);

		await store.set('feedParser', false);
		expect(store.isEnabled('feedParser')).toBe(false);
	});

	it('refuses a key no experiment has', async () => {
		const { store } = withExperiment();
		await store.load();
		await expect(store.set('gone', true)).rejects.toThrow();
	});
});
