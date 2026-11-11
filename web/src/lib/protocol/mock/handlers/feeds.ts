// Feeds and their articles.

import { ProtocolError } from '../../errors';
import type { Feed, FeedCorpusHit } from '../../types';
import { feedItem } from '../seed';
import { nowSeconds, paged } from '../support';
import type { Handlers, MockCore } from '../core';

export function feedsHandlers(core: MockCore) {
	const {
		state,
		nextId,
		feed,
		liveFeeds,
		feedSummary,
		removeItems,
		summary,
		coveredBy,
		requireTitle,
		record,
		article
	} = core;
	return {
		listArticles: ({ filter, offset = 0, limit = null }) => {
			const covered = coveredBy(filter);
			const listed = state.articles
				.filter((l) => !l.deleted && covered(l))
				.sort((a, b) => b.published_at - a.published_at || b.id - a.id)
				.map(summary);
			const { rows, total } = paged(listed, offset, limit);
			return { articles: rows, total };
		},
		setArticleRead: ({ article_id, read }) => {
			const found = article(article_id);
			found.read = read;
			return structuredClone(found);
		},
		markArticlesRead: ({ filter }) => {
			const covered = coveredBy(filter);
			let marked = 0;
			for (const l of state.articles)
				if (!l.deleted && !l.read && covered(l)) {
					l.read = true;
					marked++;
				}
			return { marked };
		},
		listFeeds: () => liveFeeds().map(feedSummary),
		// The mock can't read the web: every address reads as a feed of one item.
		addFeed: ({ url, full_page_policy = 'whenExcerpt' }) => {
			let parsed: URL;
			try {
				parsed = new URL(url.trim());
			} catch {
				parsed = new URL('about:blank');
			}
			if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')
				throw new ProtocolError(
					'validationFailed',
					'Enter a feed address starting with http:// or https://.',
					'url'
				);
			if (liveFeeds().some((f) => f.url === parsed.href))
				throw new ProtocolError('validationFailed', 'You already follow that feed.', 'url');
			const now = nowSeconds();
			const created: Feed = {
				id: nextId(state.feeds),
				deleted: false,
				url: parsed.href,
				title: parsed.host,
				sort_order: Math.max(0, ...state.feeds.map((f) => f.sort_order)) + 1,
				full_page_policy,
				status: { state: 'refreshed', at: now, added: 0 },
				created_at: now
			};
			state.feeds.push(created);
			const itemURL = new URL('/welcome', parsed).href;
			state.articles.push(
				feedItem(
					nextId(state.articles),
					created.id,
					parsed.host,
					itemURL,
					`Welcome to ${parsed.host}`,
					now,
					now,
					'The first post.'
				)
			);
			created.status = { state: 'refreshed', at: now, added: 1 };
			return structuredClone(created);
		},
		refreshFeed: ({ feed_id }) => {
			const f = feed(feed_id);
			f.status = { state: 'refreshed', at: nowSeconds(), added: 0 };
			return structuredClone(f);
		},
		refreshFeeds: () => {
			const now = nowSeconds();
			return liveFeeds().map((f) => {
				f.status = { state: 'refreshed', at: now, added: 0 };
				return structuredClone(f);
			});
		},
		renameFeed: ({ feed_id, title }) => {
			requireTitle(title);
			const f = feed(feed_id);
			f.title = title;
			return structuredClone(f);
		},
		reorderFeeds: ({ feed_ids }) => {
			feed_ids.forEach((id, i) => (feed(id).sort_order = i));
			return liveFeeds().map(feedSummary);
		},
		setFeedFullPagePolicy: ({ feed_id, full_page_policy }) => {
			const f = feed(feed_id);
			f.full_page_policy = full_page_policy;
			return structuredClone(f);
		},
		deleteFeed: ({ feed_id }) => {
			const f = feed(feed_id);
			f.deleted = true;
			removeItems([feed_id]);
			return structuredClone(f);
		},
		pruneFeeds: ({ feed_ids, before }) => ({ removed: removeItems(feed_ids, before) }),
		// Every word must start a word of the title, author or contents.
		searchFeedCorpus: ({ query, feed_ids = null, offset = 0, limit = null }) => {
			const words = (query.match(/[\p{L}\p{N}]+/gu) ?? []).map((w) => w.toLowerCase());
			if (words.length === 0) return { hits: [], total: 0 };
			const hits: FeedCorpusHit[] = [];
			for (const l of state.articles) {
				if (l.deleted) continue;
				if (feed_ids !== null && !feed_ids.includes(l.feed_id)) continue;
				const tokens =
					`${l.title} ${l.author ?? ''} ${l.contents}`.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
				if (!words.every((w) => tokens.some((t) => t.startsWith(w)))) continue;
				const { id, feed_id, read, url, title, author, published_at, published_precision } = l;
				hits.push({
					id,
					feed_id,
					read,
					url,
					title,
					author,
					published_at,
					published_precision,
					excerpt: l.contents.slice(0, 160)
				});
			}
			const { rows, total } = paged(hits, offset, limit);
			return { hits: rows, total };
		},
		getArticle: ({ article_id }) => structuredClone(article(article_id)),
		resnapshotArticle: ({ article_id }) => {
			const found = article(article_id);
			return record(found, new URL(found.url));
		},
		renameArticle: ({ article_id, title }) => {
			requireTitle(title);
			const found = article(article_id);
			found.title = title;
			found.modified = nowSeconds();
			return structuredClone(found);
		}
	} satisfies Partial<Handlers>;
}
