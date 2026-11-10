// The icon an item shows where it is named by a link, as a card
// (`EmbedCard.svelte`) or a row of Backlinks (`Backlinks.svelte`): its
// kind's, a task's box as it stands, a journal entry's notebook.

import BookUser from '@lucide/svelte/icons/book-user';
import Bot from '@lucide/svelte/icons/bot';
import FileText from '@lucide/svelte/icons/file-text';
import Newspaper from '@lucide/svelte/icons/newspaper';
import NotebookPen from '@lucide/svelte/icons/notebook-pen';
import Package from '@lucide/svelte/icons/package';
import Square from '@lucide/svelte/icons/square';
import SquareCheck from '@lucide/svelte/icons/square-check';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import SquareX from '@lucide/svelte/icons/square-x';
import Table2 from '@lucide/svelte/icons/table-2';
import TableProperties from '@lucide/svelte/icons/table-properties';
import type { Component } from 'svelte';
import type { Embed } from '$lib/protocol/types';
import type { ItemKind } from './routes';

/** Each kind's icon, for a link not yet resolved or whose item is gone. */
export const KIND_ICONS: Record<ItemKind, Component> = {
	task: Square,
	document: FileText,
	resource: Package,
	database: Table2,
	'database-row': TableProperties,
	contact: BookUser,
	agent: Bot,
	'actor-session': SquareTerminal,
	article: Newspaper
};

const EMBED_KIND_ICONS: Record<Embed['kind'], Component> = {
	task: Square,
	document: FileText,
	journalEntry: NotebookPen,
	resource: Package,
	database: Table2,
	databaseRow: TableProperties,
	contact: BookUser,
	agent: Bot,
	actorSession: SquareTerminal,
	article: Newspaper
};

/** `embed`'s icon: a task's box checked or crossed as it stands, else its kind's. */
export function embedIcon(embed: Embed): Component {
	if (embed.item.kind === 'task') {
		const { status } = embed.item.task;
		return status === 'completed' ? SquareCheck : status === 'cancelled' ? SquareX : Square;
	}
	return EMBED_KIND_ICONS[embed.kind];
}
