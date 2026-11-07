// Feeds: the RSS and Atom feeds every workspace shares, in their order with
// their unread counts, and searching the corpus of articles they brought in.

import type {
	Feed,
	FeedCleanup,
	FeedCorpusPage,
	FeedFullPagePolicy,
	FeedSummary,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

export class FeedsStore {
	readonly list: Resource<FeedSummary[]>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: () => send('listFeeds'),
			initial: [],
			matches: (key) => key.kind === 'feeds'
		});
	}

	load(): Promise<void> {
		return this.list.load();
	}

	/** Every feed's unread articles together. */
	get unread(): number {
		return this.list.value.reduce((sum, f) => sum + f.unread, 0);
	}

	/** Reads `url` and follows it. Reading a feed's items can take a while, so this goes over HTTP. */
	async add(url: string): Promise<Feed> {
		const feed = await this.#send('addFeed', { url });
		this.#replace([feed]);
		return feed;
	}

	/** Reads one feed now, over HTTP. */
	async refresh(feed_id: number): Promise<Feed> {
		const feed = await this.#send('refreshFeed', { feed_id });
		this.#replace([feed]);
		return feed;
	}

	/** Reads every feed now, over HTTP. */
	async refreshAll(): Promise<Feed[]> {
		const feeds = await this.#send('refreshFeeds');
		this.#replace(feeds);
		return feeds;
	}

	async rename(feed_id: number, title: string): Promise<Feed> {
		const feed = await this.#send('renameFeed', { feed_id, title });
		this.#replace([feed]);
		return feed;
	}

	/** Puts the feeds in the order of `feed_ids`, showing it before the daemon confirms. */
	async reorder(feed_ids: number[]): Promise<void> {
		this.list.update((list) => feed_ids.map((id) => list.find((f) => f.id === id)!));
		try {
			this.list.set(await this.#send('reorderFeeds', { feed_ids }));
		} catch (error) {
			void this.list.refetch();
			throw error;
		}
	}

	async setFullPagePolicy(feed_id: number, full_page_policy: FeedFullPagePolicy): Promise<Feed> {
		const feed = await this.#send('setFeedFullPagePolicy', { feed_id, full_page_policy });
		this.#replace([feed]);
		return feed;
	}

	/** Stops following a feed; the daemon removes its articles. */
	async remove(feed_id: number): Promise<Feed> {
		const feed = await this.#send('deleteFeed', { feed_id });
		this.list.update((list) => list.filter((f) => f.id !== feed_id));
		return feed;
	}

	/** Removes the articles of `feed_ids` published before `before` (Unix seconds). */
	prune(feed_ids: number[], before: number): Promise<FeedCleanup> {
		return this.#send('pruneFeeds', { feed_ids, before });
	}

	/** A page of the full-text matches in the feed corpus, best first. */
	search(
		query: string,
		feed_ids: number[] | undefined,
		offset: number,
		limit: number
	): Promise<FeedCorpusPage> {
		return this.#send('searchFeedCorpus', { query, feed_ids: feed_ids ?? null, offset, limit });
	}

	/**
	 * Counts `articles`, just marked read (or unread), out of (or into) their
	 * feeds' unread counts, as the daemon's next listing would.
	 */
	countRead(articles: { feed_id: number }[], read: boolean): void {
		const step = read ? -1 : 1;
		this.list.update((list) =>
			list.map((f) => {
				const n = articles.filter((a) => a.feed_id === f.id).length;
				return n === 0 ? f : { ...f, unread: Math.max(0, f.unread + step * n) };
			})
		);
	}

	/** Takes replies into the list; a new feed's items, all just added, are unread. */
	#replace(changed: Feed[]): void {
		this.list.update((list) => [
			...list.map((f) => {
				const c = changed.find((c) => c.id === f.id);
				return c ? { ...c, unread: f.unread } : f;
			}),
			...changed
				.filter((c) => !list.some((f) => f.id === c.id))
				.map((c) => ({ ...c, unread: c.status.state === 'refreshed' ? c.status.added : 0 }))
		]);
	}
}

export function createFeedsStore(send: Send = defaultSend): FeedsStore {
	return new FeedsStore(send);
}

export const feeds = createFeedsStore();
