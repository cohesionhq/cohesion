import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { refreshInvalidated } from './resource.svelte';
import { createExtensionsStore, sortHooks, sortPermissions } from './extensions.svelte';

describe('extensions store', () => {
	it('refetches the updates when the daemon says they changed', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.updates.load();
		const before = store.updates.value.length;
		expect(before).toBeGreaterThan(0);
		const updated = store.updates.value[0];
		mock.state.extensions.find((e) => e.id === updated.extension_id)!.version =
			updated.result.manifest.version;
		await refreshInvalidated([{ kind: 'extensionUpdates' }]);
		expect(store.updates.value).toHaveLength(before - 1);
	});

	it('lists installed extensions alphabetically by script identifier', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.load();
		const identifiers = store.list.value.map((e) => e.script_identifier);
		expect(identifiers).toEqual(identifiers.toSorted((a, b) => a.localeCompare(b)));
		expect(identifiers).not.toEqual(mock.state.extensions.map((e) => e.script_identifier));
	});

	it('toggles a permission in place and on the daemon', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.load();
		const id = store.list.value.find(
			(e) => e.script_identifier === 'com.cohesion.cloud.ios-companion'
		)!.id;
		const permissions = store.permissions(id);
		await permissions.load(id);
		expect(permissions.value.find((p) => p.permission === 'network')?.enabled).toBe(false);

		await store.setPermission(id, 'network', true);
		expect(permissions.value.find((p) => p.permission === 'network')?.enabled).toBe(true);
		expect(mock.state.permissions.find((p) => p.permission === 'network')?.enabled).toBe(true);
		expect(store.permissions(id)).toBe(permissions);
	});

	it('toggles a hook in place and on the daemon', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.load();
		const id = store.list.value.find(
			(e) => e.script_identifier === 'com.cohesion.cloud.ios-companion'
		)!.id;
		const hooks = store.hooks(id);
		await hooks.load(id);
		expect(hooks.value.find((h) => h.hook === 'task:status')?.enabled).toBe(false);

		await store.setHook(id, 'task:status', true);
		expect(hooks.value.find((h) => h.hook === 'task:status')?.enabled).toBe(true);
		expect(mock.state.hooks.find((h) => h.hook === 'task:status')?.enabled).toBe(true);
		expect(store.hooks(id)).toBe(hooks);
	});

	it('searches only registries granted `registry`, keeping a failing one as an error', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		const sections = await store.search('');
		expect(sections.map((s) => s.extension_id)).toEqual([2, 3]);
		expect(sections[0].results.length).toBeGreaterThan(0);
		expect(sections[1].error).toBeTruthy();

		expect((await store.search('issues'))[0].results.map((r) => r.manifest.name)).toEqual([
			'Linear'
		]);

		await store.setPermission(3, 'registry', false);
		expect((await store.search('')).map((s) => s.extension_id)).toEqual([2]);
	});

	it('installs a previewed script with every permission and hook off', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.load();
		const [result] = (await store.search('linear'))[0].results;
		const preview = await store.preview(result.source_url);
		expect(preview.installed).toBeNull();

		await store.install(preview.script, false);
		const installed = store.list.value.find(
			(e) => e.script_identifier === preview.manifest.identifier
		)!;
		expect(installed.author).toBe(preview.manifest.author);
		const permissions = store.permissions(installed.id);
		await permissions.load(installed.id);
		expect(permissions.value.map((p) => p.permission).sort()).toEqual(
			[...preview.manifest.permissions].sort()
		);
		expect(permissions.value.every((p) => !p.enabled)).toBe(true);
		const hooks = store.hooks(installed.id);
		await hooks.load(installed.id);
		expect(hooks.value.every((h) => !h.enabled)).toBe(true);
	});

	it('keeps existing grants when a reinstall replaces an extension', async () => {
		const mock = createMock();
		const store = createExtensionsStore(mock.send);
		await store.load();
		const results = (await store.search('iphone'))[0].results;
		const preview = await store.preview(results[0].source_url);
		expect(preview.installed?.version).toBe(8);

		await store.install(preview.script, true);
		expect(store.list.value.find((e) => e.id === 1)?.version).toBe(9);
		const permissions = store.permissions(1);
		await permissions.load(1);
		expect(permissions.value.find((p) => p.permission === 'timer')?.enabled).toBe(true);
		// Dropped by the new manifest.
		expect(permissions.value.some((p) => p.permission === 'network')).toBe(false);
	});

	it('sorts permissions and hooks, or rows naming them, in the daemon’s order', () => {
		expect(sortPermissions(['connector', 'network', 'cohesion:read'])).toEqual([
			'cohesion:read',
			'network',
			'connector'
		]);
		const rows = [
			{ hook: 'resource:import' },
			{ hook: 'launch' },
			{ hook: 'task:create' }
		] as const;
		expect(sortHooks(rows, (r) => r.hook).map((r) => r.hook)).toEqual([
			'launch',
			'task:create',
			'resource:import'
		]);
		// A copy: the manifest's own list keeps its order.
		const names = ['timer', 'cohesion:write'] as const;
		sortPermissions(names);
		expect(names).toEqual(['timer', 'cohesion:write']);
	});
});
