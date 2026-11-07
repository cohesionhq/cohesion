// Tags: the names a workspace puts on its tasks.

import { send as defaultSend } from '$lib/protocol/api';
import type { Send, Tag, Task } from '$lib/protocol/types';
import { Resource } from './resource.svelte';

const byName = (a: Tag, b: Tag) =>
	a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }) || a.id - b.id;

export class TagsStore {
	/** The workspace's tags, in their order. */
	readonly list: Resource<Tag[], number>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (workspace_id) => send('listTags', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'tags' && key.id === workspace_id
		});
	}

	load(workspace_id: number): Promise<void> {
		return this.list.load(workspace_id);
	}

	async create(workspace_id: number, name: string): Promise<Tag> {
		const tag = await this.#send('createTag', { workspace_id, name });
		this.list.update((list) => [...list, tag]);
		return tag;
	}

	async rename(workspace_id: number, tag_id: number, name: string): Promise<Tag> {
		const tag = await this.#send('renameTag', { workspace_id, tag_id, name });
		this.list.update((list) => list.map((t) => (t.id === tag_id ? tag : t)));
		return tag;
	}

	/** Puts the tags in the order of `tag_ids`, showing it before the daemon confirms. */
	async reorder(workspace_id: number, tag_ids: number[]): Promise<void> {
		this.list.update((list) => tag_ids.map((id) => list.find((t) => t.id === id)!));
		try {
			this.list.set(await this.#send('reorderTags', { workspace_id, tag_ids }));
		} catch (error) {
			void this.list.refetch();
			throw error;
		}
	}

	/** Puts the tags in alphabetical order. */
	sortByName(workspace_id: number): Promise<void> {
		return this.reorder(
			workspace_id,
			this.list.value.toSorted(byName).map((t) => t.id)
		);
	}

	/** Deletes a tag; the daemon takes it off every task, and the lists refetch. */
	async remove(workspace_id: number, tag_id: number): Promise<Tag> {
		const tag = await this.#send('deleteTag', { workspace_id, tag_id });
		this.list.update((list) => list.filter((t) => t.id !== tag_id));
		return tag;
	}

	/** Puts a tag on a task or takes it off; replies with the task. */
	setTagged(workspace_id: number, tag_id: number, task_id: number, tagged: boolean): Promise<Task> {
		return this.#send('setTagged', { workspace_id, tag_id, task_id, tagged });
	}
}

export function createTagsStore(send: Send = defaultSend): TagsStore {
	return new TagsStore(send);
}
