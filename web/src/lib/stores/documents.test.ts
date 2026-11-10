import { afterEach, describe, expect, it, vi } from 'vitest';
import { localMidnight } from '$lib/format/dates';
import { createMock } from '$lib/protocol/mock';
import type { Document } from '$lib/protocol/types';
import {
	createDocumentsStore,
	excerpt,
	groupByMonth,
	groupByRecency,
	neighbours
} from './documents.svelte';

const doc = (id: number, fields: Partial<Document>): Document => ({
	id,
	workspace_id: 1,
	document_type: 'normal',
	title: `Doc ${id}`,
	contents: '',
	modified: 0,
	journal_date: null,
	deleted: false,
	folder_id: null,
	...fields
});

describe('document helpers', () => {
	it('excerpts the first non-empty line without Markdown markers', () => {
		expect(excerpt('\n\n# Today\n\nMore')).toBe('Today');
		expect(excerpt('  - Morning walk')).toBe('Morning walk');
		expect(excerpt('# Plans\n\nFirst step', 90, 'Plans')).toBe('First step');
		expect(excerpt('')).toBe('');
		const long = 'word '.repeat(40);
		const cut = excerpt(long);
		expect(cut.length).toBeLessThanOrEqual(90);
		expect(cut.endsWith('…')).toBe(true);
	});

	it('groups journals by month, newest first', () => {
		const groups = groupByMonth([
			doc(1, { journal_date: '2026-09-30' }),
			doc(2, { journal_date: '2026-10-12' }),
			doc(3, { journal_date: '2026-10-02' })
		]);
		expect(groups.map((g) => g.key)).toEqual(['2026-10', '2026-09']);
		expect(groups[0].items.map((d) => d.id)).toEqual([2, 3]);
	});

	it('groups documents into today, this week and earlier', () => {
		const today = localMidnight(0, new Date(2026, 9, 12));
		const groups = groupByRecency(
			[
				doc(1, { modified: today - 30 * 86_400 }),
				doc(2, { modified: today + 3600 }),
				doc(3, { modified: today - 2 * 86_400 })
			],
			today
		);
		expect(groups.map((g) => [g.label, g.items.map((d) => d.id)])).toEqual([
			['Today', [2]],
			['This week', [3]],
			['Earlier', [1]]
		]);
	});

	it('finds the older and newer neighbours of an entry', () => {
		const days = [3, 1, 2].map((n) => doc(n, { journal_date: `2026-10-0${n}` }));
		expect(neighbours(days, 2)).toMatchObject({ prev: { id: 1 }, next: { id: 3 } });
		expect(neighbours(days, 3)).toMatchObject({ prev: { id: 2 }, next: null });
	});
});

