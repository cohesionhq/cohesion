import { describe, expect, it } from 'vitest';
import type { DocumentFolder, DocumentFolderRepresentation } from '$lib/protocol/types';
import {
	folderPath,
	folderTree,
	listedIn,
	siblings,
	subtreeIds,
	tableIds,
	visibleNodes
} from './folderTree';

const folder = (
	id: number,
	title: string,
	parent_id: number | null,
	sort_order: number,
	representation: DocumentFolderRepresentation = 'list'
): DocumentFolder => ({
	id,
	workspace_id: 1,
	parent_id,
	title,
	sort_order,
	deleted: false,
	representation
});

// Work › Plans › Old, and Home beside Work, listed out of order.
const folders = [
	folder(3, 'Old', 2, 0),
	folder(4, 'Home', null, 0),
	folder(2, 'Plans', 1, 0),
	folder(1, 'Work', null, 1)
];

describe('folder tree', () => {
	it('nests folders under their parents in their order', () => {
		const tree = folderTree(folders);
		expect(tree.map((n) => n.folder.title)).toEqual(['Home', 'Work']);
		expect(tree[1].children[0].folder.title).toBe('Plans');
		expect(tree[1].children[0].children[0]).toMatchObject({ depth: 2, folder: { title: 'Old' } });
	});

	it('puts a folder whose parent is gone at the top', () => {
		expect(folderTree([folder(2, 'Plans', 9, 0)])[0].depth).toBe(0);
	});

	it('lists the open folders in reading order', () => {
		const tree = folderTree(folders);
		expect(visibleNodes(tree, new Set()).map((n) => n.folder.title)).toEqual([
			'Home',
			'Work',
			'Plans',
			'Old'
		]);
		expect(visibleNodes(tree, new Set([1])).map((n) => n.folder.title)).toEqual(['Home', 'Work']);
	});

	it('finds what sits inside a folder and the path down to one', () => {
		expect(subtreeIds(folders, 1).sort()).toEqual([1, 2, 3]);
		expect(subtreeIds(folders, 4)).toEqual([4]);
		expect(folderPath(folders, 3).map((f) => f.title)).toEqual(['Work', 'Plans', 'Old']);
		expect(folderPath(folders, 9)).toEqual([]);
		expect(siblings(folders, folders[3]).map((f) => f.title)).toEqual(['Home', 'Work']);
	});

	it('leaves the rows of tables out of All, unless they are searched for', () => {
		const tables = tableIds([...folders, folder(5, 'Books', null, 2, 'table')]);
		expect([...tables]).toEqual([5]);
		const inbox = { folder_id: null };
		const plain = { folder_id: 1 };
		const row = { folder_id: 5 };
		expect([inbox, plain, row].map((d) => listedIn(d, 'all', tables))).toEqual([true, true, false]);
		expect(listedIn(row, 'all', tables, true)).toBe(true);
		expect(listedIn(row, 5, tables)).toBe(true);
		expect(listedIn(row, 'inbox', tables)).toBe(false);
		expect(listedIn(inbox, 'inbox', tables)).toBe(true);
		expect(listedIn(plain, 5, tables)).toBe(false);
	});
});
