// The modules (optional areas of the app, such as Calendar) and whether each
// is on in a workspace. `current` is the selected workspace's, which the
// sidebar, ⌘K and a module's page read through `isEnabled`; `edited` is the
// workspace Settings › Workspaces is changing. Both refetch on that
// workspace's `modules` key.

import { send as defaultSend } from '$lib/protocol/api';
import type { Module, ModuleSetting, Send } from '$lib/protocol/types';
import { Resource } from './resource.svelte';

function modulesOf(send: Send): Resource<ModuleSetting[], number> {
	return new Resource({
		fetch: (workspace_id) => send('listWorkspaceModules', { workspace_id }),
		initial: [],
		matches: (key, workspace_id) => key.kind === 'modules' && key.id === workspace_id
	});
}

export class ModulesStore {
	readonly current: Resource<ModuleSetting[], number>;
	readonly edited: Resource<ModuleSetting[], number>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.current = modulesOf(send);
		this.edited = modulesOf(send);
	}

	/**
	 * Whether `module` is on in the selected workspace. A module is on unless
	 * the daemon says otherwise, so it shows while the setting loads.
	 */
	isEnabled(module: Module): boolean {
		return this.current.value.find((setting) => setting.module === module)?.enabled ?? true;
	}

	/** Turns `module` on or off in `workspace_id`. */
	async set(workspace_id: number, module: Module, enabled: boolean): Promise<void> {
		const now = await this.#send('setModuleEnabled', { workspace_id, module, enabled });
		for (const resource of [this.current, this.edited])
			if (resource.key === workspace_id) resource.set(now);
	}
}

export function createModulesStore(send: Send = defaultSend): ModulesStore {
	return new ModulesStore(send);
}

export const modules = createModulesStore();
