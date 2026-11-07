import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { refreshInvalidated } from '../resource.svelte';
import { createClientsStore, shortKey } from './clients.svelte';

describe('clients store', () => {
	it('revokes a client and drops its row', async () => {
		const mock = createMock();
		const store = createClientsStore(mock.send);
		await store.load();
		expect(store.list.value.map((c) => c.client_name)).toEqual(['apollo', 'iPhone', 'MBP']);

		const revoked = await store.revoke(1);
		expect(revoked.deleted).toBe(true);
		expect(store.list.value.map((c) => c.client_name)).toEqual(['apollo', 'MBP']);
		await expect(store.revoke(1)).rejects.toMatchObject({ code: 'notFound' });
		expect(store.list.value).toHaveLength(2);
	});

	it('knows which clients are online, and forgets a revoked one', async () => {
		const mock = createMock();
		const store = createClientsStore(mock.send);
		await store.load();
		expect([2, 3].map((id) => store.isOnline(id))).toEqual([true, false]);

		await store.revoke(2);
		expect(store.isOnline(2)).toBe(false);
	});

	it('refetches the list when the clients change', async () => {
		const mock = createMock();
		const store = createClientsStore(mock.send);
		await store.load();
		mock.state.onlineClients.push(3);
		await refreshInvalidated([{ kind: 'clients' }]);
		expect(store.isOnline(3)).toBe(true);
	});

	it('marks the client this page is, and revokes every other', async () => {
		const mock = createMock();
		const store = createClientsStore(mock.send);
		await store.load();
		expect(store.current?.client_name).toBe('apollo');

		expect(await store.revokeOthers()).toBe(2);
		expect(store.list.value.map((c) => c.client_name)).toEqual(['apollo']);
	});

	it('says when this client can’t open pairing, and refetches when security changes', async () => {
		const mock = createMock();
		const store = createClientsStore(mock.send);
		mock.state.currentClient = 1;
		await store.load();
		expect(store.pairing.value.refusal).toBeNull();

		mock.state.security.pairing_opened_by = 'localClients';
		await refreshInvalidated([{ kind: 'security' }]);
		expect(store.pairing.value.refusal).toMatch(/computer Cohesion runs on/);
		await expect(store.beginPairing()).rejects.toMatchObject({ code: 'badRequest' });
	});

	it('shortens a key to its first 12 hex digits', () => {
		expect(shortKey('0123456789ABCDEF0123')).toBe('0123456789ab…');
		expect(shortKey('ab:cd')).toBe('abcd');
	});
});
