// Model threads, read-only: the stored transcript of an agent's conversation
// or of one of its runs, opened from the runs list or Usage.

import type { IntelligenceThreadDetail, Send } from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export class ThreadsStore {
	readonly current: Resource<IntelligenceThreadDetail | null, number>;

	constructor(send: Send) {
		this.current = new Resource({
			fetch: (thread_id) => send('getIntelligenceThread', { thread_id }),
			initial: null,
			matches: (key) => key.kind === 'intelligenceCostEvents'
		});
	}

	open(thread_id: number): Promise<void> {
		return this.current.load(thread_id);
	}
}

export function createThreadsStore(send: Send = defaultSend): ThreadsStore {
	return new ThreadsStore(send);
}
