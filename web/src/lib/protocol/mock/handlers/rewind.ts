// Rewind: the versions kept of documents and tasks, and what was deleted.

import type { DeletedItem, Version } from '../../types';
import { nowSeconds } from '../support';
import type { Handlers, MockCore } from '../core';

export function rewindHandlers(core: MockCore) {
	const { state, tablesChanged, itemKey, keepDocument, keepTask, versionSummary, notFoundVersion } =
		core;
	return {
		listVersions: ({ workspace_id, item }) =>
			state.versions
				.filter(
					(v) =>
						v.workspace_id === workspace_id &&
						(item.kind === 'task'
							? v.item.kind === 'task' && v.item.task_id === item.task_id
							: v.item.kind === 'document' && v.item.document_id === item.document_id)
				)
				.map(versionSummary)
				.reverse(),
		getVersion: ({ workspace_id, version_id }) => {
			const v = state.versions.find((v) => v.workspace_id === workspace_id && v.id === version_id);
			if (!v) throw notFoundVersion();
			return { ...v };
		},
		restoreVersion: ({ workspace_id, version_id }) => {
			const v = state.versions.find((v) => v.workspace_id === workspace_id && v.id === version_id);
			if (!v) throw notFoundVersion();
			if (v.item.kind === 'task') {
				const { task_id, status } = v.item;
				const t = state.tasks.find((t) => t.id === task_id);
				if (!t) throw notFoundVersion();
				const unchanged =
					t.title === v.title && t.description === v.body && t.status === status && !t.deleted;
				if (unchanged) return { kind: 'task', task: { ...t } };
				const folder = state.folders.find((f) => f.id === t.task_folder_id);
				if (folder?.deleted) t.task_folder_id = null;
				Object.assign(t, { title: v.title, description: v.body, status, deleted: false });
				keepTask(t, 'restored');
				return { kind: 'task', task: { ...t } };
			}
			const { document_id } = v.item;
			const d = state.documents.find((d) => d.id === document_id);
			if (!d) throw notFoundVersion();
			if (d.title === v.title && d.contents === v.body && !d.deleted)
				return { kind: 'document', document: { ...d } };
			Object.assign(d, {
				title: v.title,
				contents: v.body,
				deleted: false,
				modified: nowSeconds()
			});
			keepDocument(d, 'restored');
			tablesChanged(d.folder_id);
			return { kind: 'document', document: { ...d } };
		},
		listDeletedItems: ({ workspace_id }) => {
			const latestDeleted = (key: string) =>
				state.versions.findLast((v) => itemKey(v) === key && v.change === 'deleted');
			const row = (
				v: Version | undefined,
				item: DeletedItem['item'],
				journal_date: string | null
			): DeletedItem[] =>
				v
					? [
							{
								item,
								title: v.title,
								journal_date,
								author: v.author,
								author_name: v.author_name,
								modified: v.modified
							}
						]
					: [];
			const tasks = state.tasks
				.filter((t) => t.workspace_id === workspace_id && t.deleted)
				.flatMap((t) => {
					const v = latestDeleted(`task:${t.id}`);
					return row(v, { kind: 'task', task_id: t.id, status: t.status }, null);
				});
			const documents = state.documents
				.filter((d) => d.workspace_id === workspace_id && d.deleted)
				.flatMap((d) =>
					row(
						latestDeleted(`document:${d.id}`),
						{ kind: 'document', document_id: d.id },
						d.journal_date
					)
				);
			return [...tasks, ...documents].sort((a, b) => b.modified - a.modified);
		}
	} satisfies Partial<Handlers>;
}
