// The document folders as a tree: the daemon lists them flat, each naming its parent.

import type { Document, DocumentFolder } from '$lib/protocol/types';

export interface FolderNode {
	folder: DocumentFolder;
	/** How many folders it sits inside; 0 at the top level. */
	depth: number;
	children: FolderNode[];
}

const bySortOrder = (a: DocumentFolder, b: DocumentFolder) =>
	a.sort_order - b.sort_order || a.id - b.id;

/** The top-level folders, each holding its children in their order. A folder whose parent is missing sits at the top. */
export function folderTree(folders: DocumentFolder[]): FolderNode[] {
	const ids = new Set(folders.map((f) => f.id));
	const childrenOf = new Map<number | null, DocumentFolder[]>();
	for (const f of [...folders].sort(bySortOrder)) {
		const parent = f.parent_id !== null && ids.has(f.parent_id) ? f.parent_id : null;
		childrenOf.set(parent, [...(childrenOf.get(parent) ?? []), f]);
	}
	const build = (parent: number | null, depth: number): FolderNode[] =>
		(childrenOf.get(parent) ?? []).map((folder) => ({
			folder,
			depth,
			children: build(folder.id, depth + 1)
		}));
	return build(null, 0);
}

/** The nodes in reading order, leaving out those inside a folder `collapsed` holds. */
export function visibleNodes(tree: FolderNode[], collapsed: Set<number>): FolderNode[] {
	return tree.flatMap((node) => [
		node,
		...(collapsed.has(node.folder.id) ? [] : visibleNodes(node.children, collapsed))
	]);
}

/** `root` and the id of every folder inside it. */
export function subtreeIds(folders: DocumentFolder[], root: number): number[] {
	const ids = [root];
	for (let i = 0; i < ids.length; i++)
		for (const f of folders) if (f.parent_id === ids[i]) ids.push(f.id);
	return ids;
}

/** The folders from the top level down to `id`, `id` last; empty when it isn't listed. */
export function folderPath(folders: DocumentFolder[], id: number): DocumentFolder[] {
	const path: DocumentFolder[] = [];
	for (
		let f = folders.find((f) => f.id === id);
		f && !path.includes(f);
		f = folders.find((p) => p.id === f!.parent_id)
	)
		path.unshift(f);
	return path;
}

/** The folders with the same parent as `folder`, in their order, `folder` included. */
export function siblings(folders: DocumentFolder[], folder: DocumentFolder): DocumentFolder[] {
	return folders.filter((f) => f.parent_id === folder.parent_id).sort(bySortOrder);
}

/** What the Documents page lists: the Inbox, every document, or one folder by id. */
export type DocumentView = 'inbox' | 'all' | number;

/**
 * Whether the Documents page lists `doc` in `view`. All leaves out the
 * documents of the folders in `tables`, the rows of tables, so a large table
 * doesn't bury the rest, unless they are being looked for (`searching`).
 */
export function listedIn(
	doc: Pick<Document, 'folder_id'>,
	view: DocumentView,
	tables: ReadonlySet<number>,
	searching = false
): boolean {
	if (view === 'all') return searching || doc.folder_id === null || !tables.has(doc.folder_id);
	return doc.folder_id === (view === 'inbox' ? null : view);
}

/** The ids of the folders that are tables. */
export function tableIds(folders: DocumentFolder[]): Set<number> {
	return new Set(folders.filter((f) => f.is_table).map((f) => f.id));
}
