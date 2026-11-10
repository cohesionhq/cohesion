import { describe, expect, it } from 'vitest';
import { ProtocolError } from '$lib/protocol/errors';
import { createMock } from '$lib/protocol/mock';
import type { Send } from '$lib/protocol/types';
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

	it('saves no note when the entry could not be loaded', async () => {
		const mock = createMock();
		const send: Send = (command, ...args) =>
			command === 'getJournal'
				? Promise.reject(new ProtocolError('disconnected', 'Not connected.'))
				: mock.send(command, ...args);
		const store = createJournalStore(send);
		const before = mock.state.documents.find((d) => d.id === 1)?.contents;

		await expect(store.append(1, 'A new thought')).rejects.toThrow('Not connected.');
		expect(mock.state.documents.find((d) => d.id === 1)?.contents).toBe(before);
		expect(store.dirty).toBe(false);
	});
});
