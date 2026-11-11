// Rewind: the versions the daemon keeps of a task, document or journal entry,
// and what was deleted that can still come back. A version is only ever read
// and restored from; restoring keeps a new one, so it can be undone in turn.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	DeletedItem as StoredDeletedItem,
	Invalidation,
	RestoredItem,
	Send,
	Version,
	VersionAuthor,
	VersionSummary,
	VersionTarget
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

/** What a history is of. */
export type RewindItem =
	| { kind: 'task'; workspace_id: number; id: number }
	| { kind: 'document'; workspace_id: number; id: number };

/** A row of a history, without the text it holds. */
export type VersionRow = VersionSummary;

/** A version with its text. */
export type VersionText = Version;

/** A deleted item that can still come back. */
export type DeletedItem = RewindItem & {
	title: string;
	journal_date: string | null;
	author: VersionAuthor;
	author_name: string | null;
	modified: number;
};

/** What restoring gives back: the item as it now is. */
export type Restored = RestoredItem;

/** Who made a version, as a person reads it. */
export function authorLabel(row: {
	author: VersionAuthor;
	author_name: string | null;
}): string | null {
	switch (row.author) {
		case 'user':
			return 'You';
		case 'script':
			return 'Script';
		case 'agent':
		case 'extension':
			return row.author_name;
		case 'unknown':
			return null;
	}
}

function touches(key: Invalidation, kind: RewindItem['kind'], workspace_id: number): boolean {
	return kind === 'task'
		? key.kind === 'taskList' && key.id.workspace_id === workspace_id
		: key.kind === 'documents' && key.id === workspace_id;
}

function target({ kind, id }: RewindItem): VersionTarget {
	return kind === 'task' ? { kind, task_id: id } : { kind, document_id: id };
}

function deletedItem(workspace_id: number, deleted: StoredDeletedItem): DeletedItem {
	const { item, ...rest } = deleted;
	return item.kind === 'task'
		? { kind: 'task', workspace_id, id: item.task_id, ...rest }
		: { kind: 'document', workspace_id, id: item.document_id, ...rest };
}

export class RewindStore {
	/** The history the open panel shows, newest first. */
	readonly versions: Resource<VersionRow[], RewindItem>;
	/** A workspace's deleted items that can still come back, most recently deleted first. */
	readonly deleted: Resource<DeletedItem[], number>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.versions = new Resource({
			fetch: (item): Promise<VersionRow[]> =>
				send('listVersions', { workspace_id: item.workspace_id, item: target(item) }),
			initial: [],
			matches: (key, { kind, workspace_id }) => touches(key, kind, workspace_id)
		});
		this.deleted = new Resource({
			fetch: async (workspace_id) =>
				(await send('listDeletedItems', { workspace_id })).map((deleted) =>
					deletedItem(workspace_id, deleted)
				),
			initial: [],
			matches: (key, workspace_id) =>
				touches(key, 'task', workspace_id) || touches(key, 'document', workspace_id)
		});
	}

	/** One version with its text. */
	read({ workspace_id }: RewindItem, version_id: number): Promise<VersionText> {
		return this.#send('getVersion', { workspace_id, version_id });
	}

	/**
	 * Puts `item` back to a version. The reply is the item, not its history, so
	 * the history is fetched again for the version this kept; the item leaves
	 * the deleted list at once.
	 */
	async restore(item: RewindItem, version_id: number): Promise<Restored> {
		const { kind, workspace_id, id } = item;
		const restored = await this.#send('restoreVersion', { workspace_id, version_id });
		if (this.deleted.key === workspace_id)
			this.deleted.update((items) => items.filter((i) => !(i.kind === kind && i.id === id)));
		await this.versions.refetch();
		return restored;
	}
}

export function createRewindStore(send: Send = defaultSend): RewindStore {
	return new RewindStore(send);
}

/** Rows of one day, under the day's name. */
export interface Day<T> {
	key: string;
	label: string;
	rows: T[];
}

/** Splits rows (newest first) by local day: Today, Yesterday, then each day's short date. */
export function groupByDay<T extends { modified: number }>(
	rows: T[],
	todayStart: number,
	dateLabel: (seconds: number) => string
): Day<T>[] {
	const days: Day<T>[] = [];
	for (const row of rows) {
		const back = Math.max(0, Math.ceil((todayStart - row.modified) / 86_400));
		const key = back === 0 ? 'today' : back === 1 ? 'yesterday' : String(back);
		const last = days.at(-1);
		if (last?.key === key) last.rows.push(row);
		else
			days.push({
				key,
				label: back === 0 ? 'Today' : back === 1 ? 'Yesterday' : dateLabel(row.modified),
				rows: [row]
			});
	}
	return days;
}
