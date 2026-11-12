// The folders the sidebar lists under Documents and Tasks, and which of them is open.

import { folderTree, type FolderNode } from '$lib/features/documents/folderTree';
import type { DocumentFolder, SavedTaskView, TaskFolder } from '$lib/protocol/types';
import { routes } from '$lib/shell/routes';
import { prioritiesQuery, sizesQuery, tagsQuery } from '$lib/stores/tasks.svelte';

export interface FolderRow {
	key: string;
	/** The folder's id; null for the Inbox, which is no folder. */
	id: number | null;
	href: string;
	label: string;
	/** How many folders it sits inside; 0 at the top level. */
	depth: number;
	/** Whether the page shows this folder. */
	current: boolean;
	/** Whether it is a table (a document folder with properties). */
	table: boolean;
}

/** The Inbox, then the document folders in reading order, each a step in from its parent; current while the Documents page shows it. */
export function documentFolderRows(
	ws: number,
	folders: DocumentFolder[],
	pathname: string,
	search: string
): FolderRow[] {
	const open = pathname === routes.documents(ws) ? new URLSearchParams(search).get('folder') : null;
	const flat = (nodes: FolderNode[]): FolderRow[] =>
		nodes.flatMap(({ folder, depth, children }) => [
			{
				key: `d${folder.id}`,
				id: folder.id,
				href: routes.documentFolder(ws, folder.id),
				label: folder.title,
				depth,
				current: open === String(folder.id),
				table: folder.representation === 'table'
			},
			...flat(children)
		]);
	return [
		{
			key: 'dinbox',
			id: null,
			href: `${routes.documents(ws)}?folder=inbox`,
			label: 'Inbox',
			depth: 0,
			current: open === 'inbox',
			table: false
		},
		...flat(folderTree(folders.filter((f) => !f.deleted)))
	];
}

/** The Inbox, then the task folders in the order listed; current while the Tasks page shows it. */
export function taskFolderRows(
	ws: number,
	folders: TaskFolder[],
	pathname: string,
	search: string
): FolderRow[] {
	const base = routes.tasks(ws);
	const view = pathname === base ? new URLSearchParams(search).get('view') : null;
	const row = (id: string, label: string): FolderRow => ({
		key: `t${id}`,
		id: id === 'inbox' ? null : Number(id),
		href: `${base}?view=${id}`,
		label,
		depth: 0,
		current: view === id,
		table: false
	});
	return [
		row('inbox', 'Inbox'),
		// In the store's order, which a reorder changes before the daemon confirms it.
		...folders.filter((f) => !f.deleted).map((f) => row(String(f.id), f.title))
	];
}

/** The saved task views in their order, each opening the list, tags, priorities and sizes it keeps; current while the page's address is exactly that. */
export function savedViewRows(
	ws: number,
	views: SavedTaskView[],
	pathname: string,
	search: string
): FolderRow[] {
	return views.map((v) => {
		const query = `?view=${v.scope}${tagsQuery(v.tags)}${prioritiesQuery(v.priorities)}${sizesQuery(v.sizes)}`;
		return {
			key: `v${v.id}`,
			id: v.id,
			href: `${routes.tasks(ws)}${query}`,
			label: v.name,
			depth: 0,
			current: pathname === routes.tasks(ws) && search === query,
			table: false
		};
	});
}
