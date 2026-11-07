import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createTagsStore } from './tags.svelte';
import { createTasksStore } from './tasks.svelte';

describe('tags store', () => {
	it('keeps a workspace’s tags in order, new ones last, and refuses a name taken', async () => {
		const store = createTagsStore(createMock().send);
		await store.load(1);
		await store.create(1, 'urgent');
		const later = await store.create(1, 'Errands');
		expect(store.list.value.map((t) => t.name)).toEqual(['urgent', 'Errands']);
		await expect(store.create(1, 'URGENT')).rejects.toMatchObject({ field: 'name' });
		await store.rename(1, later.id, 'zz');
		expect(store.list.value.map((t) => t.name)).toEqual(['urgent', 'zz']);
		await store.reorder(1, [later.id, store.list.value[0].id]);
		expect(store.list.value.map((t) => t.name)).toEqual(['zz', 'urgent']);
		await store.sortByName(1);
		expect(store.list.value.map((t) => t.name)).toEqual(['urgent', 'zz']);
		await store.remove(1, later.id);
		expect(store.list.value.map((t) => t.name)).toEqual(['urgent']);
	});

	it('drops a task from a tag’s list once the tag comes off it', async () => {
		const mock = createMock();
		const tags = createTagsStore(mock.send);
		const tasks = createTasksStore(mock.send);
		const tag = await tags.create(1, 'urgent');
		await tasks.open(1, 'all');
		const [first] = tasks.list.value.tasks;
		await tags.setTagged(1, tag.id, first.id, true);

		await tasks.open(1, 'all', undefined, { with: [tag.id], without: [] });
		expect(tasks.list.value.tasks.map((t) => t.id)).toEqual([first.id]);
		expect(tasks.list.value.tasks[0].tag_ids).toEqual([tag.id]);

		tasks.takeTagged(await tags.setTagged(1, tag.id, first.id, false));
		expect(tasks.list.value).toEqual({ tasks: [], total: 0 });
	});
});
