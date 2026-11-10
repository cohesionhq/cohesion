// Settings → Usage: what recent model calls cost.

import { usageCsv } from '$lib/format/usage';
import { send as defaultSend } from '$lib/protocol/api';
import type { IntelligenceCostEvent, Send } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

/** How many of the most recent cost events the section shows. */
export const USAGE_LIMIT = 200;

/** A limit no daemon reaches, so an export has every cost event. */
const EVERY_EVENT = Number.MAX_SAFE_INTEGER;

export class UsageStore {
	readonly events: Resource<IntelligenceCostEvent[]>;
	exporting = $state(false);
	readonly #send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.events = new Resource({
			fetch: () => send('listIntelligenceCostEvents', { limit: USAGE_LIMIT }),
			initial: [],
			matches: (key) => key.kind === 'intelligenceCostEvents'
		});
	}

	load(): Promise<void> {
		return this.events.load();
	}

	/** Every model call ever recorded, not only the section's most recent, as CSV. */
	async exportCsv(): Promise<string> {
		this.exporting = true;
		try {
			return usageCsv(await this.#send('listIntelligenceCostEvents', { limit: EVERY_EVENT }));
		} finally {
			this.exporting = false;
		}
	}
}

export function createUsageStore(send: Send = defaultSend): UsageStore {
	return new UsageStore(send);
}

export const usage = createUsageStore();
