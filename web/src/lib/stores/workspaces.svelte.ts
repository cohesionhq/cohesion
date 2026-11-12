import { send as defaultSend } from '$lib/protocol/api';
import type { Send, Workspace } from '$lib/protocol/types';
import { Resource } from './resource.svelte';

const STORAGE_KEY = 'cohesion.workspace';

function readStored(): number | null {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		return raw === null ? null : Number(raw);
	} catch {
		return null;
	}
}

function writeStored(id: number): void {
	try {
		localStorage.setItem(STORAGE_KEY, String(id));
	} catch {
		// Remembering the workspace is a convenience; ignore blocked storage.
	}
}

export class WorkspacesStore {
	readonly list: Resource<Workspace[]>;
	/** The workspace the current route is in, or null outside one. */
	currentId = $state<number | null>(null);
	current = $derived(this.findCurrent());

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: () => send('listWorkspaces'),
			initial: [],
			matches: (key) => key.kind === 'workspace'
		});
	}

	private findCurrent(): Workspace | null {
		return this.list.value.find((w) => w.id === this.currentId) ?? null;
	}

	load(): Promise<void> {
		return this.list.load();
	}

	/** The last workspace this browser used, if any. */
	stored(): number | null {
		return readStored();
	}

	/** The workspace to land in: `preferred` if it exists, else the stored one, else the first. */
	pick(preferred: number | null = null): number | null {
		const ids = this.list.value.map((w) => w.id);
		for (const id of [preferred, readStored()]) if (id !== null && ids.includes(id)) return id;
		return ids[0] ?? null;
	}

	select(id: number): void {
		this.currentId = id;
		writeStored(id);
	}

	async create(title: string, emoji: string | null = null): Promise<Workspace> {
		const created = await this.#send('createWorkspace', { title, emoji });
		this.list.update((list) => [...list, created]);
		return created;
	}

	async rename(id: number, title: string): Promise<Workspace> {
		const renamed = await this.#send('renameWorkspace', { id, title });
		this.list.update((list) => list.map((w) => (w.id === id ? renamed : w)));
		return renamed;
	}

	/** Sets the emoji shown in place of the initial; blank clears it. */
	async setEmoji(id: number, emoji: string | null): Promise<Workspace> {
		const updated = await this.#send('setWorkspaceEmoji', { id, emoji });
		this.list.update((list) => list.map((w) => (w.id === id ? updated : w)));
		return updated;
	}

	/** Puts the workspaces in the order of `ids`, showing it before the daemon confirms. */
	async reorder(ids: number[]): Promise<void> {
		this.list.update((list) => ids.map((id) => list.find((w) => w.id === id)!));
		try {
			this.list.set(await this.#send('reorderWorkspaces', { ids }));
		} catch (error) {
			void this.list.refetch();
			throw error;
		}
	}

	async remove(id: number): Promise<Workspace> {
		const deleted = await this.#send('deleteWorkspace', { id });
		this.list.update((list) => list.filter((w) => w.id !== id));
		if (this.currentId === id) this.currentId = null;
		return deleted;
	}
}

export function createWorkspacesStore(send: Send = defaultSend): WorkspacesStore {
	return new WorkspacesStore(send);
}

export const workspaces = createWorkspacesStore();
