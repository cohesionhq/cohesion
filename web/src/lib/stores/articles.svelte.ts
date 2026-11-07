// Articles: what the feeds brought in, per view (every feed or one, which every
// workspace shares), and the one open article.

import type {
	Article,
	ArticleFilter,
	ArticlePage,
	ArticleSummary,
	RefetchedArticle,
	Send
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';
import { send as defaultSend } from '$lib/protocol/api';

/** Which articles the Articles page lists: the daemon's own filter. */
export type ArticleView = ArticleFilter;

export interface ArticleListKey {
	view: ArticleView;
	/** How many of the view's articles come before the page. */
	offset: number;
	/** How many articles the page holds. */
	limit: number;
}

/** Reads `?view=` (`feeds`, `feed-2`); anything else is every feed. */
export function parseView(param: string | null): ArticleView {
	const [, id] = /^feed-(\d+)$/.exec(param ?? '') ?? [];
	return id === undefined ? 'feeds' : { feed: Number(id) };
}

/** `view` as `?view=` spells it. */
export function viewParam(view: ArticleView): string {
	return typeof view === 'string' ? view : `feed-${view.feed}`;
}

export function sameView(a: ArticleView, b: ArticleView): boolean {
	return viewParam(a) === viewParam(b);
}

const VIEW_KEY = 'cohesion.articleView';

/** The view the Articles page last showed, so a return lands there; every feed at first. */
export function rememberedView(): ArticleView {
	try {
		return parseView(localStorage.getItem(VIEW_KEY));
	} catch {
		return 'feeds';
	}
}

export function rememberView(view: ArticleView): void {
	try {
		localStorage.setItem(VIEW_KEY, viewParam(view));
	} catch {
		// Remembering the view is a convenience; ignore blocked storage.
	}
}

/** `article`'s own fields as a listing holds them, without its contents. */
const fields = ({ contents: _contents, ...rest }: Article) => rest;

export class ArticlesStore {
	/** One page of a view. */
	readonly list: Resource<ArticlePage, ArticleListKey>;
	/** The open article, by its id. */
	readonly current: Resource<Article | null, number>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.list = new Resource({
			fetch: ({ view, offset, limit }) => send('listArticles', { filter: view, offset, limit }),
			initial: { articles: [], total: 0 },
			matches: (key) => key.kind === 'feedArticles'
		});
		this.current = new Resource({
			fetch: (article_id) => send('getArticle', { article_id }),
			initial: null,
			matches: (key) => key.kind === 'feedArticles'
		});
	}

	/** Loads one page of a view. The same page reuses the loaded list. */
	async open(view: ArticleView, offset: number, limit: number): Promise<void> {
		const key = this.list.key;
		const same =
			key !== undefined && sameView(key.view, view) && key.offset === offset && key.limit === limit;
		await this.list.load(same ? key : { view, offset, limit });
	}

	read(article_id: number): Promise<void> {
		return this.current.load(article_id);
	}

	/** Fetches the open article's page again. */
	async resnapshot(): Promise<RefetchedArticle> {
		const refetched = await this.#send('resnapshotArticle', {
			article_id: this.current.key!
		});
		this.#patch(refetched.article);
		return refetched;
	}

	/** Renames the open article. */
	async rename(title: string): Promise<Article> {
		const article = await this.#send('renameArticle', {
			article_id: this.current.key!,
			title
		});
		this.#patch(article);
		return article;
	}

	/** Marks an article read or unread. */
	async setRead(article_id: number, read: boolean): Promise<Article> {
		const article = await this.#send('setArticleRead', { article_id, read });
		this.#patch(article);
		return article;
	}

	/**
	 * Marks every article the loaded view covers read, on every page; replies
	 * with how many weren't, and the ones on the loaded page that weren't.
	 */
	async markAllRead(): Promise<{ marked: number; unread: ArticleSummary[] }> {
		const { view } = this.list.key!;
		const unread = this.list.value.articles.filter((l) => !l.read);
		const { marked } = await this.#send('markArticlesRead', { filter: view });
		this.list.update((page) => ({
			...page,
			articles: page.articles.map((l) => (l.read ? l : { ...l, read: true }))
		}));
		const open = this.current.value;
		if (open && !open.read && unread.some((l) => l.id === open.id))
			this.current.set({ ...open, read: true });
		return { marked, unread };
	}

	/** Takes an article's own fields (its title, read state, snapshot) into the open article and the list. */
	#patch(article: Article): void {
		if (this.current.value?.id === article.id) this.current.set(article);
		const own = fields(article);
		this.list.update((page) => ({
			...page,
			articles: page.articles.map((l) => (l.id === article.id ? { ...l, ...own } : l))
		}));
	}
}

export function createArticlesStore(send: Send = defaultSend): ArticlesStore {
	return new ArticlesStore(send);
}
