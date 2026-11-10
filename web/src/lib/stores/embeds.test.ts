import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import type { Embed } from '$lib/protocol/types';
import { createEmbedsStore, touches } from './embeds.svelte';
import { refreshInvalidated } from './resource.svelte';

describe('embeds store', () => {
	it('resolves a link once for every card showing it, and asks again when its item changes', async () => {
		const mock = createMock();
		const store = createEmbedsStore(mock.send);
		const task = mock.state.tasks.find((t) => !t.deleted)!;
		const href = `cohesion://task/${task.workspace_id}/${task.id}`;

		const first = store.show(href);
		const second = store.show(href);
		expect(second.embed).toBe(first.embed);
		await first.embed.load(href);
		expect(first.embed.value?.title).toBe(task.title);

		task.title = 'Renamed';
		await refreshInvalidated([
			{ kind: 'taskList', id: { workspace_id: task.workspace_id, folder_id: null } }
		]);
		expect(first.embed.value?.title).toBe('Renamed');
		first.release();
		second.release();
	});

	it('shows nothing for an item that is gone', async () => {
		const store = createEmbedsStore(createMock().send);
		const { embed, release } = store.show('cohesion://agent/999');
		await embed.load('cohesion://agent/999');
		expect(embed.value).toBeNull();
		expect(embed.error).toBeNull();
		release();
	});

	it('asks again about a database row when its database changes', () => {
		const row: Embed = {
			link: 'cohesion://database-row/1/5/21',
			kind: 'databaseRow',
			workspace_id: 1,
			title: 'Dune',
			excerpt: '',
			item: {
				kind: 'databaseRow',
				row: { id: 21, database_id: 5, document_id: null, cells: [] },
				database_title: 'Books',
				properties: []
			}
		};
		const link = 'cohesion://document/1/9';
		expect(touches({ kind: 'database', id: 5 }, link, row)).toBe(true);
		expect(touches({ kind: 'database', id: 6 }, link, row)).toBe(false);
		expect(touches({ kind: 'documents', id: 1 }, link, row)).toBe(false);
	});

	it('lets a missing item come back on any change of its kind', () => {
		expect(touches({ kind: 'documents', id: 4 }, 'cohesion://document/1/9', null)).toBe(true);
		expect(touches({ kind: 'taskFolders', id: 1 }, 'cohesion://task/1/9', null)).toBe(false);
	});
});
