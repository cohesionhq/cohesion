// "Copy link" on an item: a Markdown link to it, `[Title](cohesion://…)`, ready
// to paste into a document, where on a line of its own it becomes a card; and
// "Copy plain link", the `cohesion://` address alone.

import {
	itemLinkHref,
	itemLinkMarkdown,
	type ItemKind,
	type ItemLink
} from '$lib/format/itemLinks';
import type { EmbedKind } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import type { MenuAction } from './Menu.svelte';
import { copyToClipboard } from './toasts.svelte';

const embedKinds: Record<ItemKind, EmbedKind> = {
	task: 'task',
	document: 'document',
	resource: 'resource',
	'document-folder': 'documentFolder',
	agent: 'agent',
	'actor-session': 'actorSession',
	article: 'article'
};

/**
 * Copies `link` as a Markdown link titled `title`, as it is when chosen (an
 * item with no title is called by its kind).
 */
export function copyItemLink(link: ItemLink, title: string): void {
	const named = title.trim() === '' ? vocabulary.embedKind(embedKinds[link.kind]) : title;
	copyToClipboard(itemLinkMarkdown(link, named), 'Copied the link.');
}

/** Copies `link` as its `cohesion://` address. */
export function copyPlainLink(link: ItemLink): void {
	copyToClipboard(itemLinkHref(link), 'Copied the plain link.');
}

/**
 * The two menu items that copy the link to an item: its Markdown link and its
 * plain address. `item` is read when one is chosen, so the title is the live one.
 */
export function copyLinkItems(item: () => { link: ItemLink; title: string }): MenuAction[] {
	return [
		{
			label: 'Copy link',
			onSelect: () => {
				const { link, title } = item();
				copyItemLink(link, title);
			}
		},
		{ label: 'Copy plain link', onSelect: () => copyPlainLink(item().link) }
	];
}
