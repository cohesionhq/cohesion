import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { ActorSessionStatus } from '$lib/protocol/types';
import { createActorsStore } from './actors.svelte';

describe('actors store', () => {
	it('marks a session read in the list and the open session at once', async () => {
		const mock = createMock();
		const store = createActorsStore(mock.send);
		await Promise.all([store.sessions.load(), store.session.load(2)]);
		expect(store.session.value?.unread).toBe(true);

		const marked = store.markRead(2);
		expect(store.sessions.value.find((t) => t.id === 2)?.unread).toBe(false);
		expect(store.session.value?.unread).toBe(false);
		await marked;
		expect(mock.state.actorSessions.find((t) => t.id === 2)?.unread).toBe(false);
	});

	it('lists waiting, then working, then unread, then read, each latest change first', async () => {
		const mock = createMock();
		const [base] = mock.state.actorSessions;
		const session = (id: number, status: ActorSessionStatus, unread: boolean, at: number) => ({
			...base,
			id,
			status,
			unread,
			status_changed_at: at
		});
		mock.state.actorSessions = [
			session(1, 'idle', false, 100),
			session(2, 'working', false, 90),
			session(3, 'failed', true, 80),
			session(4, 'needsAttention', false, 70),
			session(5, 'idle', true, 60),
			session(6, 'working', false, 50),
			session(7, 'needsAttention', false, 40),
			session(8, 'failed', false, 30)
		];
		const store = createActorsStore(mock.send);
		await store.sessions.load();

		expect(store.current.map((t) => t.id)).toEqual([4, 7, 2, 6, 3, 5, 1, 8]);
	});

	it('lists every session not resolved that an agent follows, in the sidebar order', async () => {
		const mock = createMock();
		const [base] = mock.state.actorSessions;
		const session = (id: number, status: ActorSessionStatus, follower: number | null) => ({
			...base,
			id,
			status,
			follower: follower === null ? null : { agent_id: follower, initiative: 'user' as const }
		});
		mock.state.actorSessions = [
			session(1, 'working', 1),
			session(2, 'needsAttention', 1),
			session(3, 'working', 2),
			session(4, 'working', null),
			session(5, 'idle', 2),
			session(6, 'failed', 1),
			session(7, 'resolved', 1)
		];
		const store = createActorsStore(mock.send);
		await store.sessions.load();

		const ids = (agent: number) => store.followedBy(agent).map((t) => t.id);
		expect(ids(1)).toEqual([2, 1, 6]);
		expect(ids(2)).toEqual([3, 5]);
		expect(ids(3)).toEqual([]);

		await store.resolve(6);
		expect(ids(1)).toEqual([2, 1]);
	});

	it('keeps every session until it is resolved, which takes it out of the list at once', async () => {
		const mock = createMock();
		const [base] = mock.state.actorSessions;
		const session = (id: number, status_changed_at: number) => ({
			...base,
			id,
			status: 'idle' as const,
			unread: false,
			status_changed_at,
			read_at: status_changed_at
		});
		mock.state.actorSessions = [session(1, 100), session(2, 10)];
		const store = createActorsStore(mock.send);
		await Promise.all([store.sessions.load(), store.session.load(2)]);
		expect(store.current.map((t) => t.id)).toEqual([1, 2]);

		await store.resolve(2);
		expect(store.current.map((t) => t.id)).toEqual([1]);
		expect(store.session.value?.status).toBe('resolved');
		await store.sessions.refetch();
		expect(store.current.map((t) => t.id)).toEqual([1]);
	});
});
