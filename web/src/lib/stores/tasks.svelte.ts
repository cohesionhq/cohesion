import { send as defaultSend } from '$lib/protocol/api';
import type {
	CreateTaskRequest,
	DayFilter,
	Invalidation,
	ListTasksRequest,
	SavedTaskView,
	Send,
	TagFilter,
	Task,
	TaskAssignee,
	TaskFolder,
	TaskPage,
	TaskPriority,
	TaskSize,
	TaskSort,
	TaskStatus
} from '$lib/protocol/types';
import { Resource } from './resource.svelte';

/** What a new task can start with besides its title and workspace. */
export type NewTaskDetails = Omit<CreateTaskRequest, 'workspace_id' | 'title'>;

/** Which tasks the Tasks page shows: the inbox, everything open, completed or cancelled ones, or a folder. */
export type TaskView = 'inbox' | 'all' | 'done' | 'cancelled' | number;

/** The order the Tasks page lists a view in. */
export interface TaskOrder {
	sort: TaskSort;
	descending: boolean;
}

/** Creation order, oldest first: the daemon's default. */
export const DEFAULT_ORDER: TaskOrder = { sort: 'created', descending: false };

export interface TaskListKey {
	workspace_id: number;
	view: TaskView;
	order: TaskOrder;
	/** The tags the list is narrowed by. */
	tags: TagFilter;
	/** The priorities a listed task has one of; any when empty. */
	priorities: TaskPriority[];
	/** The sizes a listed task has one of; any task, sized or not, when empty. */
	sizes: TaskSize[];
	narrowing: TaskNarrowing;
	/** How many of the view's tasks come before the page. */
	offset: number;
	/** How many tasks the page holds. */
	limit: number;
}

/**
 * What the Dashboard narrows a view by besides the Tasks page's filters:
 * the tasks due on matching days, and those assigned to one assignee.
 */
export interface TaskNarrowing {
	/** Any task, dated or not, when null. */
	due_date: DayFilter | null;
	/** Any task, assigned or not, when null. */
	assignee: TaskAssignee | null;
}

/** No narrowing: every task the view and filters keep. */
export const NO_NARROWING: TaskNarrowing = { due_date: null, assignee: null };

/** Whether two narrowings keep the same tasks. */
function sameNarrowing(a: TaskNarrowing, b: TaskNarrowing): boolean {
	return JSON.stringify(a) === JSON.stringify(b);
}

/** What the Tasks page shows, all of which a saved view keeps. */
export interface TaskFilters {
	view: TaskView;
	tags: TagFilter;
	/** Any priority when empty. */
	priorities: TaskPriority[];
	/** Any task, sized or not, when empty. */
	sizes: TaskSize[];
	order: TaskOrder;
}

/** Whether a saved view keeps exactly `filters`. */
export function savedAs(
	saved: SavedTaskView,
	{ view, tags, priorities, sizes, order }: TaskFilters
): boolean {
	return (
		saved.scope === view &&
		sameTags(saved.tags, tags) &&
		sameSet(saved.priorities, priorities) &&
		sameSet(saved.sizes, sizes) &&
		saved.sort === order.sort &&
		saved.descending === order.descending
	);
}

/** Reads `?view=`; anything unrecognised is the inbox. */
export function parseView(param: string | null): TaskView {
	if (param === 'all' || param === 'done' || param === 'cancelled') return param;
	const id = Number(param);
	return param !== null && Number.isInteger(id) && id > 0 ? id : 'inbox';
}

/** A filter that keeps every task, whatever its tags. */
export const NO_TAGS: TagFilter = { with: [], without: [] };

/** Reads the tags a view is narrowed by from `?tag=` (wanted) and `?without=` (unwanted), each repeatable. */
export function parseTags(params: URLSearchParams): TagFilter {
	const ids = (key: string) =>
		params
			.getAll(key)
			.map(Number)
			.filter((id) => Number.isInteger(id) && id > 0);
	return { with: ids('tag'), without: ids('without') };
}

/** The query string for `tags`, each prefixed with `&`, for appending to a view's link. */
export function tagsQuery({ with: wanted, without }: TagFilter): string {
	return [...wanted.map((id) => `&tag=${id}`), ...without.map((id) => `&without=${id}`)].join('');
}

