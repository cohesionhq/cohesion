// Settings → Experiments: features still being tried, each on or off for
// this daemon. Refetches on the `experiments` key. To branch on one, load
// the store and ask `experiments.isEnabled('<key>')`; the key is the
// experiment's `Experiment::key` in `cohesion-core`.

import { send as defaultSend } from '$lib/protocol/api';
import type { ExperimentSetting, Send } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

export class ExperimentsStore {
	readonly list: Resource<ExperimentSetting[]>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: () => send('listExperiments'),
			initial: [],
			matches: (key) => key.kind === 'experiments'
		});
	}

	load(): Promise<void> {
		return this.list.load();
	}

	release(): void {
		this.list.release();
	}

	/** Whether the experiment named `key` is on; off when it doesn't exist or hasn't loaded. */
	isEnabled(key: string): boolean {
		return this.list.value.some((experiment) => experiment.key === key && experiment.enabled);
	}

	/** Turns the experiment named `key` on or off. */
	async set(key: string, enabled: boolean): Promise<void> {
		const updated = await this.#send('setExperiment', { key, enabled });
		this.list.update(() => updated);
	}
}

export function createExperimentsStore(send: Send = defaultSend): ExperimentsStore {
	return new ExperimentsStore(send);
}

export const experiments = createExperimentsStore();
