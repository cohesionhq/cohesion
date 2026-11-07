import { describe, expect, it } from 'vitest';
import { createMock } from '$lib/protocol/mock';
import { createArticlesStore, parseView, viewParam } from './articles.svelte';
import { createFeedsStore } from './feeds.svelte';

describe('article views', () => {
	it('round-trips the view param, falling back to every feed', () => {
		for (const view of ['feeds', { feed: 2 }] as const)
			expect(parseView(viewParam(view))).toEqual(view);
		expect(parseView(null)).toBe('feeds');
		expect(parseView('folder-3')).toBe('feeds');
		expect(parseView('feed-x')).toBe('feeds');
	});
});

describe('articles store', () => {
	it('keeps an earlier snapshot when fetching again fails', async () => {
		const mock = createMock();
		const store = createArticlesStore(mock.send);
		await store.read(1);
		// The seeded capture now points at a host the mock can't reach.
		mock.state.articles[0].url = 'https://gone.example/sourdough';
		const refetched = await store.resnapshot();
		expect(refetched.failure?.kind).toBe('unreachable');
		expect(store.current.value?.snapshot.state).toBe('captured');
		expect(store.current.value?.contents).not.toBe('');
	});

	it('refetches a view and the open article on the feeds’ invalidation', async () => {
		const store = createArticlesStore(createMock().send);
		await store.open({ feed: 1 }, 0, 50);
		await store.read(1);
		expect(store.list.touchedBy([{ kind: 'feedArticles' }])).toBe(true);
		expect(store.current.touchedBy([{ kind: 'feedArticles' }])).toBe(true);
		expect(store.list.touchedBy([{ kind: 'feeds' }])).toBe(false);
	});

	it('lists a page of a view with the view’s total', async () => {
		const store = createArticlesStore(createMock().send);
		await store.open('feeds', 0, 50);
		const { articles, total } = store.list.value;
		await store.open('feeds', 1, 1);
		expect(store.list.value.articles.map((l) => l.id)).toEqual([articles[1].id]);
		expect(store.list.value.total).toBe(total);
	});

	it('renames the open article in the list too', async () => {
		const store = createArticlesStore(createMock().send);
		await store.open('feeds', 0, 50);
		const [first] = store.list.value.articles;
		await store.read(first.id);
		await store.rename('Renamed');
		expect(store.current.value?.title).toBe('Renamed');
		expect(store.list.value.articles.find((l) => l.id === first.id)?.title).toBe('Renamed');
	});

	it('marks one article and then a whole view read', async () => {
		const store = createArticlesStore(createMock().send);
		await store.open({ feed: 1 }, 0, 50);
		const [first] = store.list.value.articles;
		await store.read(first.id);
		await store.setRead(first.id, true);
		expect(store.current.value?.read).toBe(true);
		expect(store.list.value.articles.find((l) => l.id === first.id)?.read).toBe(true);

		const { marked, unread } = await store.markAllRead();
		expect(unread.map((l) => l.id)).not.toContain(first.id);
		expect(marked).toBe(unread.length);
		expect(store.list.value.articles.every((l) => l.read)).toBe(true);
		await store.list.refetch();
		expect(store.list.value.articles.every((l) => l.read)).toBe(true);
	});
});

describe('feeds store', () => {
	it('adds a feed into the list and cleans up its articles', async () => {
		const store = createFeedsStore(createMock().send);
		await store.load();
		const before = store.list.value.length;
		const feed = await store.add('https://news.example/rss');
		expect(store.list.value).toHaveLength(before + 1);
		expect(store.list.value.at(-1)?.unread).toBe(1);
		const { hits } = await store.search('first', [feed.id], 0, 50);
		expect(hits.map((h) => h.feed_id)).toEqual([feed.id]);
		const cleanup = await store.prune([feed.id], Number.MAX_SAFE_INTEGER);
		expect(cleanup.removed).toBe(1);
		expect(await store.search('first', [feed.id], 0, 50)).toEqual({ hits: [], total: 0 });
	});

	it('reorders the feeds and counts read articles out of their unread counts', async () => {
		const store = createFeedsStore(createMock().send);
		await store.load();
		const added = await store.add('https://news.example/rss');
		await store.reorder([added.id, 1]);
		expect(store.list.value.map((f) => f.id)).toEqual([added.id, 1]);
		await store.list.refetch();
		expect(store.list.value.map((f) => f.id)).toEqual([added.id, 1]);

		const unread = store.unread;
		store.countRead([{ feed_id: 1 }], true);
		expect(store.unread).toBe(unread - 1);
	});
});
