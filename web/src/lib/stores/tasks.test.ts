import { describe, expect, it } from 'vitest';
import { localDay } from '$lib/format/dates';
import { createMock } from '$lib/protocol/mock';
import {
	createTasksStore,
	DEFAULT_ORDER,
	inViewFolder,
	listTouchedBy,
	NO_NARROWING,
	NO_TAGS,
	parsePriorities,
	parseSizes,
	parseTags,
	parseView,
	prioritiesQuery,
	savedAs,
	sizesQuery,
	tagsQuery,
	viewArgs,
	viewFolder
} from './tasks.svelte';

describe('task views', () => {
	it('parses the view param, falling back to the inbox', () => {
		expect(parseView(null)).toBe('inbox');
		expect(parseView('all')).toBe('all');
		expect(parseView('done')).toBe('done');
		expect(parseView('cancelled')).toBe('cancelled');
		expect(parseView('7')).toBe(7);
		expect(parseView('nope')).toBe('inbox');
		expect(parseView('-1')).toBe('inbox');
	});

	it('reads and writes the tags a view wants and refuses', () => {
		expect(parseTags(new URLSearchParams('tag=1&tag=x&without=3&tag=2'))).toEqual({
			with: [1, 2],
			without: [3]
		});
		expect(parseTags(new URLSearchParams(''))).toEqual(NO_TAGS);
		expect(tagsQuery({ with: [1, 2], without: [3] })).toBe('&tag=1&tag=2&without=3');
	});

	it('reads and writes the priorities a view keeps, and lists only those', async () => {
		expect(
			parsePriorities(new URLSearchParams('priority=low&priority=nope&priority=urgent'))
		).toEqual(['urgent', 'low']);
		expect(prioritiesQuery(['urgent', 'none'])).toBe('&priority=urgent&priority=none');
		expect(viewArgs(1, 'all', DEFAULT_ORDER, NO_TAGS, [])).toMatchObject({ priorities: null });

		const store = createTasksStore(createMock().send);
		await store.open(1, 'all');
		const [first] = store.list.value.tasks;
		await store.setPriority(first, 'urgent');
		await store.open(1, 'all', DEFAULT_ORDER, NO_TAGS, ['urgent']);
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([first.id]);
	});

	it('reads and writes the sizes a view keeps, and lists only tasks of those sizes', async () => {
		expect(parseSizes(new URLSearchParams('size=4&size=9&size=x&size=1'))).toEqual([1, 4]);
		expect(sizesQuery([2, 5])).toBe('&size=2&size=5');
		expect(viewArgs(1, 'all')).toMatchObject({ sizes: null });

		const store = createTasksStore(createMock().send);
		await store.open(1, 'all');
		const [first, second] = store.list.value.tasks;
		await store.setSize(first, 5);
		await store.setSize(second, 2);
		await store.open(1, 'all', { sort: 'size', descending: false });
		// Sized tasks first, smallest up (the seed has small tasks too), then the unsized.
		const sized = store.list.value.tasks.map((t) => t.size);
		expect(sized.slice(0, 3)).toEqual([2, 2, 2]);
		expect(sized.filter((s) => s !== null).at(-1)).toBe(5);
		expect(sized.at(-1)).toBeNull();
		await store.open(1, 'all', DEFAULT_ORDER, NO_TAGS, [], [5]);
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([first.id]);
		expect((await store.setSize(first, null)).size).toBeNull();
	});

	it('assigns a task to the user or an agent, and to no one again', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'all');
		const [first] = store.list.value.tasks;
		const agent = { kind: 'agent', agent_id: 1 } as const;
		expect((await store.setAssignee(first, agent)).assignee).toEqual(agent);
		expect(store.list.value.tasks[0].assignee).toEqual(agent);
		expect((await store.setAssignee(first, { kind: 'user' })).assignee).toEqual({ kind: 'user' });
		expect((await store.setAssignee(first, null)).assignee).toBeNull();
		await expect(store.setAssignee(first, { kind: 'agent', agent_id: 999 })).rejects.toThrow();
	});

	it('narrows a view to one assignee and to due days', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		const mine = { due_date: null, assignee: { kind: 'user' } } as const;
		await store.open(1, 'all', DEFAULT_ORDER, NO_TAGS, [], [], 0, 50, mine);
		expect(store.list.value.tasks.map((t) => t.title)).toEqual([
			'Configure and deploy the Linear connector'
		]);

		const first = store.list.value.tasks[0];
		await store.schedule(first, null, '2026-01-02');
		await store.open(1, 'all', DEFAULT_ORDER, NO_TAGS, [], [], 0, 50, {
			due_date: { before: '2026-01-03' },
			assignee: null
		});
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([first.id]);
	});

	it('maps each view onto listTasks args and a folder for new tasks', () => {
		expect(viewArgs(1, 'inbox')).toEqual({
			workspace_id: 1,
			folder: 'inbox',
			statuses: ['incomplete'],
			sort: 'created',
			descending: false,
			tags: { with: [], without: [] },
			priorities: null,
			sizes: null,
			due_date: null,
			assignee: null
		});
		expect(viewArgs(1, 'inbox', { sort: 'dueDate', descending: true })).toMatchObject({
			sort: 'dueDate',
			descending: true
		});
		expect(viewArgs(1, 'all')).toMatchObject({ folder: 'any', statuses: ['incomplete'] });
		expect(viewArgs(1, 'done')).toMatchObject({
			folder: 'any',
			statuses: ['completed']
		});
		expect(viewArgs(1, 'cancelled')).toMatchObject({ folder: 'any', statuses: ['cancelled'] });
		expect(viewArgs(1, 3)).toMatchObject({ folder: 3 });
		expect(viewFolder('inbox')).toBeNull();
		expect(viewFolder('done')).toBeNull();
		expect(viewFolder(3)).toBe(3);
	});

	it('refetches a view only for its own list, All, Done and Cancelled for any list', () => {
		const inbox = { kind: 'taskList', id: { workspace_id: 1, folder_id: null } } as const;
		const folder = { kind: 'taskList', id: { workspace_id: 1, folder_id: 2 } } as const;
		const order = { sort: 'created', descending: false } as const;
		const key = (view: Parameters<typeof viewFolder>[0], workspace_id = 1) => ({
			workspace_id,
			view,
			order,
			tags: NO_TAGS,
			priorities: [],
			sizes: [],
			narrowing: NO_NARROWING,
			offset: 0,
			limit: 50
		});
		expect(listTouchedBy(inbox, key('inbox'))).toBe(true);
		expect(listTouchedBy(folder, key('inbox'))).toBe(false);
		expect(listTouchedBy(folder, key(2))).toBe(true);
		expect(listTouchedBy(folder, key(3))).toBe(false);
		expect(listTouchedBy(folder, key('all'))).toBe(true);
		expect(listTouchedBy(inbox, key('done'))).toBe(true);
		expect(listTouchedBy(folder, key('cancelled'))).toBe(true);
		expect(listTouchedBy(inbox, key('all', 2))).toBe(false);
		expect(listTouchedBy({ kind: 'taskFolders', id: 1 }, key('all'))).toBe(false);
	});

	it('knows which folders a view holds', () => {
		const task = { task_folder_id: 2 } as Parameters<typeof inViewFolder>[1];
		expect(inViewFolder('inbox', task)).toBe(false);
		expect(inViewFolder(2, task)).toBe(true);
		expect(inViewFolder('all', task)).toBe(true);
	});
});

