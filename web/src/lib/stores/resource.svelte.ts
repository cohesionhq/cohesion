// The building block every store is made of: one value fetched from the
// daemon, keyed (e.g. by workspace id), refetched when the server says it
// changed, and updated in place from mutation replies.

import { toProtocolError, type ProtocolError } from '$lib/protocol/errors';
import type { Invalidation } from '$lib/protocol/types';
import { working } from '$lib/protocol/working.svelte';

export interface ResourceOptions<T, K> {
	fetch: (key: K) => Promise<T>;
	initial: T;
	/** Whether an invalidation key touches this resource's data for `key`. */
	matches?: (invalidation: Invalidation, key: K) => boolean;
	/** Return false to skip a background refetch (e.g. while there are unsaved edits). */
	canRefetch?: () => boolean;
	/** Runs after every successful fetch. */
	onFetched?: (value: T) => void;
	/**
	 * Whether the value loaded for `from` keeps showing while `to` loads (the
	 * same list narrowed or reordered), so it swaps in place rather than
	 * emptying first. Otherwise a new key resets the value.
	 */
	keepsValue?: (from: K, to: K) => boolean;
}

// Bookkeeping only; nothing renders from them.
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const active = new Set<Resource<unknown, unknown>>();
/** Released resources: they skip refetches, but remember missing one. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const released = new Set<Resource<unknown, unknown>>();
/** Puts a value on the page; a page fade calls it inside a transition of its own. */
export type Reveal = (show: () => void) => Promise<void>;

/** A fetch for a key with nothing to show yet: what a page opening waits on. */
interface FirstFetch {
	done: Promise<void>;
	/** Set once a page fade stopped waiting for it, so its value fades in rather than popping in. */
	reveal?: Reveal;
}

// eslint-disable-next-line svelte/prefer-svelte-reactivity
const firstFetches = new Set<FirstFetch>();

export class Resource<T, K = void> {
	value: T = $state() as T;
	loading = $state(false);
	error = $state<ProtocolError | null>(null);
	/** True once the first fetch for the current key has finished. */
	loaded = $state(false);
	key: K | undefined = $state();

	#options: ResourceOptions<T, K>;
	#generation = 0;
	#hasKey = false;
	#inflight: Promise<void> | null = null;
	/** Set while the latest fetch waits on its value, which may predate a write applied meanwhile. */
	#reading = false;
	/** Set when a refetch was skipped while released; the next `load` fetches. */
	#stale = false;

	constructor(options: ResourceOptions<T, K>) {
		this.#options = options;
		this.value = options.initial;
	}

