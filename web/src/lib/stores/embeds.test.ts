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

	it('asks again about a folder when its table or the folders change', () => {
		const folder: Embed = {
			link: 'cohesion://document-folder/1/5',
			kind: 'documentFolder',
			workspace_id: 1,
			title: 'Books',
			excerpt: '',
			item: {
				kind: 'documentFolder',
				folder: {
					id: 5,
					workspace_id: 1,
					parent_id: null,
					title: 'Books',
					sort_order: 1,
					deleted: false,
					is_table: true
				},
				properties: [],
				rows: [],
				row_count: 0
			}
		};
		const link = 'cohesion://document-folder/1/5';
		expect(touches({ kind: 'documentTable', id: 5 }, link, folder)).toBe(true);
		expect(touches({ kind: 'documentTable', id: 6 }, link, folder)).toBe(false);
		expect(touches({ kind: 'documentFolders', id: 1 }, link, folder)).toBe(true);
		expect(touches({ kind: 'documents', id: 1 }, link, folder)).toBe(false);
	});

	it('resolves a table with its first rows, a plain folder with its titles, and a row as a document', async () => {
		const mock = createMock();
		const table = await mock.send('resolveLink', { link: 'cohesion://document-folder/1/4' });
		expect(table.title).toBe('Reading list');
		expect(table.item.kind === 'documentFolder' && table.item.rows).toHaveLength(5);
		expect(table.item.kind === 'documentFolder' && table.item.row_count).toBe(6);
		expect(table.item.kind === 'documentFolder' && table.item.folder.is_table).toBe(true);
		const plain = await mock.send('resolveLink', { link: 'cohesion://document-folder/1/2' });
		expect(plain.item.kind === 'documentFolder' && plain.item.properties).toEqual([]);
		expect(plain.item.kind === 'documentFolder' && plain.item.folder.is_table).toBe(false);
		const first = table.item.kind === 'documentFolder' ? table.item.rows[0] : null;
		const row = await mock.send('resolveLink', {
			link: `cohesion://document/1/${first?.document_id}`
		});
		expect(row).toMatchObject({ kind: 'document', title: 'Dune' });
		expect(row.excerpt).toMatch(/train/);
		await expect(
			mock.send('resolveLink', { link: 'cohesion://document-folder/1/99' })
		).rejects.toMatchObject({ code: 'notFound' });
	});

	it('lets a missing item come back on any change of its kind', () => {
		expect(touches({ kind: 'documents', id: 4 }, 'cohesion://document/1/9', null)).toBe(true);
		expect(touches({ kind: 'taskFolders', id: 1 }, 'cohesion://task/1/9', null)).toBe(false);
		expect(
			touches({ kind: 'documentFolders', id: 1 }, 'cohesion://document-folder/1/2', null)
		).toBe(true);
	});
});