/** Whether two lists hold the same values, in any order. */
function sameSet<T>(a: T[], b: T[]): boolean {
	return a.length === b.length && a.every((x) => b.includes(x));
}

/** Whether two filters want and refuse the same tags, in any order. */
export function sameTags(a: TagFilter, b: TagFilter): boolean {
	return sameSet(a.with, b.with) && sameSet(a.without, b.without);
}

const PRIORITIES: TaskPriority[] = ['urgent', 'high', 'medium', 'low', 'none'];

/** Reads the priorities a view keeps from `?priority=`, which repeats; any when there are none. */
export function parsePriorities(params: URLSearchParams): TaskPriority[] {
	return PRIORITIES.filter((p) => params.getAll('priority').includes(p));
}

/** The query string for `priorities`, each prefixed with `&`, for appending to a view's link. */
export function prioritiesQuery(priorities: TaskPriority[]): string {
	return priorities.map((p) => `&priority=${p}`).join('');
}

/** Whether a task's priority passes `priorities`. */
export function passesPriorities(task: Task, priorities: TaskPriority[]): boolean {
	return priorities.length === 0 || priorities.includes(task.priority);
}

/** Reads the sizes a view keeps from `?size=`, which repeats; any when there are none. */
export function parseSizes(params: URLSearchParams): TaskSize[] {
	const given = params.getAll('size');
	return ([1, 2, 3, 4, 5] as TaskSize[]).filter((s) => given.includes(String(s)));
}

/** The query string for `sizes`, each prefixed with `&`, for appending to a view's link. */
export function sizesQuery(sizes: TaskSize[]): string {
	return sizes.map((s) => `&size=${s}`).join('');
}

/** The Tasks page's query that opens a saved view: its list, tags, priorities and sizes. */
export function savedViewQuery(v: SavedTaskView): string {
	return `?view=${v.scope}${tagsQuery(v.tags)}${prioritiesQuery(v.priorities)}${sizesQuery(v.sizes)}`;
}

/** Whether a task's size passes `sizes`; a task with no size passes only when `sizes` is empty. */
export function passesSizes(task: Task, sizes: TaskSize[]): boolean {
	return sizes.length === 0 || (task.size !== null && sizes.includes(task.size));
}

/** Whether a task's tags pass `tags`. */
export function passesTags(task: Task, tags: TagFilter): boolean {
	return (
		tags.with.every((id) => task.tag_ids.includes(id)) &&
		!tags.without.some((id) => task.tag_ids.includes(id))
	);
}

const VIEWS_KEY = 'cohesion.taskViews';

function storedViews(): Record<string, unknown> {
	try {
		return JSON.parse(localStorage.getItem(VIEWS_KEY) ?? '{}') as Record<string, unknown>;
	} catch {
		return {};
	}
}

/** The view the Tasks page last showed in a workspace, so a return lands there; the inbox at first. */
export function rememberedView(workspace_id: number): TaskView {
	const stored = storedViews()[workspace_id];
	return stored === undefined ? 'inbox' : parseView(String(stored));
}

export function rememberView(workspace_id: number, view: TaskView): void {
	try {
		localStorage.setItem(VIEWS_KEY, JSON.stringify({ ...storedViews(), [workspace_id]: view }));
	} catch {
		// Remembering the view is a convenience; ignore blocked storage.
	}
}

const ORDER_KEY = 'cohesion.taskOrder';
const SORTS: TaskSort[] = ['created', 'title', 'priority', 'size', 'startDate', 'dueDate'];

/** The order the Tasks page last listed in, in any workspace; creation order at first. */
export function rememberedOrder(): TaskOrder {
	try {
		const stored = JSON.parse(localStorage.getItem(ORDER_KEY) ?? 'null') as Partial<TaskOrder>;
		if (stored && SORTS.includes(stored.sort as TaskSort)) {
			return { sort: stored.sort as TaskSort, descending: stored.descending === true };
		}
	} catch {
		// Fall through to the default.
	}
	return DEFAULT_ORDER;
}

