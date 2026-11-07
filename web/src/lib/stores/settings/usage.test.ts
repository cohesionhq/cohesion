import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { refreshInvalidated } from '../resource.svelte';
import { createUsageStore } from './usage.svelte';

describe('usage store', () => {
	it('lists cost events newest first, including a new message’s', async () => {
		const mock = createMock();
		await mock.send('sendAgentMessage', { agent_id: 1, message: 'Hello' });
		const store = createUsageStore(mock.send);
		await store.load();
		const ids = store.events.value.map((e) => e.id);
		expect(ids).toEqual([...ids].sort((a, b) => b - a));
		expect(store.events.value[0].thread_id).toBe(mock.state.threads.at(-1)?.id);
	});

	it('picks up a call recorded elsewhere when cost events are invalidated', async () => {
		const mock = createMock();
		const store = createUsageStore(mock.send);
		await store.load();
		const before = store.events.value.length;

		await mock.send('sendAgentMessage', { agent_id: 1, message: 'Hello' });
		await refreshInvalidated([{ kind: 'workspace', id: 1 }]);
		expect(store.events.value).toHaveLength(before);
		await refreshInvalidated([{ kind: 'intelligenceCostEvents' }]);
		expect(store.events.value).toHaveLength(before + 1);
		store.events.release();
	});

	it('shows a call made while the section was closed when it opens again', async () => {
		const mock = createMock();
		const store = createUsageStore(mock.send);
		await store.load();
		const before = store.events.value.length;
		store.events.release();

		await mock.send('sendAgentMessage', { agent_id: 1, message: 'Hello' });
		await refreshInvalidated([{ kind: 'intelligenceCostEvents' }]);
		await store.load();
		expect(store.events.value).toHaveLength(before + 1);
		store.events.release();
	});
});
