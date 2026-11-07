import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createStorageStore } from './storage.svelte';

describe('storage store', () => {
	it('loads the settings and the database size', async () => {
		const mock = createMock();
		const store = createStorageStore(mock.send);
		await store.load();
		expect(store.settings.value?.memory_limit).toBe(20);
		expect(store.size.value?.bytes).toBe(mock.state.databaseSize.bytes);
	});

	it('changes one setting and keeps the rest', async () => {
		const mock = createMock();
		const store = createStorageStore(mock.send);
		await store.load();
		const before = store.settings.value!;

		await store.update({ actor_session_retention_days: 30 });
		expect(store.settings.value).toEqual({ ...before, actor_session_retention_days: 30 });
		expect(mock.state.storage.actor_session_retention_days).toBe(30);
	});

	it('keeps the settings when a value is refused', async () => {
		const mock = createMock();
		const store = createStorageStore(mock.send);
		await store.load();
		const before = store.settings.value!;

		await expect(store.update({ memory_limit: 0 })).rejects.toThrow();
		expect(store.settings.value).toEqual(before);
	});
});
