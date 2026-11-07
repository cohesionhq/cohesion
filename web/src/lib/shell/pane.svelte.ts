// The sheet's halves. The main half is SvelteKit's page; a split view opens
// a second half beside it (`SplitPane`) that routes on its own, so each half
// has its own address and history. A page reads where it is from `pane()`,
// never from `$app/state`, and moves with `pane().goto`, so the same page
// works in either half; a link clicked in a half opens there
// (`PaneHost.svelte`). What sits outside both (the sidebar, dialogs) moves
// the main half.

import { getContext, onDestroy, setContext } from 'svelte';
import { beforeNavigate, goto } from '$app/navigation';
import { page } from '$app/state';
import { createPageStores, dropPageStores, type PageStores } from '$lib/stores/pageStores.svelte';
import { pageName } from './back';
import { fade, fadingScope } from './fade';
import { resolveRoute, type Shown } from './splitRoutes';

export interface GotoOptions {
	/** Takes the place of this entry, which the new page then follows in its stead: back skips it. */
	replaceState?: boolean;
	keepFocus?: boolean;
}

/** One half of the sheet: where it is, and how to move it. */
export interface Pane {
	readonly url: URL;
	readonly params: Record<string, string>;
	readonly route: { id: string | null };
	readonly state: App.PageState;
	goto(href: string, options?: GotoOptions): Promise<void>;
	/** Steps through this half's history, -1 back and 1 forward. */
	step(delta: number): void;
	/** Runs `save` before this half leaves its page; call while a component initialises. */
	beforeLeave(save: () => void): void;
	/** Names the page this half shows, as a `<title>` names the window's. */
	setTitle(title: string): void;
}

/** The main half: the window's own address, history and title. */
export const mainPane: Pane = {
	get url() {
		return page.url;
	},
	get params() {
		return page.params;
	},
	get route() {
		return page.route;
	},
	get state() {
		return page.state;
	},
	goto: (href, options) =>
		goto(
			href,
			options?.replaceState ? { ...options, state: { back: page.state.back ?? null } } : options
		),
	step: (delta) => history.go(delta),
	beforeLeave: (save) => beforeNavigate(save),
	setTitle: (title) => {
		document.title = title;
	}
};

interface Entry {
	href: string;
	state: App.PageState;
}

/**
 * The split view's second half, blank until a page opens in it. Its history
 * is its own list of entries, so going back in one half leaves the other
 * where it is.
 */
export class SplitPane implements Pane {
	/** What it shows, or null while blank. */
	shown = $state.raw<Shown | null>(null);
	/** The page's title, for the back link of the page opened from it. */
	title = $state('');
	#entries = $state.raw<Entry[]>([]);
	#index = $state(-1);
	// Bookkeeping, not state: nothing draws it.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#leaving = new Set<() => void>();
	/** The element the half is drawn in, which a navigation fades. */
	root: HTMLElement | undefined;
	/** Its own stores, so its page and the main half's never share an open item. */
	readonly stores: PageStores = createPageStores();

	get url() {
		// A blank half is nowhere; the URL is never changed in place.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		return this.shown?.url ?? new URL('/', location.origin);
	}
	get params() {
		return this.shown?.params ?? {};
	}
	get route() {
		return this.shown?.route ?? { id: null };
	}
	get state() {
		return this.#entries[this.#index]?.state ?? {};
	}
	/** Where it is, as a path, or null while blank. */
	get href(): string | null {
		return this.#entries[this.#index]?.href ?? null;
	}

	async goto(href: string, options: GotoOptions = {}) {
		const from = this.shown;
		const label = pageName(this.title);
		const replaced = this.state;
		const next = await this.#show(href);
		const at = next.url.pathname + next.url.search;
		// A query change (a list's view) is the same page, as `trackBack` has it.
		const back =
			from && from.url.pathname !== next.url.pathname
				? { href: from.url.pathname + from.url.search, label }
				: undefined;
		const entry = { href: at, state: options.replaceState ? replaced : back ? { back } : {} };
		const kept = this.#entries.slice(0, this.#index + (options.replaceState ? 0 : 1));
		this.#entries = [...kept, entry];
		this.#index = kept.length;
		panes.save();
	}

	step(delta: number) {
		const entry = this.#entries[this.#index + delta];
		if (!entry) return;
		this.#index += delta;
		void this.#show(entry.href);
		panes.save();
	}

	beforeLeave(save: () => void) {
		this.#leaving.add(save);
		onDestroy(() => this.#leaving.delete(save));
	}

	setTitle(title: string) {
		this.title = title;
	}

	/** Loads `href`, then swaps it in, fading what changes. */
	async #show(href: string): Promise<Shown> {
		for (const save of this.#leaving) save();
		const next = await resolveRoute(href);
		const from = this.shown;
		const target = this.root && fadingScope(this.root, from, next);
		const swap = async () => {
			this.shown = next;
		};
		await (target ? fade(target, swap) : swap());
		return next;
	}
}

export type Side = 'main' | 'split';

const STORAGE_KEY = 'cohesion.split';

/** Whether the sheet is split, and which half the user is working in. */
class Panes {
	split = $state<SplitPane | null>(null);
	/** The half last clicked or focused: back and forward step it, and ⌘K opens in it. */
	focused = $state<Side>('main');

	/** The half the user is working in. */
	get current(): Pane {
		return this.focused === 'split' && this.split ? this.split : mainPane;
	}

	/** Opens the second half, blank or at `href`, or moves it to `href` when it is open. */
	openSplit(href?: string) {
		this.split ??= new SplitPane();
		this.focused = 'split';
		if (href) void this.split.goto(href);
		else this.save();
	}

	/** Closes one half; the other fills the sheet. */
	close(side: Side) {
		const split = this.split;
		if (!split) return;
		if (side === 'main' && split.href) void mainPane.goto(split.href);
		this.split = null;
		dropPageStores(split.stores);
		this.focused = 'main';
		this.save();
	}

	/** Keeps the split for a reload of this window; each window has its own. */
	save() {
		if (this.split) sessionStorage.setItem(STORAGE_KEY, this.split.href ?? '');
		else sessionStorage.removeItem(STORAGE_KEY);
	}

	/** Reopens the split this window had before a reload. */
	restore() {
		const href = sessionStorage.getItem(STORAGE_KEY);
		if (href === null) return;
		this.openSplit(href || undefined);
		this.focused = 'main';
	}
}

export const panes = new Panes();

const KEY = Symbol('pane');

/** Makes `pane` the half the components below are drawn in. */
export function providePane(pane: Pane) {
	setContext(KEY, pane);
}

/** The half this component is drawn in; outside both, the main one. */
export function pane(): Pane {
	return getContext<Pane | undefined>(KEY) ?? mainPane;
}