describe('documents store', () => {
	afterEach(() => vi.useRealTimers());

	it('lists normal documents and the journal archive separately', async () => {
		const store = createDocumentsStore(createMock().send);
		await Promise.all([store.list.load(1), store.archive.load(1)]);
		expect(store.list.value.map((d) => d.title).slice(0, 6)).toEqual([
			'Full-text search syntax and errors',
			'Extensions Registry',
			'JS script engine pool sizes',
			'Rewind: storage and lifecycle',
			'Launch plan',
			'Winter reading'
		]);
		// The rows of the two tables are documents too, listed like any other.
		expect(store.list.value).toHaveLength(17);
		expect(store.archive.value.map((d) => d.id)).toEqual([1, 2, 3, 4]);
	});

	it('creates, renames and deletes from replies', async () => {
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await store.list.load(1);
		const created = await store.create(1);
		expect(created.title).toBe('Untitled');
		expect(store.list.value[0]).toEqual(created);

		await store.open(1, created.id);
		await store.rename('Packing list');
		expect(store.current.value?.title).toBe('Packing list');
		expect(store.list.value[0].title).toBe('Packing list');

		await expect(store.rename('')).rejects.toMatchObject({
			code: 'validationFailed',
			message: 'Title can’t be empty.',
			field: undefined
		});
		expect(store.current.value?.title).toBe('Packing list');

		await store.remove();
		expect(store.current.value).toBeNull();
		expect(store.list.value.some((d) => d.id === created.id)).toBe(false);
		expect(mock.state.documents.find((d) => d.id === created.id)?.deleted).toBe(true);
	});

	it('files documents into folders, nests folders and deletes them into the Inbox', async () => {
		const store = createDocumentsStore(createMock().send);
		await Promise.all([store.list.load(1), store.folders.load(1)]);
		expect(store.folders.value.map((f) => f.title)).toEqual([
			'Design',
			'Search',
			'Research',
			'Reading list',
			'Kitchen renovation'
		]);

		const drafts = await store.createFolder(1, 'Drafts');
		const inner = await store.createFolder(1, 'Inner', drafts.id);
		expect(store.folders.value.at(-1)).toEqual(inner);
		const created = await store.create(1, 'Plan', '', inner.id);
		expect(created.folder_id).toBe(inner.id);

		const registry = store.list.value.find((d) => d.title === 'Extensions Registry')!;
		await store.move(registry, drafts.id);
		expect(store.list.value.find((d) => d.id === registry.id)?.folder_id).toBe(drafts.id);

		await store.renameFolder(1, drafts.id, 'Notes');
		await store.moveFolder(1, drafts.id, null, 1);
		const top = store.folders.value
			.filter((f) => f.parent_id === null)
			.sort((a, b) => a.sort_order - b.sort_order);
		expect(top.map((f) => f.title)).toEqual([
			'Notes',
			'Design',
			'Research',
			'Reading list',
			'Kitchen renovation'
		]);
		await expect(store.moveFolder(1, drafts.id, inner.id)).rejects.toMatchObject({
			code: 'validationFailed'
		});

		await store.deleteFolder(1, drafts.id);
		expect(store.folders.value.map((f) => f.title)).toEqual([
			'Design',
			'Search',
			'Research',
			'Reading list',
			'Kitchen renovation'
		]);
		for (const id of [created.id, registry.id])
			expect(store.list.value.find((d) => d.id === id)?.folder_id).toBeNull();
	});

	it('autosaves the draft and skips refetches while dirty', async () => {
		vi.useFakeTimers();
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await store.open(1, 5);
		expect(store.text.draft).toContain('FTS5');

		store.text.edit('Changed');
		expect(store.text.dirty).toBe(true);
		mock.state.documents.find((d) => d.id === 5)!.contents = 'From elsewhere';
		await store.current.refresh();
		expect(store.text.draft).toBe('Changed');

		await vi.advanceTimersByTimeAsync(800);
		expect(store.text.dirty).toBe(false);
		expect(mock.state.documents.find((d) => d.id === 5)?.contents).toBe('Changed');
	});

	it('flushes the previous document before opening the next', async () => {
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await store.open(1, 5);
		store.text.edit('Unsaved');
		await store.open(1, 6);
		expect(mock.state.documents.find((d) => d.id === 5)?.contents).toBe('Unsaved');
		expect(store.text.draft).toContain('registryResults');
	});

	it('edits a past journal entry and updates the archive', async () => {
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await store.archive.load(1);
		await store.open(1, 2);
		store.text.edit('Rewritten');
		await store.text.flush();
		expect(store.archive.value.find((d) => d.id === 2)?.contents).toBe('Rewritten');
	});

	it('reports a missing document as notFound', async () => {
		const store = createDocumentsStore(createMock().send);
		await store.open(1, 999);
		expect(store.current.error?.code).toBe('notFound');
	});

	it('asks before a move out of a table only when the document holds values set by hand', async () => {
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await Promise.all([store.list.load(1), store.folders.load(1)]);
		// Folder 4, the reading list, is a table; 1, Design, isn't. Stars (7) is a formula.
		const rows = await mock.send('listTableRows', { workspace_id: 1, folder_id: 4, filters: [] });
		const withValues = store.list.value.find((d) => d.id === rows[0].document_id)!;
		expect(await store.moveLosesValues(withValues, 1)).toBe(true);
		expect(await store.moveLosesValues(withValues, null)).toBe(true);
		expect(await store.moveLosesValues(withValues, 5)).toBe(true);
		expect(await store.moveLosesValues(withValues, 4)).toBe(false);

		// A plain document moving into a table asks nothing and arrives with no values.
		const plain = store.list.value.find((d) => d.folder_id === null)!;
		expect(await store.moveLosesValues(plain, 4)).toBe(false);
		const moved = await store.move(plain, 4);
		const row = await mock.send('getTableRow', {
			workspace_id: 1,
			folder_id: 4,
			document_id: moved.id
		});
		// No values; the formula on Rating fails on an empty one.
		expect(row.cells).toEqual([]);
		expect(await store.moveLosesValues(moved, null)).toBe(false);

		// Leaving the table deletes the values; coming back brings none.
		await store.move(withValues, null);
		await store.move(withValues, 4);
		const back = await mock.send('getTableRow', {
			workspace_id: 1,
			folder_id: 4,
			document_id: withValues.id
		});
		expect(back.cells.filter((c) => c.property_id !== 7)).toEqual([]);
	});

	it('deleting a table folder sends its documents to the Inbox without their values', async () => {
		const mock = createMock();
		const store = createDocumentsStore(mock.send);
		await Promise.all([store.list.load(1), store.folders.load(1)]);
		await store.deleteFolder(1, 4);
		expect(store.list.value.filter((d) => d.folder_id === 4)).toEqual([]);
		expect(mock.state.folderProperties.some((p) => p.folder_id === 4)).toBe(false);
		const dune = store.list.value.find((d) => d.title === 'Dune')!;
		expect(dune.folder_id).toBeNull();
		expect(mock.state.documentValues.some((v) => v.document_id === dune.id)).toBe(false);
	});
});
