import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createBacklinksStore, touches } from './backlinks.svelte';
import { refreshInvalidated } from './resource.svelte';

describe('backlinks store', () => {
	it('lists the documents linking to an item once for every panel, and asks again when they change', async () => {
		const mock = createMock();
		const store = createBacklinksStore(mock.send);
		const href = 'cohesion://task/1/4';

		const first = store.show(href);
		const second = store.show(href);
		expect(second.backlinks).toBe(first.backlinks);
		await first.backlinks.load(href);
		expect(first.backlinks.value.map((embed) => embed.title)).toEqual(['Launch plan']);

		const plan = mock.state.documents.find((d) => d.title === 'Launch plan')!;
		plan.contents = 'Nothing linked.';
		await refreshInvalidated([{ kind: 'backlinks', id: { kind: 'task', id: 4 } }]);
		expect(first.backlinks.value).toEqual([]);
		first.release();
		second.release();
	});

	it('leaves out a document linking to itself', async () => {
		const mock = createMock();
		const plan = mock.state.documents.find((d) => d.title === 'Launch plan')!;
		plan.contents = `[Me](cohesion://document/${plan.workspace_id}/${plan.id})`;
		const href = `cohesion://document/${plan.workspace_id}/${plan.id}`;
		const { backlinks, release } = createBacklinksStore(mock.send).show(href);
		await backlinks.load(href);
		expect(backlinks.value).toEqual([]);
		release();
	});

	it('asks again on its own item’s backlinks or a workspace going, and nothing else', () => {
		const href = 'cohesion://actor-session/2';
		expect(touches({ kind: 'backlinks', id: { kind: 'actorSession', id: 2 } }, href)).toBe(true);
		expect(touches({ kind: 'backlinks', id: { kind: 'task', id: 2 } }, href)).toBe(false);
		expect(touches({ kind: 'workspace', id: 1 }, href)).toBe(true);
		expect(touches({ kind: 'documents', id: 1 }, href)).toBe(false);
	});
});
