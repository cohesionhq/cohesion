// Which requests are still in flight, for the working mark. Every request
// `send` makes counts as one of two kinds: a load, the fetches a
// `Resource` makes to load and refetch its value (navigating, or a list
// refreshing after a change), or an action, everything else, which the
// user set off. The mark waits longer before showing for a load, so it
// only shows one when the connection is slow.

import type { Send } from './types';

/** What a request in flight is for. */
type Kind = 'action' | 'load';

class Working {
	/** Whether an action is in flight. */
	acting = $state(false);
	/** Whether a load is in flight. */
	loading = $state(false);

	/**
	 * Requests in flight of each kind. Plain, not state: `send` runs inside
	 * effects, and an effect that read a count it then changed would rerun
	 * itself without end. Assigning `acting` or `loading` reads nothing.
	 */
	#pending: Record<Kind, number> = { action: 0, load: 0 };
	#kind: Kind = 'action';

	/** `send`, counting each request it makes: a load inside `loading`, else an action. */
	track(send: Send): Send {
		return ((type, ...args) => {
			const kind = this.#kind;
			this.#settle(kind, 1);
			return send(type, ...args).finally(() => this.#settle(kind, -1));
		}) as Send;
	}

	#settle(kind: Kind, change: 1 | -1): void {
		this.#pending[kind] += change;
		if (kind === 'action') this.acting = this.#pending.action > 0;
		else this.loading = this.#pending.load > 0;
	}

	/**
	 * Runs `fetch`, counting its requests as loads. Only those it sends
	 * before its first `await` are: the rest count as actions.
	 */
	load<T>(fetch: () => T): T {
		this.#kind = 'load';
		try {
			return fetch();
		} finally {
			this.#kind = 'action';
		}
	}
}

export const working = new Working();
