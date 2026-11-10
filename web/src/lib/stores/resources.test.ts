import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { RequestType, Send } from '$lib/protocol/types';
import { refreshAll, refreshInvalidated } from './resource.svelte';
import { createResourcesStore } from './resources.svelte';
import { createConnectorsStore } from './connectors.svelte';

describe('resources store', () => {
	it('lists the resources of one workspace, newest first', async () => {
		const mock = createMock();
		const connectors = createConnectorsStore(mock.send);
		await connectors.importResource(1, 1, 'db-budget');
		await connectors.importResource(1, 2, 'page-roadmap');
		const store = createResourcesStore(mock.send);

		await store.load(1);
		expect(store.list.value.map((a) => a.identifier)).toEqual([
			'page-launch',
			'page-roadmap',
			'db-budget',
			'page-ideas',
			'page-playlist'
		]);
		expect(store.list.value.every((a) => a.workspace_id === 1)).toBe(true);
		await store.load(2);
		expect(store.list.value.map((a) => a.identifier)).toEqual(['page-roadmap']);
	});

	it('takes an import into the loaded list and drops a removed resource', async () => {
		const mock = createMock();
		const store = createResourcesStore(mock.send);
		await store.load(1);
		const imported = await createConnectorsStore(mock.send).importResource(1, 1, 'db-budget');
		store.imported(imported);
		expect(store.list.value.map((a) => a.id)).toContain(imported.id);

		await store.open(1, imported.id);
		expect(store.current.value?.title).toBe('Budget');
		await store.remove(1, imported.id);
		expect(store.current.value).toBeNull();
		expect(store.list.value.map((a) => a.id)).not.toContain(imported.id);
		await expect(store.open(2, 1)).resolves.toBeUndefined();
		expect(store.current.error?.code).toBe('notFound');
	});

	it('opens a resource with the view its connector draws, if any', async () => {
		const store = createResourcesStore(createMock().send);
		await store.open(1, 4);
		expect(store.view.value?.blocks.length).toBeGreaterThan(0);
		await store.open(1, 1);
		expect(store.current.value?.identifier).toBe('page-roadmap');
		expect(store.view.value).toBeNull();
	});

	it('draws a resource again only when that resource changes, keeping the last view meanwhile', async () => {
		const mock = createMock();
		const sent: RequestType[] = [];
		let contents: string | null = null;
		const send = (async (type, ...args) => {
			sent.push(type);
			const reply = await mock.send(type, ...args);
			return type === 'getResource' && contents !== null
				? { ...(reply as object), contents }
				: reply;
		}) as Send;
		const store = createResourcesStore(send);
		await store.open(1, 4);
		const drawn = () => sent.filter((t) => t === 'renderResource').length;
		const view = store.view.value;
		expect(view?.blocks.length).toBeGreaterThan(0);
		expect(drawn()).toBe(1);

		// Something else in the workspace changed, or the window refocused.
		await refreshInvalidated([{ kind: 'resources', id: 1 }]);
		await refreshAll();
		expect(drawn()).toBe(1);

		// This resource changed: the connector runs again; a refetch never
		// empties the view first.
		contents = 'changed';
		await refreshInvalidated([{ kind: 'resources', id: 1 }]);
		expect(drawn()).toBe(2);
		expect(store.view.value).toEqual(view);
	});

	it('refreshes resources whose copy on the service is newer', async () => {
		const mock = createMock();
		const store = createResourcesStore(mock.send);
		expect(await store.refresh(1)).toEqual({ checked: 3, updated: 1, failed: 0 });
		expect(await store.refresh(1)).toEqual({ checked: 3, updated: 0, failed: 0 });
	});

	it('refetches on an resources invalidation for its own workspace only', async () => {
		const store = createResourcesStore(createMock().send);
		await store.load(1);
		await store.open(1, 1);
		for (const resource of [store.list, store.current]) {
			expect(resource.touchedBy([{ kind: 'resources', id: 1 }])).toBe(true);
			expect(resource.touchedBy([{ kind: 'resources', id: 2 }])).toBe(false);
			expect(resource.touchedBy([{ kind: 'documents', id: 1 }])).toBe(false);
		}
	});

	it('reorders the shown kinds, and a kind shown later goes last', async () => {
		const store = createResourcesStore(createMock().send);
		await store.shownKinds.load(1);
		await store.setShown(1, 2, true);
		expect(store.shownKinds.value.map((k) => k.kind_id)).toEqual([1, 2]);
		await store.reorderShown(1, [2, 1]);
		expect(store.shownKinds.value.map((k) => k.kind_id)).toEqual([2, 1]);
	});
});
