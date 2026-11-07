// Extensions: the installed ones with their hooks and permissions, and what
// registries offer to install.

import type {
	ExtensionHook,
	ExtensionPermission,
	ExtensionPermissionKind,
	ExtensionPreview,
	ExtensionSummary,
	HookKind,
	ExtensionUpdate,
	Invalidation,
	RegistrySection,
	Send
} from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

/** The daemon's update check finishing, and an install or uninstall changing what still applies. */
const updatesChanged = (key: Invalidation) => key.kind === 'extensionUpdates';

/** What makes a hook fire, as a short title: "When a task is created". */
export function hookTitle(hook: HookKind): string {
	return `When ${vocabulary.hook(hook).extension_clause}`;
}

/** Sorts permissions, or rows naming one, into the order the vocabulary lists them in. */
export const sortPermissions = canonicalSort(() => vocabulary.value.permissions);
/** Sorts hooks, or rows naming one, into the order the vocabulary lists them in. */
export const sortHooks = canonicalSort(() => vocabulary.value.hooks);

function canonicalSort<K extends string>(kinds: () => readonly { kind: K }[]) {
	function sort(names: readonly K[]): K[];
	function sort<T>(rows: readonly T[], name: (row: T) => K): T[];
	function sort<T>(items: readonly T[], name = (item: T): string => String(item)): T[] {
		const rank = new Map<string, number>(kinds().map(({ kind }, i) => [kind, i]));
		return [...items].sort((a, b) => rank.get(name(a))! - rank.get(name(b))!);
	}
	return sort;
}

/**
 * One kind of grant row (permissions or hooks) for each extension that has
 * been expanded: a resource per extension, created on first use.
 */
class Grants<Row extends { enabled: boolean }, Name extends string> {
	// A plain map: each resource is reactive itself.
	readonly #resources = new Map<number, Resource<Row[], number>>();
	readonly #fetch: (extension_id: number) => Promise<Row[]>;
	readonly #name: (row: Row) => Name;
	readonly #row: (extension_id: number, name: Name, enabled: boolean) => Row;

	constructor(
		fetch: (extension_id: number) => Promise<Row[]>,
		name: (row: Row) => Name,
		row: (extension_id: number, name: Name, enabled: boolean) => Row
	) {
		this.#fetch = fetch;
		this.#name = name;
		this.#row = row;
	}

