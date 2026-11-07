// Settings → Agents → Spend limits: what model calls cost over each period,
// against its limit if it has one. The agent conversation reads it too, to
// say when a reached limit holds jobs and forks back.

import { send as defaultSend } from '$lib/protocol/api';
import type { IntelligenceSpend, Send, SpendPeriod } from '$lib/protocol/types';
import { Resource } from '../resource.svelte';

export class SpendStore {
	/** One entry per period, shortest first. */
	readonly periods: Resource<IntelligenceSpend[]>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.periods = new Resource({
			fetch: () => send('listIntelligenceSpend'),
			initial: [],
			// A call's cost changes what was spent; a limit set or cleared is an intelligence setting.
			matches: (key) => key.kind === 'intelligenceCostEvents' || key.kind === 'intelligence'
		});
	}

	load(): Promise<void> {
		return this.periods.load();
	}

	/** The periods whose limit is reached, as the daemon says. */
	get reached(): IntelligenceSpend[] {
		return this.periods.value.filter((p) => p.reached);
	}

	async setLimit(period: SpendPeriod, amount: number): Promise<void> {
		this.periods.set(await this.#send('setIntelligenceSpendLimit', { period, amount }));
	}

	async clearLimit(period: SpendPeriod): Promise<void> {
		this.periods.set(await this.#send('clearIntelligenceSpendLimit', { period }));
	}
}

export function createSpendStore(send: Send = defaultSend): SpendStore {
	return new SpendStore(send);
}

export const spend = createSpendStore();