	/**
	 * Fetches for `key` unless that key is already loaded and nothing changed
	 * while it was released; joins a fetch in flight. Either way the resource
	 * becomes active again after `release()`.
	 */
	load(key: K): Promise<void> {
		released.delete(this as Resource<unknown, unknown>);
		if (this.#hasKey && Object.is(this.key, key)) {
			active.add(this as Resource<unknown, unknown>);
			if (this.#inflight) return this.#inflight;
			if (this.loaded && !this.#stale) return Promise.resolve();
		}
		const kept = this.loaded && (this.#options.keepsValue?.(this.key as K, key) ?? false);
		if (!kept && (!this.#hasKey || !Object.is(this.key, key))) {
			this.value = this.#options.initial;
			this.loaded = false;
		}
		this.key = key;
		this.#hasKey = true;
		active.add(this as Resource<unknown, unknown>);
		return this.refetch();
	}

	/** Fetches again for the current key; a newer fetch wins over an older one. */
	refetch(): Promise<void> {
		if (!this.#hasKey) return Promise.resolve();
		const first: FirstFetch | null = this.loaded ? null : { done: Promise.resolve() };
		const run = this.#fetch(first);
		this.#inflight = run;
		if (first) {
			first.done = run;
			firstFetches.add(first);
		}
		void run.finally(() => {
			if (this.#inflight === run) this.#inflight = null;
			if (first) firstFetches.delete(first);
		});
		return run;
	}

	async #fetch(first: FirstFetch | null): Promise<void> {
		const generation = ++this.#generation;
		this.#stale = false;
		const key = this.key as K;
		this.loading = true;
		this.#reading = true;
		try {
			const value = await working.load(() => this.#options.fetch(key));
			if (generation !== this.#generation) return;
			const show = () => {
				if (generation !== this.#generation) return;
				this.#reading = false;
				this.value = value;
				this.error = null;
				this.loaded = true;
				this.#options.onFetched?.(value);
			};
			if (first?.reveal) await first.reveal(show);
			else show();
		} catch (error) {
			if (generation !== this.#generation) return;
			this.#reading = false;
			this.error = toProtocolError(error);
		} finally {
			if (generation === this.#generation) this.loading = false;
		}
	}

	/** Replaces the value with one from a mutation reply. */
	set(value: T): void {
		this.value = value;
		this.#supersede();
	}

	update(change: (value: T) => T): void {
		this.value = change(this.value);
		this.#supersede();
	}

	/**
	 * The daemon answers requests side by side, so a fetch still waiting may
	 * have read before the write just applied and would undo it: fetch again,
	 * which drops it.
	 */
	#supersede(): void {
		if (this.#reading) void this.refetch();
	}

	/** Background refetch, honouring `canRefetch`. */
	refresh(): Promise<void> {
		if (this.#options.canRefetch && !this.#options.canRefetch()) return Promise.resolve();
		return this.refetch();
	}

	/** Whether any of `keys` touches this resource. */
	touchedBy(keys: Invalidation[]): boolean {
		if (!this.#hasKey) return false;
		const matches = this.#options.matches;
		return matches ? keys.some((k) => matches(k, this.key as K)) : false;
	}

	/**
	 * Stops background refetches until the next `load`; the value is kept,
	 * and that `load` refetches if one was skipped meanwhile.
	 */
	release(): void {
		active.delete(this as Resource<unknown, unknown>);
		released.add(this as Resource<unknown, unknown>);
	}

	/** Stops background refetches for good: the half of the sheet that showed it closed. */
	drop(): void {
		active.delete(this as Resource<unknown, unknown>);
		released.delete(this as Resource<unknown, unknown>);
	}

	/** Records a skipped refetch for the next `load`. */
	markStale(): void {
		this.#stale = true;
	}
}

/**
 * Settles once every fetch for a key with nothing loaded yet has finished,
 * or after `ms`, whichever comes first: a page that just mounted has asked
 * for its data, and a page navigation waits on this so it fades to the page
 * as it will look rather than to an empty one. A fetch still running at `ms`
 * puts its value on the page through `reveal`, so what lands late fades in
 * too rather than popping in after the fade.
 */
export function firstFetchesSettled(ms: number, reveal?: Reveal): Promise<void> {
	const waiting = [...firstFetches];
	return new Promise((resolve) => {
		const timer = setTimeout(() => {
			for (const fetch of waiting) if (reveal && firstFetches.has(fetch)) fetch.reveal = reveal;
			resolve();
		}, ms);
		void Promise.allSettled(waiting.map((fetch) => fetch.done)).then(() => {
			clearTimeout(timer);
			resolve();
		});
	});
}

/** Refetches every loaded resource (window focus, `reset`, reconnect). */
export function refreshAll(): Promise<void> {
	for (const r of released) r.markStale();
	return Promise.all([...active].map((r) => r.refresh())).then(() => undefined);
}

/** Refetches the loaded resources an `invalidate` push touches. */
export function refreshInvalidated(keys: Invalidation[]): Promise<void> {
	for (const r of released) if (r.touchedBy(keys)) r.markStale();
	const touched = [...active].filter((r) => r.touchedBy(keys));
	return Promise.all(touched.map((r) => r.refresh())).then(() => undefined);
}
