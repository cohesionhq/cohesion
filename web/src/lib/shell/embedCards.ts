// Cards mounted into markup Svelte doesn't draw, the editor's lines, which
// hand each card a host element and unmount it with the function this
// returns; and what the editor looks up after `@`.

import { mount, unmount } from 'svelte';
import { send } from '$lib/protocol/api';
import { mainStores } from '$lib/stores/pageStores.svelte';
import type { EmbedMount, MentionSearch } from '$lib/ui/Editor.svelte';
import EmbedCard from './EmbedCard.svelte';
import { searchMentions } from './mentions';

const { agents } = mainStores;

/** Shows link `href` (with text `label`) as a card inside `host`, until the returned function runs. */
export function mountEmbedCard(host: HTMLElement, href: string, label: string): () => void {
	const card = mount(EmbedCard, { target: host, props: { href, label } });
	return () => void unmount(card);
}

/**
 * What the editor needs to show cards and look items up after `@` in
 * workspace `ws`'s text: the card, and the search (`searchMentions`).
 */
export function editorLinks(ws: number): { embed: EmbedMount; mentions: MentionSearch } {
	return {
		embed: mountEmbedCard,
		mentions: async (query) => {
			await agents.loadList().catch(() => undefined);
			return searchMentions(send, ws, query, agents.list.value);
		}
	};
}
