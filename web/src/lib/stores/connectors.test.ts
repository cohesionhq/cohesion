import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createResourcesStore } from './resources.svelte';
import { createConnectorsStore } from './connectors.svelte';
import { createExtensionsStore } from './extensions.svelte';

describe('connectors store', () => {
	it('lists only extensions granted `oauth` as services', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		expect((await store.services()).map((s) => s.extension_id)).toEqual([4, 5]);

		await createExtensionsStore(mock.send).setPermission(5, 'oauth', false);
		expect((await store.services()).map((s) => s.extension_id)).toEqual([4]);
		await expect(store.beginAuthentication(5)).rejects.toMatchObject({
			code: 'validationFailed'
		});
	});

	it('flags an account whose sign-in failed until it signs in again', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		await store.load();
		expect(store.account(1)?.credential_failed_at).not.toBeNull();
		expect(store.account(2)?.credential_failed_at).toBeNull();

		const again = await store.completeAuthentication(4, 'code');
		expect(again.id).toBe(1);
		expect(store.account(1)?.credential_failed_at).toBeNull();
	});

	it('imports only from accounts whose extension is granted `connector`', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		await store.load();
		expect(store.account(1)?.importable).toBe(true);

		await createExtensionsStore(mock.send).setPermission(4, 'connector', false);
		await store.accounts.refetch();
		expect(store.account(1)?.importable).toBe(false);
		await expect(store.organizationUnits(1)).rejects.toMatchObject({
			code: 'validationFailed'
		});
		// Signing in needs only `oauth`.
		expect((await store.completeAuthentication(4, 'code')).importable).toBe(false);
	});

	it('adds an account from a completed sign-in, and replaces it on a second one', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		await store.load();
		const before = store.accounts.value.length;

		const account = await store.completeAuthentication(5, 'code-1');
		expect(account.connector.icon).toBeNull();
		expect(store.accounts.value.length).toBe(before + 1);
		expect(store.account(account.id)?.connector.title).toBe('Team wiki');

		const again = await store.completeAuthentication(5, 'code-2');
		expect(again.id).toBe(account.id);
		expect(store.accounts.value.length).toBe(before + 1);
	});

	it('marks what the workspace already imported, and imports in place', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		const search = {
			account_id: 1,
			workspace_id: 1,
			kind_id: 1,
			organization_unit_id: null,
			search: null
		};
		const found = await store.searchResources(search);
		expect(found.map((r) => [r.identifier, r.imported])).toEqual([
			['page-roadmap', true],
			['page-launch', true],
			['page-playlist', true]
		]);
		// Each kind lists its own.
		expect(
			(await store.searchResources({ ...search, kind_id: 2, organization_unit_id: 'ops' })).map(
				(r) => [r.identifier, r.imported]
			)
		).toEqual([['db-budget', false]]);
		// Another workspace has imported nothing.
		expect(
			(await store.searchResources({ ...search, workspace_id: 2 })).some((r) => r.imported)
		).toBe(false);

		const resource = await store.importResource(1, 1, 'db-budget');
		expect([resource.kind, resource.kind_id]).toEqual(['com.example.notion.database', 2]);
		expect(resource.content_kind).toBe('json');
		expect((await store.searchResources({ ...search, kind_id: 2 })).every((r) => r.imported)).toBe(
			true
		);
	});

	it('removing an account removes its resources from every workspace', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		await store.load();
		await store.importResource(1, 2, 'page-roadmap');
		const resources = createResourcesStore(mock.send);
		await resources.load(1);
		expect(resources.list.value.length).toBeGreaterThan(0);

		await store.remove(1);
		expect(store.account(1)).toBeUndefined();
		expect(mock.state.resources.filter((a) => a.account_id === 1)).toEqual([]);
		await resources.list.refetch();
		expect(resources.list.value.some((r) => r.account_id === 1)).toBe(false);
	});

	it('refetches accounts on a connectorAccounts invalidation', async () => {
		const store = createConnectorsStore(createMock().send);
		await store.load();
		expect(store.accounts.touchedBy([{ kind: 'connectorAccounts' }])).toBe(true);
		expect(store.accounts.touchedBy([{ kind: 'resources', id: 1 }])).toBe(false);
	});

	it('moves an orphan’s resources to another account of its connector', async () => {
		const mock = createMock();
		const store = createConnectorsStore(mock.send);
		await store.load();
		const orphan = store.account(2)!;
		expect(store.siblings(orphan).map((a) => a.id)).toEqual([1]);

		await store.migrate(2, 1);
		expect(mock.state.resources.filter((a) => a.account_id === 2)).toEqual([]);
		expect(
			mock.state.resources.some((a) => a.account_id === 1 && a.identifier === 'page-ideas')
		).toBe(true);
	});

	it('lists each connector with an importable account once', async () => {
		const store = createConnectorsStore(createMock().send);
		await store.load();
		expect(store.connectors.map((c) => c.title)).toEqual(['Notion']);
	});
});
