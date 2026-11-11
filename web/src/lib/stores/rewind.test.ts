import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { authorLabel, createRewindStore, groupByDay, type RewindItem } from './rewind.svelte';

describe('rewind store', () => {
	it('restoring keeps a new version, so it can be undone', async () => {
		const { send } = createMock();
		const store = createRewindStore(send);
		const doc = await send('createDocument', { workspace_id: 1, title: 'Plan', contents: 'one' });
		const item: RewindItem = { kind: 'document', workspace_id: 1, id: doc.id };
		await store.versions.load(item);
		expect(store.versions.value).toHaveLength(1);
		const first = store.versions.value[0]!.id;

		await send('deleteDocument', { workspace_id: 1, document_id: doc.id });
		await store.versions.refetch();
		expect(store.versions.value.map((v) => v.change)).toEqual(['deleted', 'created']);
		expect(store.versions.value.every((v) => !v.current)).toBe(true);

		const restored = await store.restore(item, first);
		expect(restored.kind === 'document' && restored.document.deleted).toBe(false);
		expect(store.versions.value.map((v) => v.change)).toEqual(['restored', 'deleted', 'created']);
		expect(store.versions.value[0]!.current).toBe(true);
	});

	it('lists what was deleted, newest first, until it is restored', async () => {
		const { send } = createMock();
		const store = createRewindStore(send);
		const doc = await send('createDocument', { workspace_id: 1, title: 'Gone', contents: '' });
		await send('deleteDocument', { workspace_id: 1, document_id: doc.id });
		await store.deleted.load(1);
		const item = store.deleted.value.find((i) => i.kind === 'document' && i.id === doc.id)!;
		expect(item.title).toBe('Gone');

		await store.versions.load(item);
		await store.restore(item, store.versions.value[0]!.id);
		expect(store.deleted.value.some((i) => i.kind === 'document' && i.id === doc.id)).toBe(false);
	});

	it('names who made a version', () => {
		expect(authorLabel({ author: 'user', author_name: null })).toBe('You');
		expect(authorLabel({ author: 'agent', author_name: 'Ada' })).toBe('Ada');
		expect(authorLabel({ author: 'extension', author_name: 'Linear' })).toBe('Linear');
		expect(authorLabel({ author: 'script', author_name: null })).toBe('Script');
		expect(authorLabel({ author: 'unknown', author_name: null })).toBe(null);
	});
});

describe('groupByDay', () => {
	it('names today and yesterday, and dates the days before', () => {
		const today = 1_000_000;
		const rows = [today + 60, today - 60, today - 86_400 - 60, today - 3 * 86_400].map(
			(modified) => ({ modified })
		);
		const days = groupByDay(rows, today, (s) => `day ${s}`);
		expect(days.map((d) => [d.label, d.rows.length])).toEqual([
			['Today', 1],
			['Yesterday', 1],
			[`day ${today - 86_400 - 60}`, 1],
			[`day ${today - 3 * 86_400}`, 1]
		]);
	});
});