/** The seeded first workspace's folders, in order. */
const FOLDERS = ['Core', 'WebUI', 'Plus', 'Cloud', 'JavaScript', 'WebUI Harnesses'];

describe('tasks store', () => {
	it('loads a view and its folders', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		expect(store.list.value.tasks.map((t) => t.title)).toEqual([
			'Configure and deploy the Linear connector',
			'Cohesion-bench'
		]);
		expect(store.folders.value.map((f) => f.title)).toEqual(FOLDERS);

		await store.open(1, 'done');
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([4, 7, 16, 21]);

		await store.open(1, 'cancelled');
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([5]);
	});

	it('lists a page of a view with the view’s total, and finds a task’s place in it', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox', undefined, NO_TAGS, [], [], 1, 1);
		expect(store.list.value.tasks.map((t) => t.title)).toEqual(['Cohesion-bench']);
		expect(store.list.value.total).toBe(2);
		const [task] = store.list.value.tasks;
		expect(await store.indexIn(1, 'inbox', DEFAULT_ORDER, NO_TAGS, [], [], task.id)).toBe(1);
		expect(await store.indexIn(1, 'done', DEFAULT_ORDER, NO_TAGS, [], [], task.id)).toBe(-1);

		await store.remove(task.id);
		expect(store.list.value).toEqual({ tasks: [], total: 1 });
	});

	it('reopening the same view keeps the loaded list', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		await store.open(1, 1);
		const key = store.list.key;
		await mock.send('createTask', { workspace_id: 1, title: 'Elsewhere', folder_id: 1 });
		await store.open(1, 1);
		expect(store.list.key).toBe(key);
		expect(store.list.value.tasks.some((t) => t.title === 'Elsewhere')).toBe(false);
	});

	it('creates into the current folder', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		await store.open(1, 2);
		const task = await store.create('Post a letter');
		expect(task.task_folder_id).toBe(2);
		expect(store.list.value.tasks.at(-1)).toEqual(task);
	});

	it('creates with tags, and leaves a view a new task isn’t in alone', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		await store.open(1, 2);
		const before = store.list.value;
		const elsewhere = await store.create('File taxes', { folder_id: null, priority: 'high' });
		expect(elsewhere.task_folder_id).toBeNull();
		expect(elsewhere.priority).toBe('high');
		expect(store.list.value).toBe(before);

		const tag = await mock.send('createTag', { workspace_id: 1, name: 'Home' });
		const tagged = await store.create('Fix the tap', { tag_ids: [tag.id] });
		expect(tagged.tag_ids).toEqual([tag.id]);
		expect(store.list.value.tasks.at(-1)).toEqual(tagged);
	});

	it('lists in the chosen order and places a new task where that order puts it', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox', { sort: 'title', descending: false });
		expect(store.list.value.tasks.map((t) => t.title)).toEqual([
			'Cohesion-bench',
			'Configure and deploy the Linear connector'
		]);
		await store.create('Collect feedback');
		expect(store.list.value.tasks.map((t) => t.title)).toEqual([
			'Cohesion-bench',
			'Collect feedback',
			'Configure and deploy the Linear connector'
		]);

		await store.open(1, 'inbox', { sort: 'created', descending: true });
		const task = await store.create('Newest');
		expect(store.list.value.tasks[0]).toEqual(task);
	});

	it('toggling applies the changed tasks from the reply in place', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		const [first] = store.list.value.tasks;
		await store.toggle(first);
		expect(store.list.value.tasks[0]).toMatchObject({ id: first.id, status: 'completed' });
		await store.toggle(store.list.value.tasks[0]);
		expect(store.list.value.tasks[0].status).toBe('incomplete');

		await store.setStatus(1, [2], 'cancelled');
		expect(store.list.value.tasks.map((t) => t.status)).toEqual(['incomplete', 'cancelled']);
	});

	it('only the tasks in the reply change', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		await store.open(1, 'inbox');
		// Task 1 is completed elsewhere; the daemon answers with just the tasks whose status changed.
		await mock.send('changeTasksStatus', { workspace_id: 1, task_ids: [1], status: 'completed' });
		const changed = await store.setStatus(1, [1, 2], 'completed');
		expect(changed.map((t) => t.id)).toEqual([2]);
		expect(store.list.value.tasks.map((t) => t.status)).toEqual(['incomplete', 'completed']);
	});

	it('renames, sets priority, description and dates, moves out of the view and deletes from the reply', async () => {
		const mock = createMock();
		const store = createTasksStore(mock.send);
		await store.open(1, 'inbox');

		await store.rename(1, 'Renew passports');
		expect(store.list.value.tasks[0].title).toBe('Renew passports');
		await store.setPriority(store.list.value.tasks[0], 'urgent');
		expect(store.list.value.tasks[0].priority).toBe('urgent');
		await store.describe(store.list.value.tasks[0], 'Bring the old one.');
		expect(store.list.value.tasks[0].description).toBe('Bring the old one.');
		await store.schedule(store.list.value.tasks[0], null, '2026-10-20');
		expect(store.list.value.tasks[0]).toMatchObject({ start_date: null, due_date: '2026-10-20' });

		await store.move(store.list.value.tasks[0], 1);
		expect(store.list.value.tasks.map((t) => t.id)).toEqual([2]);
		expect(mock.state.tasks.find((t) => t.id === 1)?.task_folder_id).toBe(1);

		await store.remove(2);
		expect(store.list.value.tasks).toEqual([]);

		await store.open(1, 'all');
		await store.move(
			store.list.value.tasks.find((t) => t.id === 3)!,
			null
		);
		expect(store.list.value.tasks.find((t) => t.id === 3)?.task_folder_id).toBeNull();
	});

	it('manages folders from replies', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		const folder = await store.createFolder(1, 'Garden');
		expect(store.folders.value.at(-1)).toEqual(folder);
		await store.renameFolder(1, folder.id, 'Allotment');
		expect(store.folders.value.at(-1)?.title).toBe('Allotment');
		await store.deleteFolder(1, folder.id);
		expect(store.folders.value.map((f) => f.title)).toEqual(FOLDERS);

		await store.open(1, 'all');
		await store.deleteFolder(1, 1);
		expect(store.list.value.tasks.some((t) => t.task_folder_id === 1)).toBe(false);
		// The daemon deleted the folder's tasks too, so a refetch agrees.
		await store.list.refetch();
		expect(store.list.value.tasks.some((t) => t.task_folder_id === 1)).toBe(false);
	});

	it('reorders folders, and a new folder goes last', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		const garden = await store.createFolder(1, 'Garden');
		await store.reorderFolders(1, [garden.id, 2, 1, 3, 4, 5, 6]);
		const reordered = ['Garden', 'WebUI', 'Core', ...FOLDERS.slice(2)];
		expect(store.folders.value.map((f) => f.title)).toEqual(reordered);
		await store.createFolder(1, 'Car');
		await store.folders.refetch();
		expect(store.folders.value.map((f) => f.title)).toEqual([...reordered, 'Car']);
	});

	it('accepts empty titles, as the daemon does', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		expect((await store.create('')).title).toBe('');
		expect((await store.createFolder(1, '')).title).toBe('');
	});

	it('a failed rename leaves the list alone', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		await expect(store.rename(999, 'Gone')).rejects.toMatchObject({ code: 'notFound' });
		expect(store.list.value.tasks[0].title).toBe('Configure and deploy the Linear connector');
	});

	it('saves views, finds the one shown, and drops a folder’s with it', async () => {
		const store = createTasksStore(createMock().send);
		await store.open(1, 'inbox');
		const order = { sort: 'dueDate', descending: true } as const;
		const filters = { view: 1, tags: NO_TAGS, priorities: [], sizes: [], order };
		const inFolder = await store.createView(1, 'Home soon', filters);
		await store.createView(1, 'Inbox', {
			view: 'inbox',
			tags: NO_TAGS,
			priorities: ['urgent'],
			sizes: [],
			order: DEFAULT_ORDER
		});
		await expect(store.createView(1, 'home SOON', filters)).rejects.toMatchObject({
			code: 'validationFailed'
		});
		expect(savedAs(inFolder, filters)).toBe(true);
		expect(savedAs(inFolder, { ...filters, order: DEFAULT_ORDER })).toBe(false);
		expect(savedAs(inFolder, { ...filters, priorities: ['high'] })).toBe(false);
		expect(savedAs(inFolder, { ...filters, sizes: [3] })).toBe(false);

		await store.reorderViews(1, [2, inFolder.id]);
		expect(store.views.value.map((v) => v.name)).toEqual(['Inbox', 'Home soon']);

		await store.deleteFolder(1, 1);
		await store.views.refetch();
		expect(store.views.value.map((v) => v.name)).toEqual(['Inbox']);
	});
});

describe('tasks due', () => {
	it("lists a workspace's open tasks due by today, and not the ones due later", async () => {
		const store = createTasksStore(createMock().send);
		await store.due.load(`1:${localDay(-1)}`);
		const titles = store.due.value.tasks.map((t) => t.title);
		expect(titles).toContain('Configure and deploy the Linear connector');
		expect(titles).not.toContain('Cohesion-bench');
	});
});
