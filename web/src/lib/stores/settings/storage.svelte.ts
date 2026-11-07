// Settings → Storage: how long Cohesion keeps model threads, actor sessions,
// Rewind's versions and deleted items, how many memories each scope keeps, and how large
// the database is. The settings refetch on the `storage` key; the size is
// read when the section opens.

import { send as defaultSend } from '$lib/protocol/api';
import type {
	DatabaseBackup,
	DatabaseSize,
	Send,
	StorageSettings,
	UpdateStorageSettingsRequest
} from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

export class StorageStore {
	readonly settings: Resource<StorageSettings | null>;
	readonly size: Resource<DatabaseSize | null>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.settings = new Resource({
			fetch: () => send('getStorageSettings'),
			initial: null,
			matches: (key) => key.kind === 'storage'
		});
		this.size = new Resource({
			fetch: () => send('getDatabaseSize'),
			initial: null
		});
	}

	async load(): Promise<void> {
		await Promise.all([this.settings.load(), this.size.load()]);
	}

	release(): void {
		this.settings.release();
		this.size.release();
	}

	/** Writes a backup on the daemon's computer. */
	backUp(): Promise<DatabaseBackup> {
		return this.#send('backupDatabase');
	}

	/** Changes some of the settings, keeping the rest. */
	async update(change: Partial<UpdateStorageSettingsRequest>): Promise<StorageSettings> {
		const current = this.settings.value;
		if (!current) throw new Error('The storage settings haven’t loaded.');
		const updated = await this.#send('updateStorageSettings', {
			thread_retention_days: current.thread_retention_days,
			actor_session_retention_days: current.actor_session_retention_days,
			rewind_retention_days: current.rewind_retention_days,
			deleted_item_retention_days: current.deleted_item_retention_days,
			memory_limit: current.memory_limit,
			...change
		});
		this.settings.update(() => updated);
		return updated;
	}
}

export function createStorageStore(send: Send = defaultSend): StorageStore {
	return new StorageStore(send);
}

export const storage = createStorageStore();
