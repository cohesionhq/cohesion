// Embeds: the items `cohesion://` links show as cards, resolved by the daemon
// (`resolveLink`) once per link and shared by every card showing it, in
// either half of a split view. Each asks again when its item changes.

import { send as defaultSend } from '$lib/protocol/api';
import { ProtocolError } from '$lib/protocol/errors';
import type { Embed, Invalidation, Send } from '$lib/protocol/types';
import { parseItemLink, type ItemKind } from '$lib/format/itemLinks';
import { Resource } from './resource.svelte';

/** What a card shows: the item, null once it is deleted or never was. */
export type EmbedResource = Resource<Embed | null, string>;

export class EmbedsStore {
	#send: Send;
	/** Each link's embed and how many cards show it. */
	// Bookkeeping only; each card renders from its own `Resource`.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	#shown = new Map<string, { embed: EmbedResource; cards: number }>();

	constructor(send: Send) {
		this.#send = send;
	}

	/**
	 * The embed for link `href`, loading, for a card to show until it calls
	 * the `release` that comes with it.
	 */
	show(href: string): { embed: EmbedResource; release: () => void } {
		let shown = this.#shown.get(href);
		if (!shown) {
			const embed: EmbedResource = new Resource({
				fetch: (link) => this.#resolve(link),
				initial: null,
				matches: (key, link) => touches(key, link, embed.value)
			});
			shown = { embed, cards: 0 };
			this.#shown.set(href, shown);
		}
		shown.cards++;
		void shown.embed.load(href);
		let released = false;
		const entry = shown;
		return {
			embed: entry.embed,
			release: () => {
				if (released) return;
				released = true;
				if (--entry.cards === 0) entry.embed.release();
			}
		};
	}

	async #resolve(link: string): Promise<Embed | null> {
		try {
			return await this.#send('resolveLink', { link });
		} catch (error) {
			if (error instanceof ProtocolError && error.code === 'notFound') return null;
			throw error;
		}
	}
}

/**
 * Whether invalidation `key` may change what `link` shows: its item's own
 * key once it is resolved, else any of its kind's.
 */
export function touches(key: Invalidation, link: string, embed: Embed | null): boolean {
	if (embed !== null) {
		const ws = embed.workspace_id;
		const workspaceGone = key.kind === 'workspace' && key.id === ws;
		const { item } = embed;
		switch (item.kind) {
			case 'task':
				return workspaceGone || (key.kind === 'taskList' && key.id.workspace_id === ws);
			case 'document':
				return workspaceGone || (key.kind === 'documents' && key.id === ws);
			case 'resource':
				return workspaceGone || (key.kind === 'resources' && key.id === ws);
			case 'contact':
				return workspaceGone || (key.kind === 'contacts' && key.id === ws);
			case 'agent':
				return key.kind === 'agent' && key.id === item.agent.id;
			case 'actorSession':
				return key.kind === 'actorSession' && key.id === item.session.id;
			case 'article':
				return key.kind === 'feedArticles';
			// A folder's rows and columns, or its title and whether it is a table.
			case 'documentFolder':
				return (
					workspaceGone ||
					(key.kind === 'documentTable' && key.id === item.folder.id) ||
					(key.kind === 'documentFolders' && key.id === ws)
				);
		}
	}
	const kind = parseItemLink(link)?.kind;
	return kind !== undefined && key.kind === KEYS[kind];
}

/** The invalidation that says an item of a kind changed somewhere. */
const KEYS: Record<ItemKind, Invalidation['kind']> = {
	task: 'taskList',
	document: 'documents',
	resource: 'resources',
	'document-folder': 'documentFolders',
	contact: 'contacts',
	agent: 'agent',
	'actor-session': 'actorSession',
	article: 'feedArticles'
};

export function createEmbedsStore(send: Send = defaultSend): EmbedsStore {
	return new EmbedsStore(send);
}

/** Every card's embeds, shared by both halves of a split view. */
export const embeds = createEmbedsStore();
