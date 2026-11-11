// The folders the sidebar lists under Documents and Tasks, and which of them is open.

import { folderTree, type FolderNode } from '$lib/features/documents/folderTree';
import type { DocumentFolder, TaskFolder } from '$lib/protocol/types';
import { routes } from '$lib/shell/routes';

export interface FolderRow {
	key: string;
	href: string;
	label: string;
	/** How many folders it sits inside; 0 at the top level. */
	depth: number;
	/** Whether the page shows this folder. */
	current: boolean;
	/** Whether it is a table (a document folder with properties). */
	table: boolean;
}

/** The document folders in reading order, each a step in from its parent; current while the Documents page shows it. */
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
				href: routes.documentFolder(ws, folder.id),
				label: folder.title,
				depth,
				current: open === String(folder.id),
				table: folder.representation === 'table'
			},
			...flat(children)
		]);
	return flat(folderTree(folders.filter((f) => !f.deleted)));
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
