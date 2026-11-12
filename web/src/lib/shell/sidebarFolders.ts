// The pinned folders the sidebar lists under Documents and Tasks, and which of them is open.

import { folderTree, type FolderNode } from '$lib/features/documents/folderTree';
import type { DocumentFolder, TaskFolder } from '$lib/protocol/types';
import { routes } from '$lib/shell/routes';

export interface FolderRow {
	key: string;
	id: number;
	href: string;
	label: string;
	/** Whether the page shows this folder. */
	current: boolean;
	/** Whether it is a table (a document folder with properties). */
	table: boolean;
}

/** The pinned document folders, in reading order; current while the Documents page shows it. */
export function documentFolderRows(
	ws: number,
	folders: DocumentFolder[],
	pathname: string,
	search: string
): FolderRow[] {
	const open = pathname === routes.documents(ws) ? new URLSearchParams(search).get('folder') : null;
	const flat = (nodes: FolderNode[]): DocumentFolder[] =>
		nodes.flatMap(({ folder, children }) => [folder, ...flat(children)]);
	return flat(folderTree(folders.filter((f) => !f.deleted)))
		.filter((f) => f.pinned)
		.map((folder) => ({
			key: `d${folder.id}`,
			id: folder.id,
			href: routes.documentFolder(ws, folder.id),
			label: folder.title,
			current: open === String(folder.id),
			table: folder.representation === 'table'
		}));
}

/** The pinned task folders, in the order listed; current while the Tasks page shows it. */
export function taskFolderRows(
	ws: number,
	folders: TaskFolder[],
	pathname: string,
	search: string
): FolderRow[] {
	const base = routes.tasks(ws);
	const view = pathname === base ? new URLSearchParams(search).get('view') : null;
	return folders
		.filter((f) => !f.deleted && f.pinned)
		.map((f) => ({
			key: `t${f.id}`,
			id: f.id,
			href: `${base}?view=${f.id}`,
			label: f.title,
			current: view === String(f.id),
			table: false
		}));
}
