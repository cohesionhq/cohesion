// Tasks, their folders, saved views and tags.

import { ProtocolError } from '../../errors';
import type { SavedTaskView, Tag, Task } from '../../types';
import { compareTasks, nowSeconds, onDay, paged } from '../support';
import type { Handlers, MockCore } from '../core';

export function tasksHandlers(core: MockCore) {
	const {
		state,
		nextId,
		workspace,
		contactRow,
		task,
		liveFolders,
		folder,
		savedViews,
		savedView,
		ensureViewName,
		ensureViewFilters,
		tagsOf,
		tagOf,
		ensureTagName,
		withTags,
		keepTask,
		agent
	} = core;
	return {
		listTaskFolders: ({ workspace_id }) => liveFolders(workspace_id).map((f) => ({ ...f })),
		createTaskFolder: ({ workspace_id, title }) => {
			workspace(workspace_id);
			const sort_order = Math.max(0, ...liveFolders(workspace_id).map((f) => f.sort_order)) + 1;
			const created = {
				id: nextId(state.folders),
				workspace_id,
				title,
				sort_order,
				deleted: false,
				pinned: false
			};
			state.folders.push(created);
			return { ...created };
		},
		deleteTaskFolder: ({ workspace_id, folder_id }) => {
			const f = folder(workspace_id, folder_id);
			f.deleted = true;
			// The daemon deletes the folder's tasks with it, and the views that list it.
			for (const t of state.tasks)
				if (t.task_folder_id === folder_id) {
					t.deleted = true;
					keepTask(t, 'deleted');
				}
			state.savedTaskViews = state.savedTaskViews.filter((v) => v.scope !== folder_id);
			return { ...f };
		},
		renameTaskFolder: ({ workspace_id, folder_id, title }) => {
			const f = folder(workspace_id, folder_id);
			f.title = title;
			return { ...f };
		},
		setTaskFolderPinned: ({ workspace_id, folder_id, pinned }) => {
			const f = folder(workspace_id, folder_id);
			f.pinned = pinned;
			return { ...f };
		},
		reorderTaskFolders: ({ workspace_id, folder_ids }) => {
			folder_ids.forEach((id, i) => (folder(workspace_id, id).sort_order = i));
			return liveFolders(workspace_id).map((f) => ({ ...f }));
		},
		listSavedTaskViews: ({ workspace_id }) =>
			savedViews(workspace_id).map((v) => structuredClone(v)),
		createSavedTaskView: ({
			workspace_id,
			name,
			scope,
			tags,
			priorities,
			sizes = [],
			sort,
			descending
		}) => {
			workspace(workspace_id);
			ensureViewName(workspace_id, name);
			ensureViewFilters(workspace_id, scope, tags);
			const sort_order = Math.max(0, ...savedViews(workspace_id).map((v) => v.sort_order)) + 1;
			const created: SavedTaskView = {
				id: nextId(state.savedTaskViews),
				workspace_id,
				name,
				scope,
				tags: structuredClone(tags),
				priorities: [...priorities],
				sizes: [...sizes],
				sort,
				descending,
				sort_order
			};
			state.savedTaskViews.push(created);
			return structuredClone(created);
		},
		updateSavedTaskView: ({
			workspace_id,
			view_id,
			scope,
			tags,
			priorities,
			sizes = [],
			sort,
			descending
		}) => {
			const v = savedView(workspace_id, view_id);
			ensureViewFilters(workspace_id, scope, tags);
			Object.assign(v, {
				scope,
				tags: structuredClone(tags),
				priorities: [...priorities],
				sizes: [...sizes],
				sort,
				descending
			});
			return structuredClone(v);
		},
		renameSavedTaskView: ({ workspace_id, view_id, name }) => {
			const v = savedView(workspace_id, view_id);
			ensureViewName(workspace_id, name, view_id);
			v.name = name;
			return structuredClone(v);
		},
		reorderSavedTaskViews: ({ workspace_id, view_ids }) => {
			view_ids.forEach((id, i) => (savedView(workspace_id, id).sort_order = i));
			return savedViews(workspace_id).map((v) => structuredClone(v));
		},
		deleteSavedTaskView: ({ workspace_id, view_id }) => {
			const v = savedView(workspace_id, view_id);
			state.savedTaskViews = state.savedTaskViews.filter((x) => x.id !== view_id);
			return structuredClone(v);
		},
		listTasks: ({
			ids,
			workspace_id,
			folder: filter = 'any',
			statuses,
			priorities = null,
			sizes = null,
			start_date = null,
			due_date = null,
			assignee = null,
			tags = { with: [], without: [] },
			sort = 'created',
			descending = false,
			offset = 0,
			limit = null,
			around = null
		}) => {
			const listed = state.tasks
				.map(withTags)
				.filter(
					(t) =>
						!t.deleted &&
						tags.with.every((id) => t.tag_ids.includes(id)) &&
						!tags.without.some((id) => t.tag_ids.includes(id)) &&
						(priorities == null || priorities.includes(t.priority)) &&
						(sizes == null || (t.size !== null && sizes.includes(t.size))) &&
						(start_date == null || onDay(t.start_date, start_date)) &&
						(due_date == null || onDay(t.due_date, due_date)) &&
						(assignee == null || JSON.stringify(t.assignee) === JSON.stringify(assignee)) &&
						(ids == null || ids.includes(t.id)) &&
						(workspace_id == null || t.workspace_id === workspace_id) &&
						(filter === 'any' ||
							(filter === 'inbox' ? t.task_folder_id === null : t.task_folder_id === filter)) &&
						(statuses == null || statuses.includes(t.status))
				)
				.sort((a, b) => compareTasks(a, b, sort, descending));
			// The page holding `around`, of pages `limit` long, as the daemon finds it.
			const position = around === null ? -1 : listed.findIndex((t) => t.id === around);
			const size = Math.min(limit ?? 50, 500);
			const start = position === -1 ? offset : Math.floor(position / size) * size;
			const { rows, total } = paged(listed, start, limit);
			return { tasks: rows, total, offset: start };
		},
		createTask: ({
			workspace_id,
			title,
			folder_id = null,
			priority = null,
			size = null,
			assignee = null,
			description = null,
			start_date = null,
			due_date = null,
			tag_ids = []
		}) => {
			workspace(workspace_id);
			if (folder_id !== null) folder(workspace_id, folder_id);
			if (assignee?.kind === 'agent') agent(assignee.agent_id);
			if (assignee?.kind === 'contact') contactRow(workspace_id, assignee.contact_id);
			for (const tag_id of tag_ids) tagOf(workspace_id, tag_id);
			const created: Task = {
				id: nextId(state.tasks),
				workspace_id,
				task_folder_id: folder_id,
				title,
				description: description ?? '',
				status: 'incomplete',
				priority: priority ?? 'none',
				size,
				assignee,
				start_date,
				due_date,
				deleted: false,
				tag_ids: []
			};
			state.tasks.push(created);
			keepTask(created, 'created');
			for (const tag_id of tag_ids) state.tagged.push({ tag_id, task_id: created.id });
			return withTags(created);
		},
		// Only the tasks whose status changed come back.
		changeTasksStatus: ({ workspace_id, task_ids, status }) =>
			task_ids
				.map((id) => task(workspace_id, id))
				.filter((t) => t.status !== status)
				.map((t) => {
					t.status = status;
					keepTask(t, 'edited');
					return { ...t };
				}),
		renameTask: ({ workspace_id, task_id, title }) => {
			const t = task(workspace_id, task_id);
			t.title = title;
			keepTask(t, 'edited');
			return { ...t };
		},
		setTaskPriority: ({ workspace_id, task_id, priority }) => {
			const t = task(workspace_id, task_id);
			t.priority = priority;
			return { ...t };
		},
		setTaskSize: ({ workspace_id, task_id, size }) => {
			const t = task(workspace_id, task_id);
			t.size = size;
			return { ...t };
		},
		setTaskAssignee: ({ workspace_id, task_id, assignee }) => {
			const t = task(workspace_id, task_id);
			if (assignee?.kind === 'agent') agent(assignee.agent_id);
			if (assignee?.kind === 'contact') contactRow(workspace_id, assignee.contact_id);
			t.assignee = assignee;
			return { ...t };
		},
		editTaskDescription: ({ workspace_id, task_id, description }) => {
			const t = task(workspace_id, task_id);
			t.description = description;
			keepTask(t, 'edited');
			return { ...t };
		},
		scheduleTask: ({ workspace_id, task_id, start_date, due_date }) => {
			const t = task(workspace_id, task_id);
			t.start_date = start_date;
			t.due_date = due_date;
			return { ...t };
		},
		moveTask: ({ workspace_id, task_id, folder_id = null }) => {
			const t = task(workspace_id, task_id);
			if (folder_id !== null) folder(workspace_id, folder_id);
			t.task_folder_id = folder_id;
			return { ...t };
		},
		deleteTask: ({ workspace_id, task_id }) => {
			const t = task(workspace_id, task_id);
			t.deleted = true;
			keepTask(t, 'deleted');
			return { ...t };
		},

		listTags: ({ workspace_id }) => {
			workspace(workspace_id);
			return tagsOf(workspace_id);
		},
		reorderTags: ({ workspace_id, tag_ids }) => {
			workspace(workspace_id);
			tag_ids.forEach((id, i) => (tagOf(workspace_id, id).sort_order = i));
			return tagsOf(workspace_id);
		},
		createTag: ({ workspace_id, name }) => {
			workspace(workspace_id);
			ensureTagName(workspace_id, name);
			const created: Tag = {
				id: nextId(state.tags),
				workspace_id,
				name,
				sort_order:
					Math.max(
						0,
						...state.tags.filter((t) => t.workspace_id === workspace_id).map((t) => t.sort_order)
					) + 1,
				created_at: nowSeconds()
			};
			state.tags.push(created);
			return { ...created };
		},
		renameTag: ({ workspace_id, tag_id, name }) => {
			const tag = tagOf(workspace_id, tag_id);
			ensureTagName(workspace_id, name, tag_id);
			tag.name = name;
			return { ...tag };
		},
		deleteTag: ({ workspace_id, tag_id }) => {
			const tag = tagOf(workspace_id, tag_id);
			state.tags = state.tags.filter((t) => t.id !== tag_id);
			state.tagged = state.tagged.filter((x) => x.tag_id !== tag_id);
			// The daemon takes it out of every saved view too.
			for (const v of state.savedTaskViews) {
				v.tags.with = v.tags.with.filter((id) => id !== tag_id);
				v.tags.without = v.tags.without.filter((id) => id !== tag_id);
			}
			return { ...tag };
		},
		setTagged: ({ workspace_id, tag_id, task_id, tagged }) => {
			tagOf(workspace_id, tag_id);
			const found = state.tasks.find((t) => t.id === task_id && t.workspace_id === workspace_id);
			if (!found)
				throw new ProtocolError('notFound', 'This operation contains an invalid reference.');
			state.tagged = state.tagged.filter((x) => !(x.tag_id === tag_id && x.task_id === task_id));
			if (tagged) state.tagged.push({ tag_id, task_id });
			return withTags(found);
		}
	} satisfies Partial<Handlers>;
}
