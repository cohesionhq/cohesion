// Resources: read-only copies of records imported from connected services,
// per workspace, and the one open resource.

import type {
	Component,
	Invalidation,
	Resource as ImportedResource,
	ResourceRefreshSummary,
	ResourceSort,
	Send,
	WorkspaceResourceKind
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export interface ResourceKey {
	workspace_id: number;
	resource_id: number;
}

/** The order a workspace's resources list in. */
export interface ResourceOrder {
	sort: ResourceSort;
	descending: boolean;
}

export interface ResourceListKey extends ResourceOrder {
	workspace_id: number;
}

/** Most recently updated first. */
export const DEFAULT_RESOURCE_ORDER: ResourceOrder = { sort: 'lastUpdated', descending: true };

const ORDERS_KEY = 'cohesion.resourceOrders';

function storedOrders(): Record<string, Partial<ResourceOrder>> {
	try {
		return JSON.parse(localStorage.getItem(ORDERS_KEY) ?? '{}') as Record<
			string,
			Partial<ResourceOrder>
		>;
	} catch {
		return {};
	}
}

/**
 * The order kind `kind_id`'s page last listed in, if it is one of `sorts`
 * (the keys its connector still declares); most recently updated first at
 * first.
 */
export function rememberedOrder(kind_id: number, sorts: ResourceSort[]): ResourceOrder {
	const stored = storedOrders()[kind_id];
	if (stored && sorts.includes(stored.sort as ResourceSort)) {
		return { sort: stored.sort as ResourceSort, descending: stored.descending === true };
	}
	return DEFAULT_RESOURCE_ORDER;
}

export function rememberOrder(kind_id: number, order: ResourceOrder): void {
	try {
		localStorage.setItem(ORDERS_KEY, JSON.stringify({ ...storedOrders(), [kind_id]: order }));
	} catch {
		// Remembering the order is a convenience; ignore blocked storage.
	}
}

const inWorkspace = (key: Invalidation, workspace_id: number) =>
	key.kind === 'resources' && key.id === workspace_id;

export class ResourcesStore {
	readonly list: Resource<ImportedResource[], ResourceListKey>;
	readonly current: Resource<ImportedResource | null, ResourceKey>;
	/**
	 * The interface the open resource's connector shows it with, or `null`
	 * when it has none and its contents show instead. Running the connector's
	 * script is costly, so it follows no invalidation: it runs again only when
	 * the open resource changes (see `#viewedAs`), and the last interface keeps
	 * showing meanwhile.
	 */
	readonly view: Resource<Component | null, ResourceKey>;
	/** The kinds shown in a workspace; every other kind is hidden there. */
	readonly shownKinds: Resource<WorkspaceResourceKind[], number>;

	#send: Send;
	/** What the open resource's `view` was drawn from, to tell when it changes. */
	#viewedAs: { id: number; shown: string } | null = null;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: (key) => send('listResources', key),
			initial: [],
			matches: (key, { workspace_id }) => inWorkspace(key, workspace_id)
		});
		this.current = new Resource({
			fetch: ({ workspace_id, resource_id }) => send('getResource', { workspace_id, resource_id }),
			initial: null,
			matches: (key, { workspace_id }) => inWorkspace(key, workspace_id),
			onFetched: (resource) => this.#drawn(resource)
		});
		this.view = new Resource({
			fetch: ({ workspace_id, resource_id }) =>
				send('renderResource', { workspace_id, resource_id }),
			initial: null,
			// Only `#drawn` runs it again; a focus or reconnect refresh doesn't.
			canRefetch: () => false
		});
		this.shownKinds = new Resource({
			fetch: (workspace_id) => send('listWorkspaceResourceKinds', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'resourceKinds' && key.id === workspace_id
		});
	}

	/** Draws the open resource again when what its connector draws from changed. */
	#drawn(resource: ImportedResource | null): void {
		if (!resource) return;
		const { id, kind_id, title, contents, last_updated } = resource;
		const shown = JSON.stringify([kind_id, title, contents, last_updated]);
		const changed = this.#viewedAs?.id === id && this.#viewedAs.shown !== shown;
		this.#viewedAs = { id, shown };
		if (changed) void this.view.refetch();
	}

	/** Shows kind `kind_id` in `workspace_id`, or hides it there. */
	async setShown(workspace_id: number, kind_id: number, shown: boolean): Promise<void> {
		const now = await this.#send('setResourceKindShown', { workspace_id, kind_id, shown });
		if (this.shownKinds.key === workspace_id) this.shownKinds.set(now);
	}

	/** Puts the shown kinds in the order of `kind_ids`, showing it before the daemon confirms. */
	async reorderShown(workspace_id: number, kind_ids: number[]): Promise<void> {
		this.shownKinds.update((list) => kind_ids.map((id) => list.find((k) => k.kind_id === id)!));
		try {
			this.shownKinds.set(
				await this.#send('reorderWorkspaceResourceKinds', { workspace_id, kind_ids })
			);
		} catch (error) {
			void this.shownKinds.refetch();
			throw error;
		}
	}

	load(
		workspace_id: number,
		{ sort, descending }: ResourceOrder = DEFAULT_RESOURCE_ORDER
	): Promise<void> {
		const key = this.list.key;
		// Reuse the loaded key: a new object never matches it.
		const same =
			key?.workspace_id === workspace_id && key.sort === sort && key.descending === descending;
		return this.list.load(same ? key : { workspace_id, sort, descending });
	}

	open(workspace_id: number, resource_id: number): Promise<void> {
		const key = this.current.key;
		// Reuse the loaded key: a new object never matches it.
		const same = key?.workspace_id === workspace_id && key.resource_id === resource_id;
		const opened = same ? key : { workspace_id, resource_id };
		if (!same) this.#viewedAs = null;
		return Promise.all([this.current.load(opened), this.view.load(opened)]).then(() => undefined);
	}

	/** Stops refetching the open resource until it opens again. */
	close(): void {
		this.current.release();
		this.view.release();
	}

	/** Removes a resource from its workspace. */
	async remove(workspace_id: number, resource_id: number): Promise<ImportedResource> {
		const resource = await this.#send('deleteResource', { workspace_id, resource_id });
		if (this.current.value?.id === resource.id) {
			this.current.set(null);
			this.view.set(null);
		}
		if (this.list.key?.workspace_id === workspace_id)
			this.list.update((list) => list.filter((a) => a.id !== resource.id));
		return resource;
	}

	/** Takes a resource just imported into the list of its workspace, if that is loaded. */
	imported(resource: ImportedResource): void {
		if (this.list.key?.workspace_id === resource.workspace_id)
			this.list.update((list) => [resource, ...list.filter((a) => a.id !== resource.id)]);
	}

	/**
	 * Asks each connector for the latest copy of every resource in
	 * `workspace_id`. Connectors are scripts, so this goes over HTTP; the
	 * resources that changed arrive by invalidation.
	 */
	refresh(workspace_id: number): Promise<ResourceRefreshSummary> {
		return this.#send('refreshResources', { workspace_id });
	}
}

export function createResourcesStore(send: Send = defaultSend): ResourcesStore {
	return new ResourcesStore(send);
}