export function rememberOrder(order: TaskOrder): void {
	try {
		localStorage.setItem(ORDER_KEY, JSON.stringify(order));
	} catch {
		// Remembering the order is a convenience; ignore blocked storage.
	}
}

const IDS_KEY = 'cohesion.showTaskIds';

/** Whether rows show their task ID, in any workspace; off at first. */
export function rememberedShowIds(): boolean {
	try {
		return localStorage.getItem(IDS_KEY) === 'true';
	} catch {
		return false;
	}
}

export function rememberShowIds(show: boolean): void {
	try {
		localStorage.setItem(IDS_KEY, String(show));
	} catch {
		// Remembering the setting is a convenience; ignore blocked storage.
	}
}

/** The `listTasks` args for a view, narrowed by `tags`, `priorities`, `sizes` and `narrowing`. */
export function viewArgs(
	workspace_id: number,
	view: TaskView,
	{ sort, descending }: TaskOrder = DEFAULT_ORDER,
	tags: TagFilter = NO_TAGS,
	priorities: TaskPriority[] = [],
	sizes: TaskSize[] = [],
	{ due_date, assignee }: TaskNarrowing = NO_NARROWING
): ListTasksRequest {
	return {
		workspace_id,
		folder: typeof view === 'number' || view === 'inbox' ? view : 'any',
		statuses: [viewStatus(view)],
		sort,
		descending,
		tags,
		priorities: priorities.length === 0 ? null : priorities,
		sizes: sizes.length === 0 ? null : sizes,
		due_date,
		assignee
	};
}

/** The status of the tasks a view lists: Done's are completed, Cancelled's cancelled, the rest open. */
export function viewStatus(view: TaskView): TaskStatus {
	if (view === 'done') return 'completed';
	if (view === 'cancelled') return 'cancelled';
	return 'incomplete';
}

/** Whether a view spans folders: All, Done and Cancelled. */
export function spansFolders(view: TaskView): boolean {
	return view === 'all' || view === 'done' || view === 'cancelled';
}

/** The view that lists `task`: its folder (or the inbox) while open, else Done or Cancelled. */
export function viewOf(task: Task): TaskView {
	if (task.status === 'completed') return 'done';
	if (task.status === 'cancelled') return 'cancelled';
	return task.task_folder_id ?? 'inbox';
}

/** The folder a task created in this view goes into. */
export function viewFolder(view: TaskView): number | null {
	return typeof view === 'number' ? view : null;
}

/** Whether a task's folder still places it in `view` (status aside). */
export function inViewFolder(view: TaskView, task: Task): boolean {
	if (view === 'inbox') return task.task_folder_id === null;
	if (typeof view === 'number') return task.task_folder_id === view;
	return true;
}

/** Whether `key` changes a task list the view shows: its own list, or any list for a view spanning folders. */
export function listTouchedBy(key: Invalidation, { workspace_id, view }: TaskListKey): boolean {
	if (key.kind !== 'taskList' || key.id.workspace_id !== workspace_id) return false;
	if (spansFolders(view)) return true;
	return key.id.folder_id === viewFolder(view);
}

/** A workspace and the first day after the ones due ("2026-11-08"): `workspace:day`. */
export type DueKey = `${number}:${string}`;

/** Folders, saved views and one page of the task list for one view of a workspace; and its open tasks due by today. */
export class TasksStore {
	readonly folders: Resource<TaskFolder[], number>;
	readonly views: Resource<SavedTaskView[], number>;
	readonly list: Resource<TaskPage, TaskListKey>;
	/** A workspace's open tasks due today or earlier, soonest first: what the Dashboard's Today lists. */
	readonly due: Resource<TaskPage, DueKey>;

	#send: Send;

