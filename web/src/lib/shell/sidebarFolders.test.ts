import { describe, expect, it } from 'vitest';
import type { DocumentFolder, SavedTaskView, TaskFolder } from '$lib/protocol/types';
import { savedViewQuery } from '$lib/stores/tasks.svelte';
import { routes } from './routes';
import { documentFolderRows, taskFolderRows } from './sidebarFolders';

const docFolder = (
	id: number,
	title: string,
	parent_id: number | null,
	sort_order: number,
	extra: Partial<DocumentFolder> = {}
): DocumentFolder => ({
	id,
	workspace_id: 1,
	parent_id,
	title,
	sort_order,
	deleted: false,
	representation: 'list',
	pinned: true,
	...extra
});

const taskFolder = (
	id: number,
	title: string,
	sort_order: number,
	extra: Partial<TaskFolder> = {}
): TaskFolder => ({
	id,
	workspace_id: 1,
	title,
	sort_order,
	deleted: false,
	pinned: true,
	...extra
});

describe('documentFolderRows', () => {
	const folders = [
		docFolder(2, 'Research', null, 2),
		docFolder(1, 'Design', null, 1, { pinned: false }),
		docFolder(3, 'Search', 2, 1),
		docFolder(4, 'Reading list', null, 3, { representation: 'table' }),
		docFolder(5, 'Old', null, 4, { deleted: true })
	];

	it('lists the live pinned folders in reading order, a nested one after its parent', () => {
		const rows = documentFolderRows(1, folders, '/', '');
		expect(rows.map((r) => [r.label, r.table])).toEqual([
			['Research', false],
			['Search', false],
			['Reading list', true]
		]);
		expect(rows[2].href).toBe(routes.documentFolder(1, 4));
	});

	it('marks the folder the Documents page shows', () => {
		const rows = documentFolderRows(1, folders, routes.documents(1), '?folder=3');
		expect(rows.filter((r) => r.current).map((r) => r.label)).toEqual(['Search']);
	});

	it('marks nothing on another page with the same query', () => {
		const rows = documentFolderRows(1, folders, routes.tasks(1), '?folder=3');
		expect(rows.some((r) => r.current)).toBe(false);
	});
});

describe('taskFolderRows', () => {
	const folders = [
		taskFolder(2, 'WebUI', 2),
		taskFolder(1, 'Core', 1),
		taskFolder(4, 'Plus', 4, { pinned: false }),
		taskFolder(3, 'Gone', 3, { deleted: true })
	];

	it('lists the live pinned folders in the order listed, as a reorder leaves them', () => {
		const rows = taskFolderRows(1, folders, '/', '');
		expect(rows.map((r) => r.label)).toEqual(['WebUI', 'Core']);
		expect(rows[0].href).toBe(`${routes.tasks(1)}?view=2`);
	});

	it('marks the folder the Tasks page shows', () => {
		expect(
			taskFolderRows(1, folders, routes.tasks(1), '?view=2')
				.filter((r) => r.current)
				.map((r) => r.label)
		).toEqual(['WebUI']);
	});

	it('marks nothing when the Tasks page opens without a view', () => {
		expect(taskFolderRows(1, folders, routes.tasks(1), '').some((r) => r.current)).toBe(false);
	});
});

describe('savedViewQuery', () => {
	const view = (extra: Partial<SavedTaskView> = {}): SavedTaskView => ({
		id: 1,
		workspace_id: 1,
		name: 'View',
		scope: 'all',
		tags: { with: [], without: [] },
		priorities: [],
		sizes: [],
		sort: 'created',
		descending: false,
		sort_order: 1,
		...extra
	});

	it('opens a view on the list, tags, priorities and sizes it keeps', () => {
		expect(savedViewQuery(view())).toBe('?view=all');
		expect(savedViewQuery(view({ scope: 3, tags: { with: [7], without: [8] } }))).toBe(
			'?view=3&tag=7&without=8'
		);
	});
});
