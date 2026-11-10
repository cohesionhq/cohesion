// Backlinks: the items whose text links to an item (`backlinksOf`), once per
// item and shared by every panel showing it, in either half of a split view.
// Each asks again when what links to its item changes.

import { send as defaultSend } from '$lib/protocol/api';
import type { Embed, Invalidation, LinkKind, Send } from '$lib/protocol/types';
import { parseItemLink, type ItemKind } from '$lib/format/itemLinks';
import { Resource } from './resource.svelte';

/** What a panel's Backlinks show: the items linking to its item. */
export type BacklinksResource = Resource<Embed[], string>;

export class BacklinksStore {
	#send: Send;
	/** Each link's backlinks and how many panels show them. */
	// Bookkeeping only; each panel renders from its own `Resource`.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#shown = new Map<string, { backlinks: BacklinksResource; panels: number }>();

	constructor(send: Send) {
		this.#send = send;
	}

	/**
	 * The backlinks of the item link `href` names, loading, for a panel to
	 * show until it calls the `release` that comes with it.
	 */
	show(href: string): { backlinks: BacklinksResource; release: () => void } {
		let shown = this.#shown.get(href);
		if (!shown) {
			const backlinks: BacklinksResource = new Resource({
				fetch: (link) => this.#send('backlinksOf', { link }),
				initial: [],
				matches: (key, link) => touches(key, link)
			});
			shown = { backlinks, panels: 0 };
			this.#shown.set(href, shown);
		}
		shown.panels++;
		void shown.backlinks.load(href);
		let released = false;
		const entry = shown;
		return {
			backlinks: entry.backlinks,
			release: () => {
				if (released) return;
				released = true;
				if (--entry.panels === 0) entry.backlinks.release();
			}
		};
	}
}

/**
 * Whether invalidation `key` may change what links to the item `link`
 * names: its `backlinks`, or a workspace going, which takes the items in it
 * that linked.
 */
export function touches(key: Invalidation, link: string): boolean {
	if (key.kind === 'workspace') return true;
	if (key.kind !== 'backlinks') return false;
	const item = parseItemLink(link);
	return item !== null && key.id.kind === LINK_KINDS[item.kind] && key.id.id === item.id;
}

/** How the daemon names each kind of item a link names. */
const LINK_KINDS: Record<ItemKind, LinkKind> = {
	task: 'task',
	document: 'document',
	resource: 'resource',
	database: 'database',
	'database-row': 'databaseRow',
	contact: 'contact',
	agent: 'agent',
	'actor-session': 'actorSession',
	article: 'article'
};

export function createBacklinksStore(send: Send = defaultSend): BacklinksStore {
	return new BacklinksStore(send);
}

/** Every panel's backlinks, shared by both halves of a split view. */
export const backlinks = createBacklinksStore();
