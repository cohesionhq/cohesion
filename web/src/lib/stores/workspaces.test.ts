import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createWorkspacesStore } from './workspaces.svelte';

describe('workspaces store', () => {
	it('loads, then applies mutation replies without refetching', async () => {
		const mock = createMock();
		const store = createWorkspacesStore(mock.send);
		await store.load();
		expect(store.list.loaded).toBe(true);
		expect(store.list.value.map((w) => w.title)).toEqual(['Personal', 'Work']);

		const created = await store.create('Garden');
		expect(store.list.value.at(-1)).toEqual(created);

		await store.rename(created.id, 'Allotment');
		expect(store.list.value.find((w) => w.id === created.id)?.title).toBe('Allotment');

		await store.setEmoji(created.id, ' 🌱 ');
		expect(store.list.value.find((w) => w.id === created.id)?.emoji).toBe('🌱');
		await store.setEmoji(created.id, '');
		expect(store.list.value.find((w) => w.id === created.id)?.emoji).toBeNull();

		store.select(created.id);
		expect(store.current?.title).toBe('Allotment');

		await store.remove(created.id);
		expect(store.list.value.some((w) => w.id === created.id)).toBe(false);
		expect(store.currentId).toBeNull();
	});

	it('a second load joins the fetch in flight', async () => {
		const store = createWorkspacesStore(createMock().send);
		const first = store.load();
		await store.load();
		expect(store.list.value).toHaveLength(2);
		await first;
	});

	it('picks the preferred workspace, else the first', async () => {
		const store = createWorkspacesStore(createMock().send);
		await store.load();
		expect(store.pick(2)).toBe(2);
		expect(store.pick(99)).toBe(1);
	});

	it('refetch picks up changes made elsewhere', async () => {
		const mock = createMock();
		const store = createWorkspacesStore(mock.send);
		await store.load();
		await mock.send('createWorkspace', { title: 'From the CLI' });
		expect(store.list.value).toHaveLength(2);
		await store.list.refetch();
		expect(store.list.value.map((w) => w.title)).toContain('From the CLI');
	});

	it('rejects a failed mutation and leaves the list alone', async () => {
		const store = createWorkspacesStore(createMock().send);
		await store.load();
		await expect(store.create('')).rejects.toMatchObject({
			code: 'validationFailed',
			message: 'Title can’t be empty.',
			field: undefined
		});
		expect(store.list.value).toHaveLength(2);
	});
});
