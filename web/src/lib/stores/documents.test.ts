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
		expect(store.list.value.map((d) => d.title)).toEqual([
			'Full-text search syntax and errors',
			'Extensions Registry',
			'JS script engine pool sizes',
			'Rewind: storage and lifecycle'
		]);
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
});