	constructor(send: Send) {
		this.#send = send;
		this.folders = new Resource({
			fetch: (workspace_id) => send('listTaskFolders', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'taskFolders' && key.id === workspace_id
		});
		this.views = new Resource({
			fetch: (workspace_id) => send('listSavedTaskViews', { workspace_id }),
			initial: [],
			matches: (key, workspace_id) => key.kind === 'savedTaskViews' && key.id === workspace_id
		});
		this.list = new Resource({
			fetch: ({ workspace_id, view, order, tags, priorities, sizes, narrowing, offset, limit }) =>
				send('listTasks', {
					...viewArgs(workspace_id, view, order, tags, priorities, sizes, narrowing),
					offset,
					limit
				}),
			initial: { tasks: [], total: 0, offset: 0 },
			matches: listTouchedBy,
			// A filter, the order or the page changing swaps the rows in place.
			keepsValue: (from, to) => from.workspace_id === to.workspace_id && from.view === to.view
		});
		this.due = new Resource({
			fetch: (key) => {
				const [workspace, day] = key.split(':');
				return send('listTasks', {
					workspace_id: Number(workspace),
					statuses: ['incomplete'],
					due_date: { before: day },
					sort: 'dueDate',
					limit: 20
				});
			},
			initial: { tasks: [], total: 0, offset: 0 },
			matches: (key, due) =>
				key.kind === 'taskList' && key.id.workspace_id === Number(due.split(':')[0])
		});
	}

	get workspaceId(): number {
		return this.list.key!.workspace_id;
	}

	get view(): TaskView {
		return this.list.key!.view;
	}

	get order(): TaskOrder {
		return this.list.key!.order;
	}

	/**
	 * Loads the folders and one page of a view, narrowed by `tags`,
	 * `priorities`, `sizes` and `narrowing`. The same page of the same view
	 * in the same order with the same filters reuses the loaded list.
	 */
	async open(
		workspace_id: number,
		view: TaskView,
		order = DEFAULT_ORDER,
		tags: TagFilter = NO_TAGS,
		priorities: TaskPriority[] = [],
		sizes: TaskSize[] = [],
		offset = 0,
		limit = 50,
		narrowing = NO_NARROWING
	): Promise<void> {
		const current = this.list.key;
		const key =
			current &&
			current.workspace_id === workspace_id &&
			current.view === view &&
			current.order.sort === order.sort &&
			current.order.descending === order.descending &&
			sameTags(current.tags, tags) &&
			sameSet(current.priorities, priorities) &&
			sameSet(current.sizes, sizes) &&
			sameNarrowing(current.narrowing, narrowing) &&
			current.offset === offset &&
			current.limit === limit
				? current
				: { workspace_id, view, order, tags, priorities, sizes, narrowing, offset, limit };
		await Promise.all([
			this.folders.load(workspace_id),
			this.views.load(workspace_id),
			this.list.load(key)
		]);
	}

	/** Task `id` in a workspace, whatever its status; null once it is deleted. */
	async find(workspace_id: number, id: number): Promise<Task | null> {
		const { tasks } = await this.#send('listTasks', {
			ids: [id],
			workspace_id,
			statuses: ['incomplete', 'completed', 'cancelled']
		});
		return tasks[0] ?? null;
	}

	/** Where task `id` sits in a view's listing, counting from 0; -1 when the view doesn't list it. */
	async indexIn(
		workspace_id: number,
		view: TaskView,
		order: TaskOrder,
		tags: TagFilter,
		priorities: TaskPriority[],
		sizes: TaskSize[],
		id: number
	): Promise<number> {
		const args = viewArgs(workspace_id, view, order, tags, priorities, sizes);
		const { tasks, offset } = await this.#send('listTasks', { ...args, around: id, limit: 500 });
		const index = tasks.findIndex((t) => t.id === id);
		return index === -1 ? -1 : offset + index;
	}

	/**
	 * Creates a task, in the view's folder unless `details` names another. One
	 * the view shows goes last in creation order (first when descending), on
	 * the page that holds that place; else where a refetch puts it.
	 */
	async create(title: string, details: NewTaskDetails = {}): Promise<Task> {
		const task = await this.#send('createTask', {
			workspace_id: this.workspaceId,
			title,
			folder_id: viewFolder(this.view),
			...details
		});
		const { order, offset, limit, tags, priorities, sizes, narrowing } = this.list.key!;
		// A narrowed list (the Dashboard's) takes a new task from its refetch.
		const shown =
			sameNarrowing(narrowing, NO_NARROWING) &&
			viewStatus(this.view) === 'incomplete' &&
			inViewFolder(this.view, task) &&
			passesTags(task, tags) &&
			passesPriorities(task, priorities) &&
			passesSizes(task, sizes);
		if (!shown) return task;
		if (order.sort !== 'created') {
			await this.list.refetch();
			return task;
		}
		this.list.update((page) => {
			const { tasks, total } = page;
			if (order.descending)
				return {
					...page,
					tasks: offset === 0 ? [task, ...tasks].slice(0, limit) : tasks,
					total: total + 1
				};
			return {
				...page,
				tasks: offset + limit > total ? [...tasks, task] : tasks,
				total: total + 1
			};
		});
		return task;
	}

	/**
	 * Changes status in bulk, in one workspace: a linked task may be in
	 * another than the list's. Rows stay where they are until the next refetch.
	 */
	async setStatus(workspace_id: number, task_ids: number[], status: TaskStatus): Promise<Task[]> {
		const changed = await this.#send('changeTasksStatus', {
			workspace_id,
			task_ids,
			status
		});
		this.#replace(changed);
		return changed;
	}

	/** Completes an open task; reopens a finished one. */
	toggle(task: Task): Promise<Task[]> {
		return this.setStatus(
			task.workspace_id,
			[task.id],
			task.status === 'incomplete' ? 'completed' : 'incomplete'
		);
	}

	async rename(task_id: number, title: string): Promise<Task> {
		const task = await this.#send('renameTask', {
			workspace_id: this.workspaceId,
			task_id,
			title
		});
		this.#replace([task]);
		return task;
	}

	/** Sets a task's priority, in its own workspace, as every edit of one task here. */
	async setPriority(task: Task, priority: TaskPriority): Promise<Task> {
		const changed = await this.#send('setTaskPriority', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			priority
		});
		this.#replace([changed]);
		return changed;
	}

	/** Sets or clears a task's size, in its own workspace. */
	async setSize(task: Task, size: TaskSize | null): Promise<Task> {
		const changed = await this.#send('setTaskSize', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			size
		});
		this.#replace([changed]);
		return changed;
	}

	/** Assigns a task to the user, an agent or a contact, or to no one with null, in its own workspace. */
	async setAssignee(task: Task, assignee: TaskAssignee | null): Promise<Task> {
		const changed = await this.#send('setTaskAssignee', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			assignee
		});
		this.#replace([changed]);
		return changed;
	}

	/** Sets a task's description, in its own workspace: a linked task may be in another. */
	async describe(task: Task, description: string): Promise<Task> {
		const described = await this.#send('editTaskDescription', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			description
		});
		this.#replace([described]);
		return described;
	}

	/** Sets both days; null clears one. */
	async schedule(task: Task, start_date: string | null, due_date: string | null): Promise<Task> {
		const changed = await this.#send('scheduleTask', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			start_date,
			due_date
		});
		this.#replace([changed]);
		return changed;
	}

	/** Moves a task; it leaves the list if the view no longer includes its folder. */
	async move(task: Task, folder_id: number | null): Promise<Task> {
		const moved = await this.#send('moveTask', {
			workspace_id: task.workspace_id,
			task_id: task.id,
			folder_id
		});
		const key = this.list.key;
		if (key === undefined || inViewFolder(key.view, moved)) this.#replace([moved]);
		else this.#drop((t) => t.id === moved.id);
		return moved;
	}

	/** Takes a task just tagged or untagged into the list, which drops it once its tags no longer pass the list's. */
	takeTagged(task: Task): void {
		const key = this.list.key;
		if (key === undefined || passesTags(task, key.tags)) this.#replace([task]);
		else this.#drop((t) => t.id === task.id);
	}

	async remove(task_id: number): Promise<Task> {
		const task = await this.#send('deleteTask', { workspace_id: this.workspaceId, task_id });
		this.#drop((t) => t.id === task_id);
		return task;
	}

	async createFolder(workspace_id: number, title: string): Promise<TaskFolder> {
		const folder = await this.#send('createTaskFolder', { workspace_id, title });
		this.folders.update((list) => [...list, folder]);
		return folder;
	}

	async renameFolder(workspace_id: number, folder_id: number, title: string): Promise<TaskFolder> {
		const folder = await this.#send('renameTaskFolder', { workspace_id, folder_id, title });
		this.folders.update((list) => list.map((f) => (f.id === folder_id ? folder : f)));
		return folder;
	}

	/** Pins a folder to the sidebar, or unpins it. */
	async setFolderPinned(workspace_id: number, folder_id: number, pinned: boolean): Promise<void> {
		const folder = await this.#send('setTaskFolderPinned', { workspace_id, folder_id, pinned });
		if (this.folders.key === workspace_id)
			this.folders.update((list) => list.map((f) => (f.id === folder_id ? folder : f)));
	}

	/** Puts the folders in the order of `folder_ids`, showing it before the daemon confirms. */
	async reorderFolders(workspace_id: number, folder_ids: number[]): Promise<void> {
		this.folders.update((list) => folder_ids.map((id) => list.find((f) => f.id === id)!));
		try {
			this.folders.set(await this.#send('reorderTaskFolders', { workspace_id, folder_ids }));
		} catch (error) {
			void this.folders.refetch();
			throw error;
		}
	}

	/** Deletes a folder and, as the daemon does, the tasks in it. */
	async deleteFolder(workspace_id: number, folder_id: number): Promise<TaskFolder> {
		const folder = await this.#send('deleteTaskFolder', { workspace_id, folder_id });
		this.folders.update((list) => list.filter((f) => f.id !== folder_id));
		// The daemon deletes a folder's tasks with it, and the views that list it.
		this.#drop((t) => t.task_folder_id === folder_id);
		this.views.update((list) => list.filter((v) => v.scope !== folder_id));
		return folder;
	}

	/** Saves `filters` as a view named `name`, last among the workspace's views. */
	async createView(
		workspace_id: number,
		name: string,
		filters: TaskFilters
	): Promise<SavedTaskView> {
		const view = await this.#send('createSavedTaskView', {
			workspace_id,
			name,
			...viewFilterArgs(filters)
		});
		this.views.update((list) => [...list, view]);
		return view;
	}

	/** Makes saved view `view_id` keep `filters` instead. */
	async updateView(
		workspace_id: number,
		view_id: number,
		filters: TaskFilters
	): Promise<SavedTaskView> {
		const view = await this.#send('updateSavedTaskView', {
			workspace_id,
			view_id,
			...viewFilterArgs(filters)
		});
		this.views.update((list) => list.map((v) => (v.id === view_id ? view : v)));
		return view;
	}

	async renameView(workspace_id: number, view_id: number, name: string): Promise<SavedTaskView> {
		const view = await this.#send('renameSavedTaskView', { workspace_id, view_id, name });
		this.views.update((list) => list.map((v) => (v.id === view_id ? view : v)));
		return view;
	}

	/** Puts the saved views in the order of `view_ids`, showing it before the daemon confirms. */
	async reorderViews(workspace_id: number, view_ids: number[]): Promise<void> {
		this.views.update((list) => view_ids.map((id) => list.find((v) => v.id === id)!));
		try {
			this.views.set(await this.#send('reorderSavedTaskViews', { workspace_id, view_ids }));
		} catch (error) {
			void this.views.refetch();
			throw error;
		}
	}

	async deleteView(workspace_id: number, view_id: number): Promise<SavedTaskView> {
		const view = await this.#send('deleteSavedTaskView', { workspace_id, view_id });
		this.views.update((list) => list.filter((v) => v.id !== view_id));
		return view;
	}

	#replace(changed: Task[]): void {
		this.list.update((page) => ({
			...page,
			tasks: page.tasks.map((t) => changed.find((c) => c.id === t.id) ?? t)
		}));
	}

	/** Takes the page's tasks matching `gone` out; the page keeps its place until the next listing, and only the total follows. */
	#drop(gone: (task: Task) => boolean): void {
		this.list.update((page) => {
			const kept = page.tasks.filter((t) => !gone(t));
			return { ...page, tasks: kept, total: page.total - (page.tasks.length - kept.length) };
		});
	}
}

function viewFilterArgs({ view, tags, priorities, sizes, order }: TaskFilters) {
	return { scope: view, tags, priorities, sizes, sort: order.sort, descending: order.descending };
}

export function createTasksStore(send: Send = defaultSend): TasksStore {
	return new TasksStore(send);
}
