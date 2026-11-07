// Settings → Usage: what recent model calls cost.

import { send as defaultSend } from '$lib/protocol/api';
import type { IntelligenceCostEvent, Send } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

/** How many of the most recent cost events the section shows. */
export const USAGE_LIMIT = 200;

export class UsageStore {
	readonly events: Resource<IntelligenceCostEvent[]>;

	constructor(send: Send) {
		this.events = new Resource({
			fetch: () => send('listIntelligenceCostEvents', { limit: USAGE_LIMIT }),
			initial: [],
			matches: (key) => key.kind === 'intelligenceCostEvents'
		});
	}

	load(): Promise<void> {
		return this.events.load();
	}
}

export function createUsageStore(send: Send = defaultSend): UsageStore {
	return new UsageStore(send);
}

export const usage = createUsageStore();
