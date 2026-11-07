import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createMemoriesStore } from './memories.svelte';
import { refreshInvalidated } from '../resource.svelte';

describe('memories store', () => {
	it('lists the global memories once, each team’s and each agent’s own', async () => {
		const mock = createMock();
		const store = createMemoriesStore(mock.send);
		await store.load();
		const { global, teams, agents } = store.list.value;
		expect(global.map((m) => m.memory)).toEqual(['Don’t send messages about unimportant things.']);
		expect(teams.map((t) => [t.team.title, t.memories.map((m) => m.memory)])).toEqual([
			[
				'Assistants',
				[
					'When the user asks for a task, react 👍 to their message instead of replying, unless more is needed.'
				]
			],
			['Cohesion Engineers', []]
		]);
		expect(agents.map((a) => [a.agent.agent_name, a.memories.map((m) => m.memory)])).toEqual([
			['Core Engineer', []],
			['Secretary', ['Daemon commands and routes go under Core.']],
			['Researcher', []],
			['Plus Engineer', []]
		]);
	});

	it('forgets a memory and drops it from the list', async () => {
		const mock = createMock();
		const store = createMemoriesStore(mock.send);
		await store.load();
		await store.forget(2);
		expect(store.list.value.agents.every((a) => a.memories.length === 0)).toBe(true);
		expect(mock.state.memories.map((m) => m.id)).toEqual([1, 3]);
		await expect(store.forget(2)).rejects.toMatchObject({ code: 'notFound' });
	});

	it('refetches when the memories change', async () => {
		const mock = createMock();
		const store = createMemoriesStore(mock.send);
		await store.load();
		mock.state.memories.push({
			id: 4,
			scope: { kind: 'agent', agent_id: 1 },
			created_at: 0,
			memory: 'Likes short summaries.'
		});
		await refreshInvalidated([{ kind: 'memories' }]);
		expect(store.list.value.agents[0].memories.map((m) => m.id)).toEqual([4]);
	});
});

describe('memories an agent sees', () => {
	it('lists its own, its team’s and the global ones', async () => {
		const mock = createMock();
		const store = createMemoriesStore(mock.send);
		const seen = await store.seenBy(2);
		expect(seen.map((m) => m.scope.kind).toSorted()).toEqual(['agent', 'global', 'team']);
	});
});
