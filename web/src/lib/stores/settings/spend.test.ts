import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { refreshInvalidated } from '../resource.svelte';
import { createSpendStore } from './spend.svelte';

describe('spend store', () => {
	it('lists every period, shortest first, with its limit', async () => {
		const store = createSpendStore(createMock().send);
		await store.load();
		expect(store.periods.value.map((p) => [p.period, p.limit])).toEqual([
			['hour', null],
			['day', null],
			['month', 20]
		]);
		expect(store.reached).toEqual([]);
		store.periods.release();
	});

	it('sets and clears a limit from the reply, reached once spent', async () => {
		const store = createSpendStore(createMock().send);
		await store.load();
		await store.setLimit('day', 0.001);
		expect(store.periods.value[1].limit).toBe(0.001);
		expect(store.reached.map((p) => p.period)).toEqual(['day']);

		await store.clearLimit('day');
		expect(store.periods.value[1].limit).toBeNull();
		expect(store.reached).toEqual([]);
		store.periods.release();
	});

	it('refuses a limit of nothing', async () => {
		const store = createSpendStore(createMock().send);
		await store.load();
		await expect(store.setLimit('hour', 0)).rejects.toThrow();
		expect(store.periods.value[0].limit).toBeNull();
		store.periods.release();
	});

	it('picks up a new call’s cost when cost events are invalidated', async () => {
		const mock = createMock();
		const store = createSpendStore(mock.send);
		await store.load();
		const before = store.periods.value[0].spent;

		await mock.send('sendAgentMessage', { agent_id: 1, message: 'Hello' });
		await refreshInvalidated([{ kind: 'intelligenceCostEvents' }]);
		expect(store.periods.value[0].spent).toBeGreaterThan(before);
		store.periods.release();
	});
});
