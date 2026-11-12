import { describe, expect, it } from 'vitest';
import type { DocumentFolder, SavedTaskView, TaskFolder } from '$lib/protocol/types';
import { routes } from './routes';
import { documentFolderRows, savedViewRows, taskFolderRows } from './sidebarFolders';

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
	...extra
});

const taskFolder = (
	id: number,
	title: string,
	sort_order: number,
	deleted = false
): TaskFolder => ({
	id,
	workspace_id: 1,
	title,
	sort_order,
	deleted
});

describe('documentFolderRows', () => {
	const folders = [
		docFolder(2, 'Research', null, 2),
		docFolder(1, 'Design', null, 1),
		docFolder(3, 'Search', 2, 1),
		docFolder(4, 'Reading list', null, 3, { representation: 'table' }),
		docFolder(5, 'Old', null, 4, { deleted: true })
	];

	it('lists the Inbox, then live folders in reading order, nested ones a step in', () => {
		const rows = documentFolderRows(1, folders, '/', '');
		expect(rows.map((r) => [r.label, r.depth, r.table])).toEqual([
			['Inbox', 0, false],
			['Design', 0, false],
			['Research', 0, false],
			['Search', 1, false],
			['Reading list', 0, true]
		]);
		expect(rows[4].href).toBe(routes.documentFolder(1, 4));
	});

	it('marks the folder the Documents page shows', () => {
		const rows = documentFolderRows(1, folders, routes.documents(1), '?folder=3');
		expect(rows.filter((r) => r.current).map((r) => r.label)).toEqual(['Search']);
	});

	it('marks the Inbox while the Documents page shows it', () => {
		const rows = documentFolderRows(1, folders, routes.documents(1), '?folder=inbox');
		expect(rows.filter((r) => r.current).map((r) => r.label)).toEqual(['Inbox']);
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
		taskFolder(3, 'Gone', 3, true)
	];

	it('lists the Inbox, then live folders in the order listed, as a reorder leaves them', () => {
		const rows = taskFolderRows(1, folders, '/', '');
		expect(rows.map((r) => r.label)).toEqual(['Inbox', 'WebUI', 'Core']);
		expect(rows[1].href).toBe(`${routes.tasks(1)}?view=2`);
	});

	it('marks the view the Tasks page shows', () => {
		expect(
			taskFolderRows(1, folders, routes.tasks(1), '?view=inbox')
				.filter((r) => r.current)
				.map((r) => r.label)
		).toEqual(['Inbox']);
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

describe('savedViewRows', () => {
	const view = (id: number, name: string, extra: Partial<SavedTaskView> = {}): SavedTaskView => ({
		id,
		workspace_id: 1,
		name,
		scope: 'all',
		tags: { with: [], without: [] },
		priorities: [],
		sizes: [],
		sort: 'created',
		descending: false,
		sort_order: id,
		...extra
	});

	it('opens each view on the list, tags, priorities and sizes it keeps', () => {
		const rows = savedViewRows(
			1,
			[view(1, 'Plain'), view(2, 'Tagged', { scope: 3, tags: { with: [7], without: [8] } })],
			routes.tasks(1),
			''
		);
		expect(rows.map((r) => [r.label, r.href])).toEqual([
			['Plain', `${routes.tasks(1)}?view=all`],
			['Tagged', `${routes.tasks(1)}?view=3&tag=7&without=8`]
		]);
	});

	it('marks the view whose address the page has', () => {
		const rows = savedViewRows(1, [view(1, 'Plain')], routes.tasks(1), '?view=all');
		expect(rows[0].current).toBe(true);
		expect(savedViewRows(1, [view(1, 'Plain')], routes.tasks(1), '?view=done')[0].current).toBe(
			false
		);
	});
});
