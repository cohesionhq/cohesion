// Marking articles read or unread, keeping the feeds' unread counts in step
// until the daemon's own listing arrives.

import type { ArticlesStore } from '$lib/stores/articles.svelte';
import { feeds } from '$lib/stores/feeds.svelte';

/** Marks one article read or unread, through the page's `articles`. */
export async function markRead(
	articles: ArticlesStore,
	article_id: number,
	read: boolean
): Promise<void> {
	const article = await articles.setRead(article_id, read);
	feeds.countRead([article], read);
}

/** Marks every article in the loaded view read; replies with how many weren't. */
export async function markAllRead(articles: ArticlesStore): Promise<number> {
	const { marked, unread } = await articles.markAllRead();
	feeds.countRead(unread, true);
	return marked;
}
