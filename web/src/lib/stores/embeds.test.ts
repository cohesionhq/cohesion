import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
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

	it('lets a missing item come back on any change of its kind', () => {
		expect(touches({ kind: 'documents', id: 4 }, 'cohesion://document/1/9', null)).toBe(true);
		expect(touches({ kind: 'taskFolders', id: 1 }, 'cohesion://task/1/9', null)).toBe(false);
	});
});
