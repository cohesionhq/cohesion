// The icon an item shows where it is named by a link, as a card
// (`EmbedCard.svelte`) or a row of Backlinks (`Backlinks.svelte`): its
// kind's, a task's box as it stands, a journal entry's notebook, a table's
// grid.

import BookUser from '@lucide/svelte/icons/book-user';
import Bot from '@lucide/svelte/icons/bot';
import Folder from '@lucide/svelte/icons/folder';
import FileText from '@lucide/svelte/icons/file-text';
import Newspaper from '@lucide/svelte/icons/newspaper';
import NotebookPen from '@lucide/svelte/icons/notebook-pen';
import Package from '@lucide/svelte/icons/package';
import Square from '@lucide/svelte/icons/square';
import SquareCheck from '@lucide/svelte/icons/square-check';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import SquareX from '@lucide/svelte/icons/square-x';
import Table2 from '@lucide/svelte/icons/table-2';
import type { Component } from 'svelte';
import type { Embed } from '$lib/protocol/types';
import { itemKindOf, type ItemKind } from './routes';

/** Each kind's icon, for a link not yet resolved or whose item is gone. */
export const KIND_ICONS: Record<ItemKind, Component> = {
	task: Square,
	document: FileText,
	resource: Package,
	'document-folder': Folder,
	contact: BookUser,
	agent: Bot,
	'actor-session': SquareTerminal,
	article: Newspaper
};

/** `embed`'s icon: a task's box checked or crossed as it stands, a table's grid, else its kind's. */
export function embedIcon(embed: Embed): Component {
	if (embed.item.kind === 'documentFolder' && embed.item.folder.is_table) return Table2;
	if (embed.item.kind === 'task') {
		const { status } = embed.item.task;
		return status === 'completed' ? SquareCheck : status === 'cancelled' ? SquareX : Square;
	}
	if (embed.kind === 'journalEntry') return NotebookPen;
	return KIND_ICONS[itemKindOf(embed.kind)];
}