	get(extension_id: number): Resource<Row[], number> {
		let resource = this.#resources.get(extension_id);
		if (!resource) {
			resource = new Resource({ fetch: this.#fetch, initial: [] });
			this.#resources.set(extension_id, resource);
		}
		return resource;
	}

	/** Sets one row's `enabled` in place, adding the row if it isn't there. */
	set(extension_id: number, name: Name, enabled: boolean) {
		this.get(extension_id).update((rows) =>
			rows.some((r) => this.#name(r) === name)
				? rows.map((r) => (this.#name(r) === name ? { ...r, enabled } : r))
				: [...rows, this.#row(extension_id, name, enabled)]
		);
	}

	refetchAll(): Promise<void>[] {
		return [...this.#resources.values()].map((r) => r.refetch());
	}

	delete(extension_id: number) {
		this.#resources.delete(extension_id);
	}
}

export class ExtensionsStore {
	/** The installed extensions, alphabetical by script identifier. */
	readonly list: Resource<ExtensionSummary[]>;
	/** What the daemon's last update check found, for extensions still at another version. */
	readonly updates: Resource<ExtensionUpdate[]>;

	#send: Send;
	#permissions: Grants<ExtensionPermission, ExtensionPermissionKind>;
	#hooks: Grants<ExtensionHook, HookKind>;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: async () =>
				(await send('listExtensions')).sort((a, b) =>
					a.script_identifier.localeCompare(b.script_identifier)
				),
			initial: []
		});
		this.updates = new Resource({
			fetch: () => send('listExtensionUpdates'),
			initial: [],
			matches: updatesChanged
		});
		this.#permissions = new Grants(
			(extension_id) => send('listExtensionPermissions', { extension_id }),
			(p) => p.permission,
			(extension_id, permission, enabled) => ({ id: 0, extension_id, permission, enabled })
		);
		this.#hooks = new Grants(
			(extension_id) => send('listExtensionHooks', { extension_id }),
			(h) => h.hook,
			(extension_id, hook, enabled) => ({ id: 0, extension_id, hook, enabled })
		);
	}

	load(): Promise<void> {
		return this.list.load();
	}

	/** The permission rows of `extension_id`, created on first use; call `load` on it to fetch. */
	permissions(extension_id: number): Resource<ExtensionPermission[], number> {
		return this.#permissions.get(extension_id);
	}

	/** Grants or revokes one permission. The reply carries nothing, so the row is updated in place. */
	async setPermission(
		extension_id: number,
		permission: ExtensionPermissionKind,
		enabled: boolean
	): Promise<void> {
		await this.#send('setExtensionPermission', { extension_id, permission, enabled });
		this.#permissions.set(extension_id, permission, enabled);
	}

	/** The hook rows of `extension_id`, created on first use; call `load` on it to fetch. */
	hooks(extension_id: number): Resource<ExtensionHook[], number> {
		return this.#hooks.get(extension_id);
	}

	/**
	 * Turns one hook on or off; it applies the next time the hook fires. The
	 * reply carries nothing, so the row is updated in place.
	 */
	async setHook(extension_id: number, hook: HookKind, enabled: boolean): Promise<void> {
		await this.#send('setExtensionHook', { extension_id, hook, enabled });
		this.#hooks.set(extension_id, hook, enabled);
	}

	/**
	 * Asks every registry the user has granted `registry` for extensions
	 * matching `query`; `""` lists everything. Registries are scripts, so this
	 * goes over HTTP. One section per registry, in priority order.
	 */
	search(query: string): Promise<RegistrySection[]> {
		return this.#send('searchExtensionRegistries', { query });
	}

	/**
	 * Asks every registry about the installed extensions now, rather than
	 * waiting for the daemon's daily check. Registries are scripts, so this
	 * goes over HTTP.
	 */
	async checkUpdates(): Promise<void> {
		this.updates.set(await this.#send('checkExtensionUpdates'));
	}

	/**
	 * Downloads the script at `source_url` and reads its real manifest, for
	 * review before installing. Nothing is installed.
	 */
	preview(source_url: string): Promise<ExtensionPreview> {
		return this.#send('previewExtension', { source_url });
	}

	/** Reads the manifest of a script from a file or a paste, for review. Nothing is installed. */
	previewScript(script: string): Promise<ExtensionPreview> {
		return this.#send('previewExtensionScript', { script });
	}

	/**
	 * Installs (or, with `allow_overwrite`, replaces) an extension. A new
	 * install starts with every permission and hook off; a reinstall keeps
	 * existing grants. The daemon evaluates the script, so this goes over HTTP. The reply carries nothing,
	 * so the list, its updates and any loaded hooks and permissions are fetched again.
	 */
	async install(script: string, allow_overwrite: boolean): Promise<void> {
		await this.#send('registerExtension', { script, allow_overwrite });
		await Promise.all([
			this.list.refetch(),
			this.updates.refetch(),
			...this.#permissions.refetchAll(),
			...this.#hooks.refetchAll()
		]);
	}

	/**
	 * Removes an extension: the daemon stops its sandbox and timers and drops
	 * its grants. The reply carries nothing, so the list is fetched again.
	 */
	async uninstall(extension_id: number): Promise<void> {
		await this.#send('uninstallExtension', { extension_id });
		this.#permissions.delete(extension_id);
		this.#hooks.delete(extension_id);
		await this.list.refetch();
	}
}

export function createExtensionsStore(send: Send = defaultSend): ExtensionsStore {
	return new ExtensionsStore(send);
}

export const extensions = createExtensionsStore();
