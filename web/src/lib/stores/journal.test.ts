import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createJournalStore } from './journal.svelte';

describe('journal store', () => {
	it('saves today’s draft and keeps it through a refetch while dirty', async () => {
		const mock = createMock();
		const store = createJournalStore(mock.send);
		await store.open(1);
		expect(store.draft).toContain('consolidated repos');
		expect(store.dirty).toBe(false);

		store.edit('Rewritten');
		await store.today.refresh();
		expect(store.draft).toBe('Rewritten');

		await store.flush();
		expect(store.dirty).toBe(false);
		expect(store.savedAt).not.toBeNull();
		expect(mock.state.documents.find((d) => d.id === 1)?.contents).toBe('Rewritten');
	});

	it('flushes the previous workspace before switching', async () => {
		const mock = createMock();
		const store = createJournalStore(mock.send);
		await store.open(1);
		store.edit('Pending');
		await store.open(2);
		expect(mock.state.documents.find((d) => d.id === 1)?.contents).toBe('Pending');
		expect(store.draft).toBe('');
	});

	it('creates a day nothing was written on only when it is saved', async () => {
		const mock = createMock();
		const store = createJournalStore(mock.send);
		const journals = () => mock.state.documents.filter((d) => d.workspace_id === 2).length;
		const before = journals();
		await store.open(2);
		expect(store.today.value).toBeNull();
		expect(journals()).toBe(before);

		store.edit('First line');
		await store.flush();
		expect(journals()).toBe(before + 1);
		expect(store.today.value?.contents).toBe('First line');
	});

	it('appends a note to what is already written and saves it', async () => {
		const mock = createMock();
		const store = createJournalStore(mock.send);
		await store.open(1);
		const before = store.draft;
		await store.append(1, ' A new thought ');
		expect(store.dirty).toBe(false);
		expect(mock.state.documents.find((d) => d.id === 1)?.contents).toBe(
			`${before.trimEnd()}\n\nA new thought`
		);
	});
});
